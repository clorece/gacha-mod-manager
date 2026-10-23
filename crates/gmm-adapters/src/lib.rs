// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-game rules ("adapters"), see `docs/03-game-adapters.md`.
//!
//! Each supported game is a TOML manifest in `games/`, compiled into the binary with
//! `include_str!` and validated when the [`Registry`] is built. Manifests are data only: they
//! can't add hosts, filesystem roots or launch arguments. The XXMI launch command, for
//! example, is always built in code from the importer id. Code hooks for formats a manifest
//! can't express get added per game when they're needed.

mod manifest;
mod registry;
mod validate;

pub use manifest::{
    Conflicts, Detect, Engine, EpicDetect, GameBananaInfo, GameInfo, GameManifest, Importer,
    ModKind, RootCategory, SteamDetect, Tag, Xxmi,
};
pub use registry::{ManifestError, Registry};
