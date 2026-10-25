// SPDX-License-Identifier: MIT OR Apache-2.0

//! Storage accounting (docs/02-architecture.md, "Storage accounting"): how much space each
//! mod, each game's `Mods/` folder and GMM's cache take, and how much is left on the drive.
//! Sizes are measured on disk each time; a managed mod's recorded size is refreshed when it
//! changed.

use gmm_adapters::GameManifest;
use gmm_db::Db;
use gmm_fsguard::{Guard, RootId, ScopedPath};
use serde::Serialize;

use crate::conflicts::ModRef;
use crate::error::CoreError;
use crate::library::{DiskState, Remover, library};
use crate::names::{GMM_DIR, STAGING_DIR, on_disk_name};
use crate::workspace::Workspace;

/// Stop measuring a folder after this many entries (sizes are then "at least").
const MAX_WALK_ENTRIES: usize = 500_000;

/// Size of a folder tree, without following links.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Size {
    pub bytes: u64,
    pub files: u64,
    /// The walk stopped early; the real size is larger.
    pub truncated: bool,
}

impl Size {
    fn add(&mut self, other: Size) {
        self.bytes += other.bytes;
        self.files += other.files;
        self.truncated |= other.truncated;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModUsage {
    pub mod_ref: ModRef,
    pub enabled: bool,
    pub size: Size,
}

/// One game's `Mods/` folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GameStorage {
    pub game: String,
    /// Largest first.
    pub managed: Vec<ModUsage>,
    pub unmanaged: Vec<ModUsage>,
    pub managed_total: Size,
    pub unmanaged_total: Size,
    /// Loose files in `Mods/` and leftovers in GMM's staging folder.
    pub other: Size,
    /// Free space on the drive that holds `Mods/`.
    pub free_bytes: Option<u64>,
}

/// GMM's cache folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CacheStorage {
    /// Archives in `downloads/` (including half-finished downloads).
    pub downloads: Size,
    /// Leftover extraction folders (normally empty).
    pub extract: Size,
    /// GameBanana responses kept in the database.
    pub http_entries: u64,
    pub http_bytes: u64,
    pub free_bytes: Option<u64>,
}

/// Measures one game's `Mods/` folder.
pub async fn game_storage(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
) -> Result<GameStorage, CoreError> {
    let guard = ws.guard();
    let lib = library(db, ws, mods, game).await?;
    let gmm_dir = guard.path(mods, GMM_DIR)?;

    let mut managed = Vec::new();
    let mut managed_total = Size::default();
    for m in &lib.managed {
        let enabled = match m.disk {
            DiskState::Enabled => true,
            DiskState::Disabled => false,
            DiskState::Missing | DiskState::Ambiguous => continue,
        };
        let size = tree_size(
            guard,
            &gmm_dir.join(&on_disk_name(&m.row.folder_name, enabled))?,
        );
        if !size.truncated && size.bytes != m.row.size_bytes {
            db.set_mod_size(m.row.id, size.bytes).await?;
        }
        managed_total.add(size);
        managed.push(ModUsage {
            mod_ref: ModRef::Managed {
                id: m.row.id,
                folder: m.row.folder_name.clone(),
            },
            enabled,
            size,
        });
    }

    let mut unmanaged = Vec::new();
    let mut unmanaged_total = Size::default();
    for u in &lib.unmanaged {
        let path = guard.path(mods, &u.path)?;
        let size = if u.is_dir {
            tree_size(guard, &path)
        } else {
            file_size(guard, &path)
        };
        unmanaged_total.add(size);
        unmanaged.push(ModUsage {
            mod_ref: ModRef::Unmanaged {
                path: u.path.clone(),
            },
            enabled: u.enabled,
            size,
        });
    }

    // Everything else: loose non-mod files in Mods/, and the staging folder.
    let mut other = Size::default();
    let root = guard.path(mods, "")?;
    for entry in guard.read_dir(&root).unwrap_or_default() {
        let is_mod = entry.is_dir || entry.name.to_ascii_lowercase().ends_with(".ini");
        if !is_mod && !entry.is_symlink {
            other.add(Size {
                bytes: entry.len,
                files: 1,
                truncated: false,
            });
        }
    }
    other.add(tree_size(guard, &gmm_dir.join(STAGING_DIR)?));

    managed.sort_by_key(|m| std::cmp::Reverse(m.size.bytes));
    unmanaged.sort_by_key(|m| std::cmp::Reverse(m.size.bytes));
    Ok(GameStorage {
        game: game.id().to_owned(),
        managed,
        unmanaged,
        managed_total,
        unmanaged_total,
        other,
        free_bytes: guard.available_space(&root).ok(),
    })
}

