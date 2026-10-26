// SPDX-License-Identifier: MIT OR Apache-2.0

//! XXMI, read-only (docs/03-game-adapters.md, "XXMI detection & setup states").
//!
//! - [`find_xxmi`] locates and validates the XXMI Launcher folder.
//! - [`XxmiConfig`] reads its config file through a struct that declares only the fields GMM
//!   needs. Everything else in that file (including `github_token`, `proxy` and signatures)
//!   is skipped by the parser and never stored (security rule R4).
//! - [`inspect_importer`] checks an importer folder (`ZZMI/`, `WWMI/`, ...).
//! - [`evaluate`] combines all of that with detected installs into a [`GameStatus`]: one setup
//!   state per game, with the steps to fix it.
//!
//! Nothing here writes anywhere.

mod config;
mod find;
mod importer;
mod launch;
mod status;

pub use config::{ImporterConfig, XxmiConfig};
pub use find::{XxmiInstall, find_xxmi, open_xxmi};
pub use importer::{ImporterInfo, ModsSummary, inspect_importer};
pub use launch::{
    LaunchCommand, MAX_PAGE_URL, check_page_url, game_running, launch, launch_command, open_folder,
    open_page,
};
pub use status::{GameStatus, SetupState, XXMI_RELEASES, evaluate};
