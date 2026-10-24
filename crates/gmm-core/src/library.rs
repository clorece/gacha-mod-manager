// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mods in one game's `Mods/` folder: GMM's own (from the database, checked against the
//! disk) and everything else (unmanaged, i.e. installed by hand), plus switching mods on and
//! off and uninstalling them.
//!
//! The disk is the truth for on/off: if someone renamed a folder by hand, the database is
//! updated to match. A managed mod whose folder is gone is reported as missing, never
//! silently forgotten. Unmanaged mods are only touched when the user names one: on/off is the
//! same `DISABLED ` rename, and uninstalling moves it to the Recycle Bin after the caller has
//! shown exactly what goes and got a yes (security rule R8).

use gmm_adapters::GameManifest;
use gmm_db::{Db, InstalledMod};
use gmm_fsguard::{Guard, RootId, ScopedPath};
use serde::Serialize;

use crate::error::CoreError;
use crate::names::{DISABLED_PREFIX, GMM_DIR, STAGING_DIR, on_disk_name};
use crate::workspace::Workspace;

/// Where a managed mod's folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum DiskState {
    Enabled,
    Disabled,
    /// Neither name exists (deleted or moved outside GMM, or another XXMI folder).
    Missing,
    /// Both the enabled and the `DISABLED` name exist; GMM won't guess which is its own.
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedMod {
    pub row: InstalledMod,
    pub disk: DiskState,
}

/// A folder (or loose `.ini`) in `Mods/` that GMM didn't install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UnmanagedMod {
    pub name: String,
    /// Relative to `Mods/`, with `/`: `Phoenix`, or `GMM/Something` for a folder inside GMM's
    /// own folder that the database doesn't know.
    pub path: String,
    pub enabled: bool,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    pub game: String,
    pub managed: Vec<ManagedMod>,
    pub unmanaged: Vec<UnmanagedMod>,
}

/// Lists one game's mods. Managed mods whose on/off state was changed by hand get their
/// database row corrected.
pub async fn library(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
) -> Result<Library, CoreError> {
    let guard = ws.guard();
    let rows = db.installed_mods(Some(game.id())).await?;
    let gmm_dir = guard.path(mods, GMM_DIR)?;
    let mut managed = Vec::with_capacity(rows.len());
    for mut row in rows {
        let disk = disk_state(guard, &gmm_dir, &row.folder_name)?;
        let on = match disk {
            DiskState::Enabled => Some(true),
            DiskState::Disabled => Some(false),
            _ => None,
        };
        if let Some(on) = on
            && on != row.enabled
        {
            db.set_mod_enabled(row.id, on).await?;
            row.enabled = on;
        }
        managed.push(ManagedMod { row, disk });
    }

    let mut unmanaged = Vec::new();
    let root = guard.path(mods, "")?;
    for entry in guard.read_dir(&root).unwrap_or_default() {
        if entry.name.eq_ignore_ascii_case(GMM_DIR) && entry.is_dir {
            continue;
        }
        if let Some(m) = unmanaged_entry(&entry.name, &entry.name, entry.is_dir) {
            unmanaged.push(m);
        }
    }
    for entry in guard.read_dir(&gmm_dir).unwrap_or_default() {
        if entry.name.eq_ignore_ascii_case(STAGING_DIR) {
            continue;
        }
        let base = entry
            .name
            .strip_prefix(DISABLED_PREFIX)
            .unwrap_or(&entry.name);
        let known = managed
            .iter()
            .any(|m| m.row.folder_name.eq_ignore_ascii_case(base));
        if known {
            continue;
        }
        let path = format!("{GMM_DIR}/{}", entry.name);
        if let Some(m) = unmanaged_entry(&entry.name, &path, entry.is_dir) {
            unmanaged.push(m);
        }
    }
    Ok(Library {
        game: game.id().to_owned(),
        managed,
        unmanaged,
    })
}

/// Folders, and loose `.ini` files (3DMigoto loads those too). Other loose files and hidden
/// names aren't mods.
fn unmanaged_entry(name: &str, path: &str, is_dir: bool) -> Option<UnmanagedMod> {
    if name.starts_with('.') {
        return None;
    }
    if !is_dir && !name.to_ascii_lowercase().ends_with(".ini") {
        return None;
    }
    Some(UnmanagedMod {
        name: name.to_owned(),
        path: path.to_owned(),
        enabled: !name.to_ascii_lowercase().starts_with("disabled"),
        is_dir,
    })
}

fn disk_state(guard: &Guard, gmm_dir: &ScopedPath, folder: &str) -> Result<DiskState, CoreError> {
    let on = guard.exists(&gmm_dir.join(&on_disk_name(folder, true))?)?;
    let off = guard.exists(&gmm_dir.join(&on_disk_name(folder, false))?)?;
    Ok(match (on, off) {
        (true, false) => DiskState::Enabled,
        (false, true) => DiskState::Disabled,
        (false, false) => DiskState::Missing,
        (true, true) => DiskState::Ambiguous,
    })
}

