// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pausing or cancelling an install from outside (the Downloads page). The install checks
//! its [`StopSignal`] between steps and while downloading; a paused download keeps its
//! partial file, so starting the same install again resumes it (`gmm-net`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const RUN: u8 = 0;
const PAUSE: u8 = 1;
const CANCEL: u8 = 2;

/// Why an install stopped early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The download's partial file is kept for a resume.
    Paused,
    Cancelled,
}

/// Shared between whoever runs an install and whoever may stop it. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct StopSignal(Arc<AtomicU8>);

impl StopSignal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pause(&self) {
        // A cancel stays a cancel.
        let _ = self
            .0
            .compare_exchange(RUN, PAUSE, Ordering::SeqCst, Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.0.store(CANCEL, Ordering::SeqCst);
    }

    /// Clears a pause, so the same signal can run the install again.
    pub fn resume(&self) {
        let _ = self
            .0
            .compare_exchange(PAUSE, RUN, Ordering::SeqCst, Ordering::SeqCst);
    }

    /// `Some` once a pause or cancel was asked for.
    pub fn requested(&self) -> Option<StopReason> {
        match self.0.load(Ordering::SeqCst) {
            PAUSE => Some(StopReason::Paused),
            CANCEL => Some(StopReason::Cancelled),
            _ => None,
        }
    }

    /// `Err(Stopped)` if a pause or cancel was asked for.
    pub fn check(&self) -> Result<(), crate::CoreError> {
        match self.requested() {
            Some(reason) => Err(crate::CoreError::Stopped(reason)),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_resume_cancel() {
        let s = StopSignal::new();
        assert_eq!(s.requested(), None);
        assert!(s.check().is_ok());
        let other = s.clone();
        other.pause();
        assert_eq!(s.requested(), Some(StopReason::Paused));
        s.resume();
        assert_eq!(other.requested(), None);
        s.cancel();
        s.pause();
        s.resume();
        assert_eq!(s.requested(), Some(StopReason::Cancelled));
        assert!(matches!(
            s.check(),
            Err(crate::CoreError::Stopped(StopReason::Cancelled))
        ));
    }
}
