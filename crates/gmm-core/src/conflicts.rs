// SPDX-License-Identifier: MIT OR Apache-2.0

//! Conflict detection (docs/architecture.md, "Conflict detection").
//!
//! Two enabled mods **conflict** when both replace the same resource: an override section
//! with the same `hash` (and the same `match_first_index`, when both set one) that replaces
//! geometry or textures. Overrides that only watch a hash (set a variable, run a command
//! list) don't count. Mods whose `namespace` the game's manifest lists under
//! `[conflicts] ignore_namespaces` are shared libraries and never reported. Files and
//! folders 3DMigoto skips (`DISABLED*`) aren't read.
//!
//! The index is rebuilt on every call; caching it by folder and modification time is a
//! later optimisation. Conflicts are advisory: GMM never edits a mod's ini.

use std::collections::{BTreeMap, BTreeSet};

use gmm_adapters::GameManifest;
use gmm_db::Db;
use gmm_fsguard::{Guard, RootId, ScopedPath};
use gmm_migoto::{MAX_INI_BYTES, Override, parse};
use serde::Serialize;

use crate::error::CoreError;
use crate::install::InstalledFolder;
use crate::library::{DiskState, library};
use crate::names::{GMM_DIR, on_disk_name};
use crate::workspace::Workspace;

/// Stop reading a mod after this many files; its index is then marked incomplete.
pub(crate) const MAX_FILES_PER_MOD: usize = 20_000;

/// A mod in a conflict report.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModRef {
    /// Installed by GMM (id from `gmm list`).
    Managed { id: i64, folder: String },
    /// Anything else in `Mods/`, by path relative to it.
    Unmanaged { path: String },
}

impl ModRef {
    pub fn label(&self) -> String {
        match self {
            Self::Managed { id, folder } => format!("#{id} {folder}"),
            Self::Unmanaged { path } => format!("{path} (not installed by GMM)"),
        }
    }
}

/// Two mods that replace the same things.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Conflict {
    pub a: ModRef,
    pub b: ModRef,
    /// The hashes both replace, sorted.
    pub hashes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConflictReport {
    pub game: String,
    /// Enabled mods whose ini files were read.
    pub mods_checked: usize,
    pub conflicts: Vec<Conflict>,
    /// Mods skipped as shared libraries (by namespace).
    pub libraries: Vec<ModRef>,
    /// Problems that make the report less complete (unreadable files, very large mods).
    pub notes: Vec<String>,
}

struct Unit {
    mod_ref: ModRef,
    path: ScopedPath,
    is_dir: bool,
}

struct Indexed {
    mod_ref: ModRef,
    overrides: Vec<Override>,
    library: bool,
}

/// Every conflict between the game's enabled mods (managed and unmanaged).
pub async fn conflicts(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
) -> Result<ConflictReport, CoreError> {
    let (units, _) = enabled_units(db, ws, mods, game, Include::Nothing).await?;
    Ok(check(ws.guard(), game, units, None))
}

/// What a managed mod conflicts with among the other enabled mods, whether or not it is
/// enabled itself. Used before enabling a mod, and right after installing one.
pub async fn conflicts_for(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    id: i64,
) -> Result<ConflictReport, CoreError> {
    let (units, focus) = enabled_units(db, ws, mods, game, Include::Managed(id)).await?;
    let Some(focus) = focus else {
        return Err(CoreError::NoSuchMod {
            id,
            game: game.game.name.clone(),
        });
    };
    Ok(check(ws.guard(), game, units, Some(focus)))
}

/// What freshly installed mods (which start enabled) clash with, for one warning after
/// [`crate::install`]. Both sides stay on.
pub async fn conflicts_after_install(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    installed: &[InstalledFolder],
) -> Result<Vec<Conflict>, CoreError> {
    let mut found = Vec::new();
    for folder in installed {
        found.extend(
            conflicts_for(db, ws, mods, game, folder.id)
                .await?
                .conflicts,
        );
    }
    found.dedup();
    Ok(found)
}

/// A mod to check even though it is switched off (it's about to be switched on).
#[derive(Clone, Copy)]
enum Include<'a> {
    Nothing,
    Managed(i64),
    /// Path relative to `Mods/`, exactly as listed.
    Unmanaged(&'a str),
}

/// What a mod GMM didn't install (path from `gmm list`, on or off) conflicts with among the
/// other enabled mods. Used before switching it on.
pub async fn conflicts_for_unmanaged(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    path: &str,
) -> Result<ConflictReport, CoreError> {
    let (units, focus) = enabled_units(db, ws, mods, game, Include::Unmanaged(path)).await?;
    let Some(focus) = focus else {
        return Err(CoreError::NoSuchUnmanaged {
            query: path.to_owned(),
            game: game.game.name.clone(),
        });
    };
    Ok(check(ws.guard(), game, units, Some(focus)))
}

