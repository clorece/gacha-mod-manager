// SPDX-License-Identifier: MIT OR Apache-2.0

use gmm_fsguard::FsError;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("host is not on the allowlist: {0}")]
    Disallowed(String),
    #[error("request to {url} failed: {message}")]
    Request { url: String, message: String },
    #[error("{url} answered HTTP {status}")]
    Status { url: String, status: u16 },
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
