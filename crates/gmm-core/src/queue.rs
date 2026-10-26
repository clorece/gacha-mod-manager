// SPDX-License-Identifier: MIT OR Apache-2.0

//! The install queue behind the Downloads page: installs run one at a
//! time in the order they were confirmed, each with a state the page shows, and each can be
//! paused, resumed or cancelled through its [`StopSignal`]. An install whose background
//! check found something ([`crate::review`]) waits in [`JobState::NeedsAction`] until the
//! user goes ahead ([`InstallQueue::proceed`]) or cancels. This is the bookkeeping only;
//! the app runs the installs (one worker taking [`InstallQueue::next`]).

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::install::{InstallOptions, InstalledFolder, VariantChoice};
use crate::plan::Review;
use crate::stop::StopSignal;

pub type JobId = u64;

/// Where one install is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobState {
    Queued,
    /// `total` is what GameBanana said, when known.
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    Extracting,
    Installing {
        folder: String,
    },
    /// GameBanana asked GMM to slow down; the install carries on by itself at `until`
    /// (Unix seconds). Pausing or cancelling still works.
    Waiting {
        until: i64,
    },
    Paused,
    /// The background check found something the user has to look at (versions to choose,
    /// blocked files, an outdated file). Nothing is installed until they go ahead.
    NeedsAction {
        review: Review,
    },
    Done {
        installed: Vec<InstalledFolder>,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

impl JobState {
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            Self::Done { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }

    fn is_running(&self) -> bool {
        matches!(
            self,
            Self::Downloading { .. }
                | Self::Extracting
                | Self::Installing { .. }
                | Self::Waiting { .. }
        )
    }
}

/// What a job does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum JobAction {
    /// Installs a mod (the install confirmation).
    #[default]
    Install,
    /// Moves an installed mod to another of its files: an update or a roll back
    /// ([`crate::change_file`]).
    ChangeFile,
}

/// What to install; set when the install is confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRequest {
    pub game: String,
    pub mod_id: u64,
    pub file_id: Option<u64>,
    pub options: InstallOptions,
    pub action: JobAction,
    /// The user looked at the background check's findings and went ahead: install without
    /// stopping again.
    pub reviewed: bool,
    /// Mods (library row ids) to switch off once this one is installed and on: the other
    /// side of a conflict the user resolved in the install confirmation.
    pub switch_off: Vec<i64>,
}

/// One row on the Downloads page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Job {
    pub id: JobId,
    pub action: JobAction,
    pub game: String,
    pub mod_id: u64,
    pub mod_name: String,
    pub author: Option<String>,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    /// The mod's preview, as the caller wants it shown (the app passes a `gmmimg` URL).
    pub image: Option<String>,
    /// How many versions were chosen (0 when the archive holds one mod).
    pub versions: usize,
    /// Switched on once installed ("Switch it on after installing").
    pub switch_on: bool,
    pub state: JobState,
    /// Unix seconds.
    pub queued_at: i64,
    pub finished_at: Option<i64>,
}

/// What the Downloads page shows for a job, from the mod page when it was queued.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobLabel {
    pub mod_name: String,
    pub author: Option<String>,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub image: Option<String>,
}

impl JobLabel {
    pub fn named(mod_name: impl Into<String>) -> Self {
        Self {
            mod_name: mod_name.into(),
            ..Self::default()
        }
    }
}

struct Entry {
    job: Job,
    request: JobRequest,
    stop: StopSignal,
}

/// Thread-safe; share it behind an `Arc`.
#[derive(Default)]
pub struct InstallQueue {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    next_id: JobId,
    entries: Vec<Entry>,
}

/// A job handed to the worker.
#[derive(Debug, Clone)]
pub struct NextJob {
    pub id: JobId,
    pub request: JobRequest,
    pub stop: StopSignal,
}

impl InstallQueue {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic while holding the lock leaves plain data behind; keep using it.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds an install at the end of the queue.
    pub fn push(&self, request: JobRequest, label: JobLabel) -> JobId {
        let mut inner = self.lock();
        inner.next_id += 1;
        let id = inner.next_id;
        let versions = version_count(&request.options.variants);
        inner.entries.push(Entry {
            job: Job {
                id,
                action: request.action,
                game: request.game.clone(),
                mod_id: request.mod_id,
                mod_name: label.mod_name,
                author: label.author,
                file_name: label.file_name,
                file_size: label.file_size,
                image: label.image,
                versions,
                switch_on: !request.options.start_disabled,
                state: JobState::Queued,
                queued_at: now(),
                finished_at: None,
            },
            request,
            stop: StopSignal::new(),
        });
        id
    }

    /// Every job, oldest first.
    pub fn snapshot(&self) -> Vec<Job> {
        self.lock().entries.iter().map(|e| e.job.clone()).collect()
    }

