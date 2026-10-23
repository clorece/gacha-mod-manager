// SPDX-License-Identifier: MIT OR Apache-2.0

//! Checks a manifest can't express in its types. Paths in manifests become places GMM looks
//! on disk, so they get the same validation as any other path (gmm-fsguard's `RelPath`).

use std::collections::HashSet;

use gmm_fsguard::RelPath;

use crate::manifest::GameManifest;

/// Returns every problem found, so one run of the tests lists them all.
pub(crate) fn problems(m: &GameManifest) -> Vec<String> {
    let mut out = Vec::new();
    let mut fail = |msg: String| out.push(msg);

    let id = &m.game.id;
    let id_ok = (2..=16).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !id_ok {
        fail(format!(
            "game.id {id:?} must be 2-16 chars of a-z, 0-9 or '-', starting with a letter"
        ));
    }
    for (field, value) in [
        ("game.name", &m.game.name),
        ("game.short_name", &m.game.short_name),
        ("game.risk_notice", &m.game.risk_notice),
    ] {
        if value.trim().is_empty() {
            fail(format!("{field} must not be empty"));
        }
    }

    if m.gamebanana.game_id == 0 {
        fail("gamebanana.game_id must be set".to_owned());
    }
    if m.gamebanana.root_categories.is_empty() {
        fail("gamebanana.root_categories must list at least one category".to_owned());
    }
    let mut category_ids = HashSet::new();
    for c in &m.gamebanana.root_categories {
        if c.id == 0 || !category_ids.insert(c.id) {
            fail(format!(
                "gamebanana root category id {} is zero or repeated",
                c.id
            ));
        }
    }

    if !is_exe_name(&m.xxmi.d3dx_target) {
        fail(format!(
            "xxmi.d3dx_target {:?} must be a single .exe file name",
            m.xxmi.d3dx_target
        ));
    }

    if m.detect.exe_names.is_empty() {
        fail("detect.exe_names must list at least one executable".to_owned());
    }
    for name in &m.detect.exe_names {
        if !is_exe_name(name) {
            fail(format!(
                "detect.exe_names entry {name:?} must be a single .exe file name"
            ));
        }
    }
    for child in &m.detect.required_children {
        if !RelPath::parse(child).is_ok_and(|p| !p.is_root()) {
            fail(format!(
                "detect.required_children entry {child:?} is not a safe relative path"
            ));
        }
    }
    for name in &m.detect.folder_names {
        if !is_single_name(name) {
            fail(format!(
                "detect.folder_names entry {name:?} must be a single folder name"
            ));
        }
    }
    if let Some(steam) = &m.detect.steam {
        if steam.appid == 0 {
            fail("detect.steam.appid must be set".to_owned());
        }
        if RelPath::parse(&steam.subpath).is_err() {
            fail(format!(
                "detect.steam.subpath {:?} is not a safe relative path",
                steam.subpath
            ));
        }
    }
    if let Some(epic) = &m.detect.epic
        && (epic.display_names.is_empty() || epic.display_names.iter().any(|n| n.trim().is_empty()))
    {
        fail("detect.epic.display_names must list non-empty names".to_owned());
    }

    if m.mod_kinds.is_empty() {
        fail("at least one [[mod_kind]] is required".to_owned());
    }
    let mut kinds = HashSet::new();
    for kind in &m.mod_kinds {
        if kind.id != "migoto" {
            fail(format!(
                "unknown mod_kind {:?} (supported: migoto)",
                kind.id
            ));
        }
        if !kinds.insert(kind.id.as_str()) {
            fail(format!("mod_kind {:?} is listed twice", kind.id));
        }
        if kind.ini_markers.is_empty() || kind.ini_markers.iter().any(|s| s.trim().is_empty()) {
            fail(format!(
                "mod_kind {:?} needs non-empty ini_markers",
                kind.id
            ));
        }
    }
    out
}

fn is_single_name(name: &str) -> bool {
    RelPath::parse(name).is_ok_and(|p| p.depth() == 1 && !name.contains(['/', '\\']))
}

fn is_exe_name(name: &str) -> bool {
    is_single_name(name) && name.to_ascii_lowercase().ends_with(".exe")
}
