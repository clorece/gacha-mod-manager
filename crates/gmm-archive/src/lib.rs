// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hardened extraction of mod archives (security threat T2 in `docs/04-security.md`).
//!
//! [`extract`] unpacks a `.zip`, `.7z` or `.rar` into a fresh directory through [`gmm_fsguard`]. Every
//! entry is checked before anything is written: unsafe names, links, special files, case
//! collisions, entry counts, nesting depth and path length. While writing, actual decompressed
//! bytes are counted against size and compression-ratio limits, so archives that lie about
//! their sizes are caught too. On any failure the destination is removed. RAR is read by the
//! sandboxed `gmm-extract` helper process ([`Helper`], [`rar_protocol`]); GMM still runs
//! every check and writes every file itself.
//!
//! [`scan`] then inspects what came out and reports files that must not be deployed
//! (executables, scripts) or that deserve a warning (loader configs, nested archives).

mod error;
mod extract;
mod format;
mod helper;
mod limits;
mod plan;
mod rar_fmt;
pub mod rar_protocol;
mod scan;
mod sevenz_fmt;
mod zip_fmt;

pub use error::ArchiveError;
pub use extract::{ExtractReport, ExtractedEntry, extract, extract_with};
pub use format::{Format, detect_format};
pub use helper::Helper;
pub use limits::Limits;
pub use scan::{Finding, FindingKind, ScanOptions, ScanReport, Severity, scan};