    /// The oldest queued job, now marked as downloading; `None` when nothing waits or one
    /// is already running.
    pub fn next(&self) -> Option<NextJob> {
        let mut inner = self.lock();
        if inner.entries.iter().any(|e| e.job.state.is_running()) {
            return None;
        }
        let entry = inner
            .entries
            .iter_mut()
            .find(|e| e.job.state == JobState::Queued)?;
        entry.job.state = JobState::Downloading {
            downloaded: 0,
            total: None,
        };
        Some(NextJob {
            id: entry.job.id,
            request: entry.request.clone(),
            stop: entry.stop.clone(),
        })
    }

    /// Records progress or the outcome. Ignored for finished jobs, so a late progress event
    /// can't revive one.
    pub fn set_state(&self, id: JobId, state: JobState) {
        let mut inner = self.lock();
        if let Some(e) = inner.entries.iter_mut().find(|e| e.job.id == id)
            && !e.job.state.is_finished()
        {
            if state.is_finished() {
                e.job.finished_at = Some(now());
            }
            e.job.state = state;
        }
    }

    /// Also records the file name once it's known.
    pub fn set_file_name(&self, id: JobId, name: &str) {
        if let Some(e) = self.lock().entries.iter_mut().find(|e| e.job.id == id) {
            e.job.file_name = Some(name.to_owned());
        }
    }

    /// Stops a job after its background check: it waits for the user ([`Self::proceed`] or
    /// [`Self::cancel`]).
    pub fn hold(&self, id: JobId, review: Review) {
        self.set_state(id, JobState::NeedsAction { review });
    }

    /// Goes ahead with a job that needs action, as reviewed: with `variants` when the user
    /// chose versions, without blocked files. Returns false when it isn't waiting.
    pub fn proceed(&self, id: JobId, variants: Option<VariantChoice>) -> bool {
        let mut inner = self.lock();
        match inner.entries.iter_mut().find(|e| e.job.id == id) {
            Some(e) if matches!(e.job.state, JobState::NeedsAction { .. }) => {
                if let Some(variants) = variants {
                    e.job.versions = version_count(&variants);
                    e.request.options.variants = variants;
                }
                e.request.options.skip_blocked = true;
                e.request.reviewed = true;
                e.job.state = JobState::Queued;
                true
            }
            _ => false,
        }
    }

    /// Pauses a queued or running job. Returns false if there is nothing to pause.
    pub fn pause(&self, id: JobId) -> bool {
        let mut inner = self.lock();
        let Some(e) = inner.entries.iter_mut().find(|e| e.job.id == id) else {
            return false;
        };
        match e.job.state {
            JobState::Queued => {
                e.job.state = JobState::Paused;
                true
            }
            // The worker sees the signal and reports `Paused` itself.
            ref s if s.is_running() => {
                e.stop.pause();
                true
            }
            _ => false,
        }
    }

    /// Puts a paused job back in the queue.
    pub fn resume(&self, id: JobId) -> bool {
        let mut inner = self.lock();
        match inner.entries.iter_mut().find(|e| e.job.id == id) {
            Some(e) if e.job.state == JobState::Paused => {
                e.stop.resume();
                e.job.state = JobState::Queued;
                true
            }
            _ => false,
        }
    }

    /// Cancels a job that hasn't finished. A running one stops at its next check.
    pub fn cancel(&self, id: JobId) -> bool {
        let mut inner = self.lock();
        let Some(e) = inner.entries.iter_mut().find(|e| e.job.id == id) else {
            return false;
        };
        if e.job.state.is_finished() {
            return false;
        }
        e.stop.cancel();
        if !e.job.state.is_running() {
            e.job.state = JobState::Cancelled;
            e.job.finished_at = Some(now());
        }
        true
    }

    /// Removes finished jobs (the page's "Clear finished", and the self-clearing installed
    /// rows). Returns how many were removed.
    pub fn clear_finished(&self) -> usize {
        let mut inner = self.lock();
        let before = inner.entries.len();
        inner.entries.retain(|e| !e.job.state.is_finished());
        before - inner.entries.len()
    }

    /// Removes one finished job ("Dismiss").
    pub fn dismiss(&self, id: JobId) -> bool {
        let mut inner = self.lock();
        let before = inner.entries.len();
        inner
            .entries
            .retain(|e| e.job.id != id || !e.job.state.is_finished());
        before != inner.entries.len()
    }

    /// Jobs that are queued or running (the sidebar's Downloads count).
    pub fn active(&self) -> usize {
        self.lock()
            .entries
            .iter()
            .filter(|e| !e.job.state.is_finished() && e.job.state != JobState::Paused)
            .count()
    }
}

