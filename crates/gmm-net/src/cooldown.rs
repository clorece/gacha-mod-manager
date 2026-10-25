// SPDX-License-Identifier: MIT OR Apache-2.0

//! GameBanana's "slow down" signals, shared by every client of one app: the API, the
//! background reads, images and downloads all go to GameBanana's hosts from the user's one IP
//! address, so a limit hit by any of them pauses all of them (docs/10-gamebanana-api.md).
//!
//! A strike is an HTTP 429, a 503 that says when to come back (`Retry-After`), or a
//! Cloudflare challenge (a 403 or 503 with `cf-mitigated: challenge`, the browser check GMM
//! can't pass). The pause is the server's `Retry-After` when it sends one, otherwise 30 s
//! doubling with each strike in a row, always between 5 s and 15 min. Any success clears the
//! strikes. While paused, requests fail at once with how long is left instead of adding to
//! the load, and callers fall back to saved data or wait (the install queue).

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

const FIRST_PAUSE: Duration = Duration::from_secs(30);
const MIN_PAUSE: Duration = Duration::from_secs(5);
const MAX_PAUSE: Duration = Duration::from_secs(15 * 60);

/// Pauses this short are waited out instead of failing the request.
pub(crate) const WAIT_OUT: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
pub(crate) struct Cooldown(Mutex<State>);

#[derive(Debug, Default)]
struct State {
    until: Option<Instant>,
    strikes: u32,
}

impl Cooldown {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How long GMM still holds off, if it does.
    pub(crate) fn remaining(&self) -> Option<Duration> {
        let until = self.state().until?;
        let left = until.saturating_duration_since(Instant::now());
        (!left.is_zero()).then_some(left)
    }

    /// Records a "slow down" and returns how long to hold off.
    pub(crate) fn strike(&self, retry_after: Option<Duration>) -> Duration {
        let mut state = self.state();
        let doubling = FIRST_PAUSE.saturating_mul(1 << state.strikes.min(10));
        let pause = retry_after.unwrap_or(doubling).clamp(MIN_PAUSE, MAX_PAUSE);
        state.strikes = state.strikes.saturating_add(1);
        let until = Instant::now() + pause;
        // A later strike never shortens a pause already running.
        state.until = Some(state.until.map_or(until, |u| u.max(until)));
        pause
    }

    /// A request went through: GameBanana isn't limiting GMM (any more).
    pub(crate) fn clear(&self) {
        let mut state = self.state();
        state.strikes = 0;
        if state.until.is_some_and(|u| u <= Instant::now()) {
            state.until = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pauses_double_and_reset_on_success() {
        let c = Cooldown::default();
        assert_eq!(c.remaining(), None);
        assert_eq!(c.strike(None), Duration::from_secs(30));
        assert_eq!(c.strike(None), Duration::from_secs(60));
        assert_eq!(c.strike(None), Duration::from_secs(120));
        assert!(c.remaining().is_some_and(|d| d > Duration::from_secs(110)));
        c.clear();
        assert!(c.remaining().is_some(), "a running pause isn't cut short");
        assert_eq!(
            c.strike(None),
            Duration::from_secs(30),
            "strikes start over"
        );
    }

    #[test]
    fn retry_after_is_used_within_bounds() {
        let c = Cooldown::default();
        assert_eq!(c.strike(Some(Duration::from_secs(1))), MIN_PAUSE);
        assert_eq!(
            c.strike(Some(Duration::from_secs(90))),
            Duration::from_secs(90)
        );
        assert_eq!(c.strike(Some(Duration::from_secs(86_400))), MAX_PAUSE);
        for _ in 0..40 {
            assert!(c.strike(None) <= MAX_PAUSE);
        }
    }
}
