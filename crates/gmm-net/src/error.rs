// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use gmm_fsguard::FsError;

use crate::client::Slowdown;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("host is not on the allowlist: {0}")]
    Disallowed(String),
    #[error("request to {url} failed: {message}")]
    Request { url: String, message: String },
    #[error("{url} answered HTTP {status}")]
    Status { url: String, status: u16 },
    #[error("{}", slow_down_message(*why, *retry_in))]
    SlowDown { why: Slowdown, retry_in: Duration },
    #[error("response from {url} is larger than {limit} bytes")]
    BodyTooLarge { url: String, limit: u64 },
    #[error("download is larger than allowed ({limit} bytes)")]
    FileTooLarge { limit: u64 },
    #[error("download size mismatch: expected {expected} bytes, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("download checksum mismatch (MD5 expected {expected}, got {actual})")]
    Md5Mismatch { expected: String, actual: String },
    #[error("download paused")]
    Paused,
    #[error("download destination already exists")]
    DestinationExists,
    #[error("writing the download failed: {0}")]
    Write(#[source] std::io::Error),
    #[error(transparent)]
    Fs(#[from] FsError),
    #[error("could not set up HTTPS: {0}")]
    Setup(String),
}

impl NetError {
    /// For a "slow down": how long until GMM tries GameBanana again.
    pub fn retry_in(&self) -> Option<Duration> {
        match self {
            Self::SlowDown { retry_in, .. } => Some(*retry_in),
            _ => None,
        }
    }

    /// Worth answering from saved data: GameBanana is unreachable, failing or asked GMM to
    /// wait (as opposed to a refused host, a bad file or a full disk).
    pub fn is_temporary(&self) -> bool {
        match self {
            Self::Request { .. } | Self::SlowDown { .. } => true,
            Self::Status { status, .. } => *status >= 500,
            _ => false,
        }
    }
}

fn slow_down_message(why: Slowdown, retry_in: Duration) -> String {
    let secs = retry_in.as_secs().max(1);
    let when = if secs >= 120 {
        format!("in about {} minutes", secs.div_ceil(60))
    } else {
        format!("in {secs} s")
    };
    match why {
        Slowdown::RateLimited => format!(
            "GameBanana asked GMM to slow down (too many requests from this network); GMM tries again {when}"
        ),
        Slowdown::Challenged => format!(
            "GameBanana's protection wants a browser check, which GMM can't do; GMM tries again {when}"
        ),
    }
}
