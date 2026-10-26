// SPDX-License-Identifier: MIT OR Apache-2.0

//! One-click update and roll back (Phase 6): moves an installed GameBanana mod to another of
//! its files.
//!
//! Every installed folder of the mod (one per version) is matched to the same version in the
//! other file (by version name; an archive with one mod matches a folder installed from one),
//! copied into the staging folder, and swapped in **under the same folder name and on/off
//! state**: 3DMigoto keys its saved in-game toggles by path, so they survive (02, "Install &
//! toggle model"). The swap stays on one volume: the current folder moves into the staging
//! folder, the new one is renamed into place, and the database row is pointed at the new file
//! (same id). If any of that fails, that folder is swapped back. Afterwards the previous
//! folders are deleted (R8). Their archive stays in the download cache, so rolling
//! back is the same call with the older file and downloads nothing.
//!
//! A version the other file doesn't have is left as it is and reported. Blocked files in the
//! new file are left out, as at install, and listed.

use gmm_adapters::GameManifest;
use gmm_catalog::{CatalogSource, ModFile};
use gmm_db::{Db, FileChange, InstalledMod};
use gmm_fsguard::RootId;
use gmm_net::Client;
use serde::Serialize;

use crate::analyze::Variant;
use crate::error::CoreError;
use crate::install::{InstallEvent, ModMeta, assemble_variant, prepare, variant_findings};
use crate::library::Remover;
use crate::names::{GMM_DIR, STAGING_DIR, on_disk_name};
use crate::stop::StopSignal;
use crate::workspace::{STALE_TEMP, Workspace, remove_stale, unique_id};

/// One folder moved to the other file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChangedFolder {
    pub id: i64,
    pub folder_name: String,
    pub enabled: bool,
    pub files: usize,
    pub bytes: u64,
    /// Blocked files in the new file that were left out.
    pub skipped: Vec<String>,
}

/// Outcome of [`change_file`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileChangeReport {
    pub game: String,
    pub gb_mod_id: u64,
    pub mod_name: String,
    /// The file the mod is on now.
    pub file: ModFile,
    pub changed: Vec<ChangedFolder>,
    /// Folders left as they were: their version isn't in the new file, or they already
    /// were on it.
    pub kept: Vec<String>,
    /// Previous folders that couldn't be deleted (left in the staging
    /// folder, which GMM clears after an hour).
    pub not_removed: Vec<String>,
}

/// The other files of an installed mod that are already in the download cache, newest
/// first: what a roll back can use without downloading. Excludes the files it's on now.
pub async fn rollback_choices(
    db: &Db,
    game: &GameManifest,
    gb_mod_id: u64,
) -> Result<Vec<u64>, CoreError> {
    let current: Vec<u64> = installed_rows(db, game, gb_mod_id)
        .await?
        .iter()
        .filter_map(|r| r.gb_file_id)
        .collect();
    let mut out = Vec::new();
    for cached in db.cached_downloads_for_mod(gb_mod_id).await? {
        if let Some(file) = cached.gb_file_id
            && !current.contains(&file)
            && !out.contains(&file)
        {
            out.push(file);
        }
    }
    Ok(out)
}

async fn installed_rows(
    db: &Db,
    game: &GameManifest,
    gb_mod_id: u64,
) -> Result<Vec<InstalledMod>, CoreError> {
    let rows: Vec<InstalledMod> = db
        .installed_mods_for_gb(gb_mod_id)
        .await?
        .into_iter()
        .filter(|r| r.game_id == game.id())
        .collect();
    if rows.is_empty() {
        return Err(CoreError::NotInstalled {
            gb_mod_id,
            game: game.game.name.clone(),
        });
    }
    Ok(rows)
}

/// The version of the new archive an installed folder becomes: the version of the same
/// name, or for an archive that now holds one mod, that mod when its folder has the name.
fn matching_variant<'a>(row: &InstalledMod, variants: &'a [Variant]) -> Option<&'a Variant> {
    match (row.variant.as_deref(), variants) {
        (None, [only]) => Some(only),
        (None, _) => None,
        (Some(name), [only]) if only.name.is_empty() => only
            .root
            .components()
            .last()
            .filter(|folder| folder.eq_ignore_ascii_case(name))
            .map(|_| only),
        (Some(name), _) => variants.iter().find(|v| v.name.eq_ignore_ascii_case(name)),
    }
}

/// Moves every installed folder of GameBanana mod `gb_mod_id` to `file_id` (or the newest
/// clean file): an update, or with an older cached file, a roll back.
#[allow(clippy::too_many_arguments)]
pub async fn change_file<C: CatalogSource>(
    catalog: &C,
    net: &Client,
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    gb_mod_id: u64,
    file_id: Option<u64>,
    remover: &dyn Remover,
    stop: &StopSignal,
    mut on_event: impl FnMut(InstallEvent),
) -> Result<FileChangeReport, CoreError> {
    let rows = installed_rows(db, game, gb_mod_id).await?;
    let prepared = prepare(
        catalog,
        net,
        db,
        ws,
        game,
        gb_mod_id,
        file_id,
        stop,
        &mut on_event,
    )
    .await?;
    let result = swap_in(db, ws, mods, game, &rows, &prepared, remover, &mut on_event).await;
    prepared.discard(ws);
    result
}