/// The enabled mods as folders (or loose ini files) to read. With `include`, that managed
/// mod is added even when disabled, and its position is returned.
async fn enabled_units(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    include: Include<'_>,
) -> Result<(Vec<Unit>, Option<usize>), CoreError> {
    let guard = ws.guard();
    let lib = library(db, ws, mods, game).await?;
    let gmm_dir = guard.path(mods, GMM_DIR)?;
    let mut units = Vec::new();
    let mut focus = None;
    for m in &lib.managed {
        let wanted = matches!(include, Include::Managed(id) if id == m.row.id);
        let enabled = match m.disk {
            DiskState::Enabled => true,
            DiskState::Disabled if wanted => false,
            _ => continue,
        };
        if wanted {
            focus = Some(units.len());
        }
        units.push(Unit {
            mod_ref: ModRef::Managed {
                id: m.row.id,
                folder: m.row.folder_name.clone(),
            },
            path: gmm_dir.join(&on_disk_name(&m.row.folder_name, enabled))?,
            is_dir: true,
        });
    }
    for u in &lib.unmanaged {
        let wanted = matches!(include, Include::Unmanaged(path) if path == u.path);
        if !u.enabled && !wanted {
            continue;
        }
        if wanted {
            focus = Some(units.len());
        }
        units.push(Unit {
            mod_ref: ModRef::Unmanaged {
                path: u.path.clone(),
            },
            path: guard.path(mods, &u.path)?,
            is_dir: u.is_dir,
        });
    }
    Ok((units, focus))
}

fn check(
    guard: &Guard,
    game: &GameManifest,
    units: Vec<Unit>,
    focus: Option<usize>,
) -> ConflictReport {
    let mut report = ConflictReport {
        game: game.id().to_owned(),
        mods_checked: units.len(),
        ..ConflictReport::default()
    };
    let ignore: Vec<String> = game
        .conflicts
        .ignore_namespaces
        .iter()
        .map(|n| n.to_ascii_lowercase())
        .collect();
    let indexed: Vec<Indexed> = units
        .into_iter()
        .map(|u| index_unit(guard, u, &ignore, &mut report.notes))
        .collect();
    report.libraries = indexed
        .iter()
        .filter(|i| i.library)
        .map(|i| i.mod_ref.clone())
        .collect();

    // hash -> (mod position, override) for every replacing override of a non-library mod.
    let mut by_hash: BTreeMap<&str, Vec<(usize, &Override)>> = BTreeMap::new();
    for (pos, m) in indexed.iter().enumerate().filter(|(_, m)| !m.library) {
        for o in &m.overrides {
            by_hash.entry(o.hash.as_str()).or_default().push((pos, o));
        }
    }
    let mut pairs: BTreeMap<(usize, usize), BTreeSet<String>> = BTreeMap::new();
    for (hash, hits) in &by_hash {
        for (i, (pa, oa)) in hits.iter().enumerate() {
            for (pb, ob) in &hits[i + 1..] {
                if pa == pb || !oa.overlaps(ob) {
                    continue;
                }
                let key = ((*pa).min(*pb), (*pa).max(*pb));
                if focus.is_some_and(|f| key.0 != f && key.1 != f) {
                    continue;
                }
                pairs.entry(key).or_default().insert((*hash).to_owned());
            }
        }
    }
    report.conflicts = pairs
        .into_iter()
        .map(|((a, b), hashes)| Conflict {
            a: indexed[a].mod_ref.clone(),
            b: indexed[b].mod_ref.clone(),
            hashes: hashes.into_iter().collect(),
        })
        .collect();
    report
}

fn index_unit(guard: &Guard, unit: Unit, ignore: &[String], notes: &mut Vec<String>) -> Indexed {
    let mut files = Vec::new();
    if unit.is_dir {
        let mut stack = vec![unit.path.clone()];
        let mut seen = 0usize;
        'walk: while let Some(dir) = stack.pop() {
            let Ok(entries) = guard.read_dir(&dir) else {
                notes.push(format!(
                    "couldn't read a folder of {}",
                    unit.mod_ref.label()
                ));
                continue;
            };
            for entry in entries {
                seen += 1;
                if seen > MAX_FILES_PER_MOD {
                    notes.push(format!(
                        "{} has more than {MAX_FILES_PER_MOD} files; only part of it was checked",
                        unit.mod_ref.label()
                    ));
                    break 'walk;
                }
                // 3DMigoto skips DISABLED* names; links could point anywhere.
                if entry.is_symlink || entry.name.to_ascii_lowercase().starts_with("disabled") {
                    continue;
                }
                if entry.is_dir {
                    stack.push(entry.path);
                } else if is_ini(&entry.name) {
                    files.push((entry.path, entry.len));
                }
            }
        }
    } else {
        let len = guard.metadata(&unit.path).map(|m| m.len()).unwrap_or(0);
        files.push((unit.path.clone(), len));
    }

    let mut overrides = Vec::new();
    let mut library = false;
    for (path, len) in files {
        if len > MAX_INI_BYTES as u64 {
            notes.push(format!(
                "skipped a very large ini in {}",
                unit.mod_ref.label()
            ));
            continue;
        }
        let Ok(bytes) = guard.read(&path) else {
            notes.push(format!("couldn't read an ini in {}", unit.mod_ref.label()));
            continue;
        };
        let ini = parse(&String::from_utf8_lossy(&bytes));
        if ini
            .namespace
            .as_deref()
            .is_some_and(|n| ignore.contains(&n.to_ascii_lowercase()))
        {
            library = true;
        }
        overrides.extend(ini.overrides.into_iter().filter(|o| o.replaces.any()));
    }
    Indexed {
        mod_ref: unit.mod_ref,
        overrides,
        library,
    }
}

fn is_ini(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".ini")
}
