// SPDX-License-Identifier: MIT OR Apache-2.0

//! A mod's key bindings (Phase 6): the `[Key*]` sections of its `.ini` files, which say which
//! keys switch its parts in game (`key = ...`, `back = ...`, `type = cycle | toggle | hold`).
//! Read-only: GMM shows them and never edits a mod's files. Files are walked like the
//! conflict check does (`DISABLED*` names skipped like 3DMigoto skips them, links never
//! followed, the same file and size limits).

use gmm_adapters::GameManifest;
use gmm_db::Db;
use gmm_fsguard::{Guard, RootId, ScopedPath};
use gmm_migoto::parse;
use serde::Serialize;

use crate::analyze::MAX_INI_BYTES;
use crate::conflicts::MAX_FILES_PER_MOD;
use crate::error::CoreError;
use crate::library::{ModFolder, mod_location};
use crate::workspace::Workspace;

/// One `[Key*]` section of a mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModKeyBinding {
    /// The `.ini` it's in, relative to the mod's folder (`/` separators).
    pub ini: String,
    /// The section name without `Key` (`[KeySwap]` -> `Swap`), else as written.
    pub name: String,
    /// The keys that trigger it, as written (`VK_LEFT`, `no_modifiers ctrl 1`).
    pub keys: Vec<String>,
    /// Keys that step backwards through a cycle.
    pub back: Vec<String>,
    /// `cycle`, `toggle`, `hold` or `activate`, when the section says.
    pub kind: Option<String>,
}

/// What [`mod_keybindings`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KeyBindings {
    pub bindings: Vec<ModKeyBinding>,
    /// Files that couldn't be read or were too large, as text.
    pub notes: Vec<String>,
}

/// The key bindings of one mod (GMM's by id, a hand-installed one by its listed path).
pub async fn mod_keybindings(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    which: ModFolder<'_>,
) -> Result<KeyBindings, CoreError> {
    let guard = ws.guard();
    let (at, is_dir) = mod_location(db, ws, mods, game, which).await?;
    let mut out = KeyBindings::default();
    let files = if is_dir {
        ini_files(guard, &at, &mut out.notes)
    } else {
        vec![(at.clone(), at.file_name().unwrap_or_default().to_owned())]
    };
    for (path, rel) in files {
        let too_big = guard
            .metadata(&path)
            .map(|m| m.len() > MAX_INI_BYTES)
            .unwrap_or(true);
        let bytes = if too_big {
            None
        } else {
            guard.read(&path).ok()
        };
        let Some(bytes) = bytes else {
            out.notes.push(format!("couldn't read {rel}"));
            continue;
        };
        for key in parse(&String::from_utf8_lossy(&bytes)).keys {
            let name = key
                .section
                .strip_prefix("Key")
                .or_else(|| key.section.strip_prefix("key"))
                .filter(|rest| !rest.is_empty())
                .unwrap_or(&key.section)
                .to_owned();
            out.bindings.push(ModKeyBinding {
                ini: rel.clone(),
                name,
                keys: key.keys,
                back: key.back,
                kind: key.kind,
            });
        }
    }
    Ok(out)
}

/// Every `.ini` below `dir`, with its path relative to `dir`, sorted.
pub(crate) fn ini_files(
    guard: &Guard,
    dir: &ScopedPath,
    notes: &mut Vec<String>,
) -> Vec<(ScopedPath, String)> {
    let mut found = Vec::new();
    let mut stack = vec![(dir.clone(), String::new())];
    let mut seen = 0usize;
    'walk: while let Some((folder, prefix)) = stack.pop() {
        let Ok(entries) = guard.read_dir(&folder) else {
            notes.push(format!(
                "couldn't read the folder {}",
                if prefix.is_empty() { "." } else { &prefix }
            ));
            continue;
        };
        for entry in entries {
            seen += 1;
            if seen > MAX_FILES_PER_MOD {
                notes.push(format!(
                    "more than {MAX_FILES_PER_MOD} files; only part was read"
                ));
                break 'walk;
            }
            if entry.is_symlink || entry.name.to_ascii_lowercase().starts_with("disabled") {
                continue;
            }
            let rel = if prefix.is_empty() {
                entry.name.clone()
            } else {
                format!("{prefix}/{}", entry.name)
            };
            if entry.is_dir {
                stack.push((entry.path, rel));
            } else if entry.name.to_ascii_lowercase().ends_with(".ini") {
                found.push((entry.path, rel));
            }
        }
    }
    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
}
