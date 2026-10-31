// SPDX-License-Identifier: MIT OR Apache-2.0

//! GRYPHLINK (Gryphline's launcher for Arknights: Endfield) installs games into
//! `<launcher>\games\<game folder>` (XXMI source). Its registry `install_path` names the folder
//! holding the launcher's `Games.exe`, which may be the launcher folder or a folder inside it,
//! so both that folder and its parent are tried. Nothing in the launcher's folder is read: each
//! candidate only counts if it passes a game's folder markers.

use std::path::PathBuf;

use gmm_adapters::Registry;

use crate::env::Environment;
use crate::{DetectReport, InstallSource, add_candidate, same_path};

pub(crate) fn detect(registry: &Registry, env: &dyn Environment, report: &mut DetectReport) {
    let mut games_folders: Vec<PathBuf> = Vec::new();
    for root in env.gryphlink_roots() {
        for base in [Some(root.as_path()), root.parent()].into_iter().flatten() {
            let games = base.join("games");
            if !games_folders.iter().any(|g| same_path(g, &games)) {
                games_folders.push(games);
            }
        }
    }
    for games in &games_folders {
        for game in registry.games() {
            add_candidate(report, game, InstallSource::Gryphlink, games, None);
        }
    }
}
