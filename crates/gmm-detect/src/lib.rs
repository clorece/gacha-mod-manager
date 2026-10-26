// SPDX-License-Identifier: MIT OR Apache-2.0

//! Finds installed games (docs/game-adapters.md, "Game detection").
//!
//! Every source only reads the exact files allowlisted in docs/security.md (R3), through
//! [`gmm_fsguard`] with read-only roots, and every candidate folder must pass the game's folder
//! markers before it counts. That marker check is what makes it safe to scan loosely structured
//! sources like HoYoPlay's data file: a path only counts if the game is really there.

mod env;
mod epic;
mod hoyoplay;
mod markers;
mod steam;
pub mod vdf;

use std::path::{Path, PathBuf};

use gmm_adapters::{GameManifest, Registry};
use serde::Serialize;

pub use env::{Environment, SystemEnvironment};
pub use markers::{is_game_folder, resolve_game_folder};

/// Where an install was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum InstallSource {
    Steam,
    HoYoPlay,
    Epic,
    /// A folder the user named.
    Manual,
}

impl InstallSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Steam => "Steam",
            Self::HoYoPlay => "HoYoPlay",
            Self::Epic => "Epic Games",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GameInstall {
    pub game_id: String,
    pub source: InstallSource,
    /// Canonical path of the folder holding the game exe.
    pub folder: PathBuf,
    pub steam_appid: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DetectReport {
    pub installs: Vec<GameInstall>,
    /// Sources that couldn't be read, and why. Informational; detection carries on.
    pub notes: Vec<String>,
}

impl DetectReport {
    pub fn for_game<'a>(&'a self, game_id: &'a str) -> impl Iterator<Item = &'a GameInstall> {
        self.installs.iter().filter(move |i| i.game_id == game_id)
    }
}

/// Runs every source for every registered game. `manual` adds user-named folders
/// (game id, folder); they're validated like everything else.
pub fn detect(
    registry: &Registry,
    env: &dyn Environment,
    manual: &[(String, PathBuf)],
) -> DetectReport {
    let mut report = DetectReport::default();
    steam::detect(registry, env, &mut report);
    hoyoplay::detect(registry, env, &mut report);
    epic::detect(registry, env, &mut report);
    for (game_id, folder) in manual {
        match registry.get(game_id) {
            Some(game) => add_candidate(&mut report, game, InstallSource::Manual, folder, None),
            None => report.notes.push(format!("unknown game {game_id:?}")),
        }
    }
    report
}

/// Validates `folder` for `game` (also trying the manifest's typical sub-folder names) and
/// records it once.
pub(crate) fn add_candidate(
    report: &mut DetectReport,
    game: &GameManifest,
    source: InstallSource,
    folder: &Path,
    steam_appid: Option<u32>,
) {
    let found = markers::resolve_game_folder(game, folder);
    let Some(folder) = found else {
        if source == InstallSource::Manual {
            report.notes.push(format!(
                "{} is not a {} folder (expected {})",
                folder.display(),
                game.game.name,
                game.detect.exe_names.join(" or ")
            ));
        }
        return;
    };
    let duplicate = report
        .installs
        .iter()
        .any(|i| i.game_id == game.id() && same_path(&i.folder, &folder));
    if !duplicate {
        report.installs.push(GameInstall {
            game_id: game.id().to_owned(),
            source,
            folder,
            steam_appid,
        });
    }
}

/// Case-insensitive comparison, since Windows paths are.
pub fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches(['\\', '/']))
}
