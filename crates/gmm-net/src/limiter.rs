// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};

/// Token bucket: `per_second` requests on average, bursts of up to `burst`.
#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    pub per_second: f64,
    pub burst: u32,
}

impl Default for RateLimit {
    /// About 2 requests/second with bursts of 5, to stay polite to GameBanana.
    fn default() -> Self {
        Self {
            per_second: 2.0,
            burst: 5,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Limiter {
    limit: RateLimit,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    tokens: f64,
    last: Instant,
}

impl Limiter {
    pub(crate) fn new(limit: RateLimit) -> Self {
        Self {
            limit,
            state: Mutex::new(State {
                tokens: f64::from(limit.burst),
                last: Instant::now(),
            }),
        }
    }

    /// Waits until a request may be sent.
    pub(crate) async fn acquire(&self) {
        loop {
            let wait = {
                let mut state = self.state.lock().await;
                let now = Instant::now();
                let elapsed = now.duration_since(state.last).as_secs_f64();
                state.tokens = (state.tokens + elapsed * self.limit.per_second)
                    .min(f64::from(self.limit.burst));
                state.last = now;
                if state.tokens >= 1.0 {
                    state.tokens -= 1.0;
                    return;
                }
                Duration::from_secs_f64((1.0 - state.tokens) / self.limit.per_second)
            };
            sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn bursts_then_paces() {
        let limiter = Limiter::new(RateLimit {
            per_second: 2.0,
            burst: 3,
        });
        let start = Instant::now();
        for _ in 0..3 {
            limiter.acquire().await;
        }
        assert!(
            start.elapsed() < Duration::from_millis(1),
            "burst is immediate"
        );
        limiter.acquire().await;
        limiter.acquire().await;
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(990) && elapsed < Duration::from_millis(1100),
            "two more requests at 2/s take ~1s, took {elapsed:?}"
        );
    }
}