async fn managed_row(db: &Db, game: &GameManifest, id: i64) -> Result<InstalledMod, CoreError> {
    db.installed_mod(id)
        .await?
        .filter(|m| m.game_id == game.id())
        .ok_or(CoreError::NoSuchMod {
            id,
            game: game.game.name.clone(),
        })
}

/// Outcome of [`set_enabled`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ToggleOutcome {
    pub id: i64,
    pub folder_name: String,
    pub enabled: bool,
    /// False when the mod already was in that state.
    pub changed: bool,
}

/// Switches a managed mod on or off by renaming its folder with or without XXMI's
/// `DISABLED ` prefix. Nothing else about the folder changes.
pub async fn set_enabled(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    id: i64,
    enabled: bool,
) -> Result<ToggleOutcome, CoreError> {
    let guard = ws.guard();
    let row = managed_row(db, game, id).await?;
    let gmm_dir = guard.path(mods, GMM_DIR)?;
    let outcome = |changed| ToggleOutcome {
        id,
        folder_name: row.folder_name.clone(),
        enabled,
        changed,
    };
    let current = match disk_state(guard, &gmm_dir, &row.folder_name)? {
        DiskState::Enabled => true,
        DiskState::Disabled => false,
        DiskState::Missing => return Err(CoreError::ModFolderMissing(row.folder_name)),
        DiskState::Ambiguous => return Err(CoreError::ModFolderAmbiguous(row.folder_name)),
    };
    if current == enabled {
        if row.enabled != enabled {
            db.set_mod_enabled(id, enabled).await?;
        }
        return Ok(outcome(false));
    }
    let from = gmm_dir.join(&on_disk_name(&row.folder_name, current))?;
    let to = gmm_dir.join(&on_disk_name(&row.folder_name, enabled))?;
    guard.rename(&from, &to)?;
    if let Err(e) = db.set_mod_enabled(id, enabled).await {
        // Keep disk and database in step: undo the rename.
        let _ = guard.rename(&to, &from);
        return Err(e.into());
    }
    Ok(outcome(true))
}

/// How GMM deletes a mod folder. The app always uses [`RecycleBin`]; tests use a fake so
/// they don't fill the real Recycle Bin.
pub trait Trash: Send + Sync {
    fn trash(&self, guard: &Guard, path: &ScopedPath) -> Result<(), CoreError>;
}

/// Moves folders to the Recycle Bin (security rule R8).
#[derive(Debug, Default, Clone, Copy)]
pub struct RecycleBin;

impl Trash for RecycleBin {
    fn trash(&self, guard: &Guard, path: &ScopedPath) -> Result<(), CoreError> {
        Ok(guard.move_to_trash(path)?)
    }
}

/// Outcome of [`uninstall`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UninstallOutcome {
    pub id: i64,
    pub folder_name: String,
    /// False when the folder was already gone and only the record was removed.
    pub trashed: bool,
}

/// Moves a managed mod's folder to the Recycle Bin and forgets it. The archive stays in the
/// download cache.
pub async fn uninstall(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    id: i64,
    trash: &dyn Trash,
) -> Result<UninstallOutcome, CoreError> {
    let guard = ws.guard();
    let row = managed_row(db, game, id).await?;
    let gmm_dir = guard.path(mods, GMM_DIR)?;
    let trashed = match disk_state(guard, &gmm_dir, &row.folder_name)? {
        DiskState::Enabled | DiskState::Disabled => {
            let enabled = guard.exists(&gmm_dir.join(&row.folder_name)?)?;
            let path = gmm_dir.join(&on_disk_name(&row.folder_name, enabled))?;
            trash.trash(guard, &path)?;
            true
        }
        DiskState::Missing => false,
        DiskState::Ambiguous => return Err(CoreError::ModFolderAmbiguous(row.folder_name)),
    };
    db.delete_installed_mod(id).await?;
    Ok(UninstallOutcome {
        id,
        folder_name: row.folder_name,
        trashed,
    })
}

/// Unmanaged mods matching what the user typed: the path from `gmm list` (`GMM/Phoenix`,
/// `Phoenix`), or just the name, with or without a `DISABLED` prefix. Case-insensitive.
pub fn find_unmanaged<'a>(lib: &'a Library, query: &str) -> Vec<&'a UnmanagedMod> {
    let query = query.trim().replace('\\', "/");
    let wanted = base_name(&query).to_ascii_lowercase();
    // A path with a folder names exactly one entry. A bare name may match several (`Dup`
    // and `GMM/Dup`); the caller then asks for the full path instead of guessing.
    if query.contains('/') {
        return lib
            .unmanaged
            .iter()
            .filter(|u| u.path.eq_ignore_ascii_case(&query))
            .collect();
    }
    lib.unmanaged
        .iter()
        .filter(|u| base_name(&u.name).eq_ignore_ascii_case(&wanted))
        .collect()
}

