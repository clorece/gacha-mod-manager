// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scoped filesystem access for GMM.
//!
//! Every file GMM reads or writes goes through a [`Guard`]. The guard knows a fixed set of
//! registered roots (app folders, XXMI `Mods/` folders, game folders) and whether each one is
//! read-only or read-write. A [`ScopedPath`] can only be built from a registered root plus a
//! validated relative path ([`RelPath`]), so code elsewhere in GMM cannot name an arbitrary
//! location on disk. See security rules R3/R4 in `docs/04-security.md`.

// This crate is the one place allowed to call `std::fs` directly (see clippy.toml).
#![allow(clippy::disallowed_methods)]

mod deny;
mod error;
mod guard;
mod relpath;

pub use error::{FsError, PathError};
pub use guard::{Access, DirEntryInfo, Guard, RootId, ScopedPath};
pub use relpath::RelPath;
