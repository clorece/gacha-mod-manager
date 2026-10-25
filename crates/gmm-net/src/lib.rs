// SPDX-License-Identifier: MIT OR Apache-2.0

//! The only crate in GMM that opens network connections (security rule R2).
//!
//! - Every URL, including every redirect hop, must pass the compiled-in [`Allowlist`]:
//!   HTTPS on port 443, GameBanana hosts only.
//! - API requests share a token-bucket [`RateLimit`], and back off on 429/5xx.
//! - Responses are size-capped; downloads are written through [`gmm_fsguard`], hashed while
//!   streaming, verified against the expected size and MD5, and can be paused and resumed.

// This crate is the one place allowed to build HTTP clients (see clippy.toml).
#![allow(clippy::disallowed_types)]

mod allowlist;
mod client;
mod cooldown;
mod download;
mod error;
mod limiter;

pub use allowlist::Allowlist;
pub use client::{Client, HttpResponse, NetConfig, Slowdown};
pub use download::{DownloadOutcome, DownloadRequest, Progress};
pub use error::NetError;
pub use limiter::RateLimit;
pub use url::Url;