/// Measures GMM's cache.
pub async fn cache_storage(db: &Db, ws: &Workspace) -> Result<CacheStorage, CoreError> {
    let guard = ws.guard();
    let cache = ws.cache_root();
    let (http_entries, http_bytes) = db.http_cache_stats().await?;
    Ok(CacheStorage {
        downloads: tree_size(guard, &guard.path(cache, "downloads")?),
        extract: tree_size(guard, &guard.path(cache, "extract")?),
        http_entries,
        http_bytes,
        free_bytes: guard.available_space(&guard.path(cache, "")?).ok(),
    })
}

/// Outcome of [`clear_downloads`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClearedCache {
    pub archives: u64,
    pub bytes: u64,
}

/// Empties the download cache: every archive (and partial download) is deleted for good,
/// and the records are dropped. Installed mods are not affected; reinstalling one
/// downloads it again.
pub async fn clear_downloads(
    db: &Db,
    ws: &Workspace,
    remover: &dyn Remover,
) -> Result<ClearedCache, CoreError> {
    let guard = ws.guard();
    let downloads = guard.path(ws.cache_root(), "downloads")?;
    let mut cleared = ClearedCache::default();
    for entry in guard.read_dir(&downloads)? {
        if entry.is_symlink {
            continue;
        }
        let size = if entry.is_dir {
            tree_size(guard, &entry.path)
        } else {
            file_size(guard, &entry.path)
        };
        remover.remove(guard, &entry.path)?;
        cleared.archives += 1;
        cleared.bytes += size.bytes;
    }
    for d in db.cached_downloads().await? {
        db.remove_download(&d.sha256).await?;
    }
    Ok(cleared)
}

fn file_size(guard: &Guard, path: &ScopedPath) -> Size {
    Size {
        bytes: guard.metadata(path).map(|m| m.len()).unwrap_or(0),
        files: 1,
        truncated: false,
    }
}

/// Total size of the files below `dir` (0 if it doesn't exist).
pub(crate) fn tree_size(guard: &Guard, dir: &ScopedPath) -> Size {
    let mut size = Size::default();
    let mut stack = vec![dir.clone()];
    let mut seen = 0usize;
    while let Some(next) = stack.pop() {
        let Ok(entries) = guard.read_dir(&next) else {
            continue;
        };
        for entry in entries {
            seen += 1;
            if seen > MAX_WALK_ENTRIES {
                size.truncated = true;
                return size;
            }
            // Don't follow links: sizes would double, and fsguard refuses escapes anyway.
            if entry.is_symlink {
                continue;
            }
            if entry.is_dir {
                stack.push(entry.path);
            } else {
                size.bytes += entry.len;
                size.files += 1;
            }
        }
    }
    size
}

/// Size of one entry of the library's unmanaged list (folder or loose file).
pub fn unmanaged_size(
    ws: &Workspace,
    mods: RootId,
    entry: &crate::library::UnmanagedMod,
) -> Result<Size, CoreError> {
    let guard = ws.guard();
    let path = guard.path(mods, &entry.path)?;
    Ok(if entry.is_dir {
        tree_size(guard, &path)
    } else {
        file_size(guard, &path)
    })
}