fn version_count(variants: &VariantChoice) -> usize {
    match variants {
        VariantChoice::Named(names) => names.len(),
        _ => 0,
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stop::StopReason;

    fn request(mod_id: u64) -> JobRequest {
        JobRequest {
            game: "zzz".into(),
            mod_id,
            file_id: None,
            options: InstallOptions::default(),
            action: JobAction::Install,
            reviewed: false,
            switch_off: Vec::new(),
        }
    }

    #[test]
    fn runs_one_at_a_time_in_order() {
        let q = InstallQueue::new();
        let a = q.push(request(1), JobLabel::named("A"));
        let b = q.push(
            request(2),
            JobLabel {
                file_name: Some("b.rar".into()),
                ..JobLabel::named("B")
            },
        );
        assert_eq!(q.active(), 2);
        let first = q.next().unwrap();
        assert_eq!(first.id, a);
        assert!(q.next().is_none(), "only one install runs at a time");
        q.set_state(a, JobState::Done { installed: vec![] });
        assert_eq!(q.next().unwrap().id, b);
        q.set_state(
            b,
            JobState::Failed {
                message: "x".into(),
            },
        );
        // A late progress event doesn't revive a finished job.
        q.set_state(b, JobState::Extracting);
        let jobs = q.snapshot();
        assert!(matches!(jobs[1].state, JobState::Failed { .. }));
        assert!(jobs.iter().all(|j| j.finished_at.is_some()));
        assert_eq!(q.active(), 0);
        assert_eq!(q.clear_finished(), 2);
        assert!(q.snapshot().is_empty());
    }

    #[test]
    fn pause_resume_and_cancel() {
        let q = InstallQueue::new();
        let a = q.push(request(1), JobLabel::named("A"));
        let b = q.push(request(2), JobLabel::named("B"));

        // A queued job pauses at once and is skipped by the worker.
        assert!(q.pause(b));
        let running = q.next().unwrap();
        assert_eq!(running.id, a);

        // A running job is asked to stop; the worker reports the outcome.
        assert!(q.pause(a));
        assert_eq!(running.stop.requested(), Some(StopReason::Paused));
        q.set_state(a, JobState::Paused);
        assert!(q.resume(a));
        assert_eq!(running.stop.requested(), None);

        assert!(q.cancel(b));
        assert_eq!(q.snapshot()[1].state, JobState::Cancelled);
        assert!(!q.resume(b), "a cancelled job stays cancelled");
        assert!(!q.cancel(b));

        let again = q.next().unwrap();
        assert_eq!(again.id, a);
        assert!(q.cancel(a));
        assert_eq!(again.stop.requested(), Some(StopReason::Cancelled));
        q.set_state(a, JobState::Cancelled);
        assert!(q.dismiss(a));
        assert_eq!(q.snapshot().len(), 1);
    }

    #[test]
    fn counts_versions() {
        let q = InstallQueue::new();
        let mut r = request(1);
        r.options.variants = crate::install::VariantChoice::Named(vec!["A".into(), "B".into()]);
        q.push(r, JobLabel::named("Pack"));
        assert_eq!(q.snapshot()[0].versions, 2);
    }

    #[test]
    fn needs_action_waits_until_the_user_goes_ahead() {
        let q = InstallQueue::new();
        let a = q.push(request(1), JobLabel::named("Pack"));
        let b = q.push(request(2), JobLabel::named("Other"));
        let first = q.next().unwrap();
        assert!(!first.request.reviewed);
        q.hold(
            a,
            Review {
                issues: vec![crate::plan::CheckIssue::ChooseVersions { count: 2 }],
                versions: Vec::new(),
                version_conflicts: Vec::new(),
            },
        );
        // It isn't running or finished: the next install goes ahead, and it still counts.
        assert_eq!(q.next().unwrap().id, b);
        assert_eq!(q.active(), 2);
        assert!(!q.pause(a), "nothing to pause while it waits");
        assert!(
            !q.proceed(b, None),
            "only a job that needs action can go ahead"
        );

        q.set_state(b, JobState::Done { installed: vec![] });
        assert!(q.proceed(a, Some(VariantChoice::Named(vec!["Red".into()]))));
        assert_eq!(q.snapshot()[0].versions, 1);
        let again = q.next().unwrap();
        assert_eq!(again.id, a);
        assert!(again.request.reviewed && again.request.options.skip_blocked);
        assert_eq!(
            again.request.options.variants,
            VariantChoice::Named(vec!["Red".into()])
        );

        // Cancelling one that waits finishes it at once.
        q.hold(
            a,
            Review {
                issues: Vec::new(),
                versions: Vec::new(),
                version_conflicts: Vec::new(),
            },
        );
        assert!(q.cancel(a));
        assert_eq!(q.snapshot()[0].state, JobState::Cancelled);
    }
}