#[allow(clippy::too_many_arguments)]
async fn swap_in(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    rows: &[InstalledMod],
    prepared: &crate::install::Prepared,
    remover: &dyn Remover,
    on_event: &mut impl FnMut(InstallEvent),
) -> Result<FileChangeReport, CoreError> {
    let analysis = &prepared.analysis;
    if analysis.kind.is_none() {
        return Err(CoreError::NotAMod {
            markers: game
                .mod_kinds
                .iter()
                .flat_map(|k| k.ini_markers.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" or "),
        });
    }
    let fetched = &prepared.fetched;
    let meta = ModMeta {
        gb_mod_id: Some(prepared.mod_id),
        listing: prepared.listing.clone(),
        ..ModMeta::from_gamebanana(
            &prepared.mod_name,
            prepared.mod_version.as_deref(),
            &fetched.file,
            &fetched.sha256,
        )
    };
    let mut report = FileChangeReport {
        game: game.id().to_owned(),
        gb_mod_id: prepared.mod_id,
        mod_name: prepared.mod_name.clone(),
        file: fetched.file.clone(),
        changed: Vec::new(),
        kept: Vec::new(),
        not_removed: Vec::new(),
    };

    // Which folders move, to which version. Nothing on disk changes yet.
    let mut plans = Vec::new();
    for row in rows {
        if row.gb_file_id == Some(fetched.file.id) {
            report.kept.push(row.folder_name.clone());
            continue;
        }
        match matching_variant(row, &analysis.variants) {
            Some(variant) => plans.push((row, variant)),
            None => report.kept.push(row.folder_name.clone()),
        }
    }
    if plans.is_empty() {
        return Ok(report);
    }

    let guard = ws.guard();
    let gmm_dir = guard.path(mods, GMM_DIR)?;
    let staging_root = gmm_dir.join(STAGING_DIR)?;
    guard.create_dir_all(&staging_root)?;
    remove_stale(guard, &staging_root, STALE_TEMP);
    let stage = staging_root.join(&unique_id())?;
    guard.create_dir_all(&stage)?;

    // Assemble every new folder first, so a copy failure changes nothing.
    let mut staged = Vec::with_capacity(plans.len());
    for (index, (row, variant)) in plans.into_iter().enumerate() {
        on_event(InstallEvent::Installing {
            folder: row.folder_name.clone(),
        });
        let (blocked, findings) = variant_findings(analysis, variant);
        let dir = stage.join(&format!("new-{index}"))?;
        match assemble_variant(
            guard,
            game,
            &meta,
            analysis,
            variant,
            &row.folder_name,
            &blocked,
            &findings,
            &dir,
        ) {
            Ok(assembled) => staged.push((index, row, dir, assembled)),
            Err(e) => {
                let _ = guard.remove_dir_all(&stage);
                return Err(e);
            }
        }
    }

    // Swap each folder; a failed swap is undone and stops the rest.
    let mut previous = Vec::new();
    let mut failure = None;
    for (index, row, new_dir, assembled) in staged {
        let enabled_name = gmm_dir.join(&row.folder_name)?;
        let disabled_name = gmm_dir.join(&on_disk_name(&row.folder_name, false))?;
        let current = if guard.exists(&enabled_name)? {
            Some((enabled_name, true))
        } else if guard.exists(&disabled_name)? {
            Some((disabled_name, false))
        } else {
            None
        };
        // A folder deleted by hand comes back in the state GMM last knew.
        let enabled = current.as_ref().map_or(row.enabled, |(_, on)| *on);
        let target = gmm_dir.join(&on_disk_name(&row.folder_name, enabled))?;
        let aside = stage.join(&format!("old-{index}"))?;
        if let Some((path, _)) = &current
            && let Err(e) = guard.rename(path, &aside)
        {
            failure = Some(e.into());
            break;
        }
        if let Err(e) = guard.rename(&new_dir, &target) {
            if current.is_some() {
                let _ = guard.rename(&aside, &target);
            }
            failure = Some(e.into());
            break;
        }
        let change = FileChange {
            gb_file_id: assembled.row.gb_file_id,
            version: assembled.row.version.clone(),
            file_date: assembled.row.file_date,
            archive_sha256: assembled.row.archive_sha256.clone(),
            size_bytes: assembled.row.size_bytes,
            scan_report_json: assembled.row.scan_report_json.clone(),
            listing: assembled.row.listing.clone(),
        };
        if let Err(e) = db.change_installed_mod_file(row.id, &change).await {
            let _ = guard.rename(&target, &new_dir);
            if current.is_some() {
                let _ = guard.rename(&aside, &target);
            }
            failure = Some(e.into());
            break;
        }
        if current.is_some() {
            previous.push((row.folder_name.clone(), aside));
        }
        if enabled != row.enabled {
            db.set_mod_enabled(row.id, enabled).await?;
        }
        report.changed.push(ChangedFolder {
            id: row.id,
            folder_name: row.folder_name.clone(),
            enabled,
            files: assembled.folder.files,
            bytes: assembled.folder.bytes,
            skipped: assembled.folder.skipped,
        });
    }

    for (folder, path) in previous {
        if remover.remove(guard, &path).is_err() {
            report.not_removed.push(folder);
        }
    }
    if report.not_removed.is_empty() {
        let _ = guard.remove_dir_all(&stage);
    }
    match failure {
        Some(e) if report.changed.is_empty() => Err(e),
        // Some folders moved: say so rather than hide them behind the error.
        Some(e) => Err(CoreError::PartlyChanged {
            changed: report.changed.len(),
            message: e.to_string(),
        }),
        None => Ok(report),
    }
}