/// `name` without its `DISABLED` prefix (and the separator after it), and without any folder.
fn base_name(name: &str) -> &str {
    let name = name.rsplit('/').next().unwrap_or(name);
    match (name.get(..8), name.get(8..)) {
        (Some(head), Some(rest)) if head.eq_ignore_ascii_case("disabled") => {
            rest.trim_start_matches([' ', '_', '-'])
        }
        _ => name,
    }
}

/// The one unmanaged mod `query` names in this game.
pub fn one_unmanaged<'a>(
    lib: &'a Library,
    game: &GameManifest,
    query: &str,
) -> Result<&'a UnmanagedMod, CoreError> {
    match find_unmanaged(lib, query).as_slice() {
        [only] => Ok(only),
        [] => Err(CoreError::NoSuchUnmanaged {
            query: query.to_owned(),
            game: game.game.name.clone(),
        }),
        many => Err(CoreError::AmbiguousUnmanaged {
            query: query.to_owned(),
            matches: many.iter().map(|u| u.path.clone()).collect(),
        }),
    }
}

/// Outcome of [`set_unmanaged_enabled`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UnmanagedToggle {
    /// Relative to `Mods/`, before and after.
    pub from: String,
    pub to: String,
    pub enabled: bool,
    /// False when the mod already was in that state.
    pub changed: bool,
}

/// Switches a mod GMM didn't install on or off, by the same rename XXMI uses: adding
/// `DISABLED ` in front of its name, or removing whatever `DISABLED` prefix it has. Nothing
/// inside the folder changes. 3DMigoto saves in-game toggle choices by path, so the mod's
/// saved choices start fresh afterwards.
pub async fn set_unmanaged_enabled(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    query: &str,
    enabled: bool,
) -> Result<UnmanagedToggle, CoreError> {
    let lib = library(db, ws, mods, game).await?;
    let entry = one_unmanaged(&lib, game, query)?;
    rename_unmanaged(ws, mods, entry, enabled)
}

/// [`set_unmanaged_enabled`] for the exact path from the listing (what the app passes), so
/// a top-level `Dup` and `GMM/Dup` are never confused.
pub async fn set_unmanaged_enabled_at(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    path: &str,
    enabled: bool,
) -> Result<UnmanagedToggle, CoreError> {
    let lib = library(db, ws, mods, game).await?;
    let entry = listed_unmanaged(&lib, game, path)?;
    rename_unmanaged(ws, mods, entry, enabled)
}

/// The unmanaged mod listed at exactly `path` (relative to `Mods/`, as [`library`] gives it).
pub fn listed_unmanaged<'a>(
    lib: &'a Library,
    game: &GameManifest,
    path: &str,
) -> Result<&'a UnmanagedMod, CoreError> {
    lib.unmanaged
        .iter()
        .find(|u| u.path == path)
        .ok_or_else(|| CoreError::NoSuchUnmanaged {
            query: path.to_owned(),
            game: game.game.name.clone(),
        })
}

fn rename_unmanaged(
    ws: &Workspace,
    mods: RootId,
    entry: &UnmanagedMod,
    enabled: bool,
) -> Result<UnmanagedToggle, CoreError> {
    let guard = ws.guard();
    if entry.enabled == enabled {
        return Ok(UnmanagedToggle {
            from: entry.path.clone(),
            to: entry.path.clone(),
            enabled,
            changed: false,
        });
    }
    let new_name = if enabled {
        base_name(&entry.name).to_owned()
    } else {
        format!("{DISABLED_PREFIX}{}", entry.name)
    };
    if new_name.is_empty() {
        return Err(CoreError::Io(format!(
            "{:?} has no name besides DISABLED; rename it by hand",
            entry.path
        )));
    }
    let parent = entry
        .path
        .rsplit_once('/')
        .map(|(dir, _)| format!("{dir}/"))
        .unwrap_or_default();
    let to = format!("{parent}{new_name}");
    let from_path = guard.path(mods, &entry.path)?;
    let to_path = guard.path(mods, &to)?;
    if guard.exists(&to_path)? {
        return Err(CoreError::FolderTaken { folder: to });
    }
    guard.rename(&from_path, &to_path)?;
    Ok(UnmanagedToggle {
        from: entry.path.clone(),
        to,
        enabled,
        changed: true,
    })
}

/// Moves a mod GMM didn't install to the Recycle Bin. The caller must have shown the user
/// exactly this path and got a yes (security rule R8); GMM never does this on its own.
pub async fn uninstall_unmanaged(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    path: &str,
    trash: &dyn Trash,
) -> Result<String, CoreError> {
    let guard = ws.guard();
    let lib = library(db, ws, mods, game).await?;
    // Only an exact path from the listing: the confirmation showed this path, nothing looser.
    let entry = listed_unmanaged(&lib, game, path)?;
    trash.trash(guard, &guard.path(mods, &entry.path)?)?;
    Ok(entry.path.clone())
}
