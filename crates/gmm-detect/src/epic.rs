// SPDX-License-Identifier: MIT OR Apache-2.0

//! Epic Games: `%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON). Only
//! `DisplayName` and `InstallLocation` are read. Not yet confirmed on a machine with an Epic
//! install; tested against fixtures.

use std::path::PathBuf;

use gmm_adapters::Registry;
use gmm_fsguard::{Access, Guard};
use serde::Deserialize;

use crate::env::Environment;
use crate::{DetectReport, InstallSource, add_candidate};

const MAX_ITEM_BYTES: u64 = 1024 * 1024;

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "DisplayName", default)]
    display_name: String,
    #[serde(rename = "InstallLocation", default)]
    install_location: String,
}

pub(crate) fn detect(registry: &Registry, env: &dyn Environment, report: &mut DetectReport) {
    let Some(program_data) = env.program_data() else {
        return;
    };
    let dir = program_data
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    let mut guard = Guard::new();
    let Ok(root) = guard.register_root("epic", &dir, Access::ReadOnly) else {
        return; // Epic isn't installed.
    };
    let Ok(entries) = guard.path(root, "").and_then(|top| guard.read_dir(&top)) else {
        return;
    };
    for entry in entries {
        if entry.is_dir
            || entry.len > MAX_ITEM_BYTES
            || !entry.name.to_ascii_lowercase().ends_with(".item")
        {
            continue;
        }
        let Ok(bytes) = guard.read(&entry.path) else {
            continue;
        };
        let Ok(item) = serde_json::from_slice::<Item>(&bytes) else {
            continue;
        };
        for game in registry.games() {
            let Some(epic) = &game.detect.epic else {
                continue;
            };
            if epic
                .display_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case(item.display_name.trim()))
            {
                let folder = PathBuf::from(item.install_location.trim());
                add_candidate(report, game, InstallSource::Epic, &folder, None);
            }
        }
    }
}
