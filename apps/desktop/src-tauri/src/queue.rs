// SPDX-License-Identifier: MIT OR Apache-2.0

//! The install queue behind the Downloads page: commands to add, pause, resume, cancel and
//! clear installs, and the one worker task that runs them in order
//! (`gmm_core::InstallQueue` keeps the list). Watchers get the whole list over a channel
//! whenever it changes; download progress is sent at most every 200 ms (`ProgressThrottle`).

use std::sync::{Mutex, PoisonError};

use gmm_catalog::CatalogSource;
use gmm_core::{
    CoreError, FetchEvent, InstallEvent, InstallOptions, InstallRequest, Job, JobId, JobLabel,
    JobRequest, JobState, NextJob, StopReason,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::AppState;
use crate::commands::{game_by_id, mods_target};
use crate::error::CmdResult;
use crate::views::{InstallArgs, ProgressThrottle, rewrite_summary};

/// Who gets the list when it changes.
#[derive(Default)]
pub struct Watchers(Mutex<Vec<Channel<Vec<Job>>>>);

impl Watchers {
    fn add(&self, channel: Channel<Vec<Job>>) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(channel);
    }

    /// Sends `jobs` to every watcher, dropping the ones whose window has gone.
    fn send(&self, jobs: &[Job]) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|w| w.send(jobs.to_vec()).is_ok());
    }
}

fn broadcast(state: &AppState) {
    state.watchers.send(&state.queue.snapshot());
}

/// Starts the worker: it waits for a queued install, runs it, and waits again.
pub fn spawn_worker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            match state.queue.next() {
                Some(job) => {
                    broadcast(&state);
                    run(&state, job).await;
                    broadcast(&state);
                }
                None => state.wake.notified().await,
            }
        }
    });
}

async fn run(state: &AppState, job: NextJob) {
    let outcome = install(state, &job).await;
    let final_state = match outcome {
        Ok(installed) => JobState::Done { installed },
        Err(CoreError::Stopped(StopReason::Paused)) => JobState::Paused,
        Err(CoreError::Stopped(StopReason::Cancelled)) => JobState::Cancelled,
        Err(e) => JobState::Failed {
            message: e.to_string(),
        },
    };
    state.queue.set_state(job.id, final_state);
}

async fn install(
    state: &AppState,
    job: &NextJob,
) -> Result<Vec<gmm_core::InstalledFolder>, CoreError> {
    let game = game_by_id(&job.request.game).map_err(|e| CoreError::Io(e.message().to_owned()))?;
    let target = mods_target(state, game)
        .await
        .map_err(|e| CoreError::Io(e.message().to_owned()))?;
    let request = InstallRequest {
        mod_id: job.request.mod_id,
        file_id: job.request.file_id,
        options: job.request.options.clone(),
    };
    let mut throttle = ProgressThrottle::default();
    let report = gmm_core::install(
        &state.gb,
        &state.net,
        &state.db,
        &target.ws,
        target.mods,
        game,
        &request,
        &job.stop,
        |event| {
            let send = throttle.pass(&event);
            let next = match event {
                InstallEvent::Fetch(FetchEvent::UsingCache) => JobState::Extracting,
                InstallEvent::Fetch(FetchEvent::Downloading { size }) => JobState::Downloading {
                    downloaded: 0,
                    total: Some(size),
                },
                InstallEvent::Fetch(FetchEvent::Progress(p)) => JobState::Downloading {
                    downloaded: p.downloaded,
                    total: p.total,
                },
                InstallEvent::Extracting => JobState::Extracting,
                InstallEvent::Installing { folder } => JobState::Installing { folder },
            };
            state.queue.set_state(job.id, next);
            if send {
                broadcast(state);
            }
        },
    )
    .await?;
    // Versions that replace the same things can't all be on: the first one picked stays on
    // and the ones clashing with it are switched off (docs/08, "Install confirmation").
    if !job.request.options.start_disabled && report.installed.len() > 1 {
        gmm_core::switch_version(
            &state.db,
            &target.ws,
            target.mods,
            game,
            report.installed[0].id,
        )
        .await?;
    }
    Ok(report.installed)
}

/// Adds a confirmed install to the queue (after the install confirmation). Returns its id.
#[tauri::command]
pub async fn queue_install(state: State<'_, AppState>, args: InstallArgs) -> CmdResult<JobId> {
    let profile = state.gb.mod_profile(args.mod_id).await?;
    let game = gmm_core::game_for_mod(&profile)?;
    // Refuse now, not in the queue, when there's no Mods folder to install into.
    mods_target(&state, game).await?;
    let file = match args.file_id {
        Some(id) => profile
            .files
            .files
            .iter()
            .chain(&profile.files.archived_files)
            .find(|f| f.id == id),
        None => profile.files.default_choice(),
    };
    let mut summary = profile.summary.clone();
    rewrite_summary(&state.images, &mut summary);
    let label = JobLabel {
        mod_name: profile.summary.name.clone(),
        author: Some(profile.summary.submitter.name.clone()).filter(|n| !n.is_empty()),
        file_name: file.map(|f| f.name.clone()),
        file_size: file.map(|f| f.size),
        image: summary.banner().map(|b| b.url_for_width(220).to_owned()),
    };
    let id = state.queue.push(
        JobRequest {
            game: game.id().to_owned(),
            mod_id: args.mod_id,
            file_id: args.file_id,
            options: InstallOptions {
                variants: args.variants.into(),
                skip_blocked: args.skip_blocked,
                start_disabled: args.start_disabled,
            },
        },
        label,
    );
    state.wake.notify_one();
    broadcast(&state);
    Ok(id)
}

/// Every install in the queue, oldest first.
#[tauri::command]
pub fn queue_jobs(state: State<'_, AppState>) -> Vec<Job> {
    state.queue.snapshot()
}

/// Sends the list now and again whenever it changes.
#[tauri::command]
pub fn queue_watch(state: State<'_, AppState>, on_change: Channel<Vec<Job>>) {
    let _ = on_change.send(state.queue.snapshot());
    state.watchers.add(on_change);
}

/// Pauses a queued or running install (a download keeps its partial file).
#[tauri::command]
pub fn queue_pause(state: State<'_, AppState>, id: JobId) -> bool {
    let done = state.queue.pause(id);
    broadcast(&state);
    done
}

/// Puts a paused install back in the queue; a download picks up where it stopped.
#[tauri::command]
pub fn queue_resume(state: State<'_, AppState>, id: JobId) -> bool {
    let done = state.queue.resume(id);
    state.wake.notify_one();
    broadcast(&state);
    done
}

#[tauri::command]
pub fn queue_cancel(state: State<'_, AppState>, id: JobId) -> bool {
    let done = state.queue.cancel(id);
    broadcast(&state);
    done
}

/// Removes one finished install from the list.
#[tauri::command]
pub fn queue_dismiss(state: State<'_, AppState>, id: JobId) -> bool {
    let done = state.queue.dismiss(id);
    broadcast(&state);
    done
}

/// Removes every finished install from the list. Returns how many.
#[tauri::command]
pub fn queue_clear_finished(state: State<'_, AppState>) -> usize {
    let n = state.queue.clear_finished();
    broadcast(&state);
    n
}
