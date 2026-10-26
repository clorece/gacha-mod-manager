// SPDX-License-Identifier: MIT OR Apache-2.0

//! The startup check's GameBanana steps: pull the newest
//! catalogue page per game so the store opens fresh, and compare each GMM-installed mod with
//! its GameBanana page to find updates (a newer clean file than the one installed).

use std::collections::BTreeMap;

use gmm_adapters::GameManifest;
use gmm_catalog::{CatalogSource, ListQuery, ModFile, Sort};
use gmm_db::Db;
use serde::Serialize;

use crate::error::CoreError;

/// The startup check accepts a mod's file list saved this recently instead of asking again,
/// so starting GMM a few times a day doesn't ask about every installed mod each time.
const UPDATE_CHECK_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

/// A GameBanana mod with a newer file than the installed one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModUpdateInfo {
    pub game: String,
    pub gb_mod_id: u64,
    pub name: String,
    /// The installed folders (versions) of this mod.
    pub installed_ids: Vec<i64>,
    pub installed_file_id: Option<u64>,
    /// Upload date of the installed file (Unix seconds).
    pub installed_file_date: Option<i64>,
    /// The newest file GameBanana scanned as clean.
    pub newest: ModFile,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateCheck {
    /// GameBanana mods looked at.
    pub checked: usize,
    pub updates: Vec<ModUpdateInfo>,
    /// Mods that couldn't be checked (removed from GameBanana, network trouble), as text.
    pub notes: Vec<String>,
}

/// Reads the newest page of a game's catalogue, which also refreshes the cache the store
/// opens from. Returns how many mods it listed.
pub async fn refresh_catalog<C: CatalogSource>(
    catalog: &C,
    game: &GameManifest,
) -> Result<usize, CoreError> {
    let query = ListQuery {
        sort: Sort::Newest,
        ..ListQuery::new(game.gamebanana.game_id)
    };
    Ok(catalog.list_mods(&query).await?.items.len())
}

/// Compares every GameBanana mod installed for `game` with its current files. One failing
/// mod doesn't stop the others; it becomes a note.
pub async fn check_updates<C: CatalogSource>(
    catalog: &C,
    db: &Db,
    game: &GameManifest,
) -> Result<UpdateCheck, CoreError> {
    let mut by_mod: BTreeMap<u64, Vec<gmm_db::InstalledMod>> = BTreeMap::new();
    for row in db.installed_mods(Some(game.id())).await? {
        if let Some(gb) = row.gb_mod_id {
            by_mod.entry(gb).or_default().push(row);
        }
    }
    let mut check = UpdateCheck {
        checked: by_mod.len(),
        ..UpdateCheck::default()
    };
    for (gb_mod_id, rows) in by_mod {
        let name = rows[0].name.clone();
        let files = match catalog
            .mod_files_within(gb_mod_id, UPDATE_CHECK_MAX_AGE)
            .await
        {
            Ok(files) => files,
            Err(e) => {
                check.notes.push(format!("{name} (mod {gb_mod_id}): {e}"));
                continue;
            }
        };
        let Some(newest) = files.default_choice() else {
            continue;
        };
        let installed_file_id = rows.iter().filter_map(|r| r.gb_file_id).max();
        let installed_file_date = rows.iter().filter_map(|r| r.file_date).max();
        let is_installed = rows.iter().any(|r| r.gb_file_id == Some(newest.id));
        let is_newer = installed_file_date.is_none_or(|d| newest.date_added > d);
        if !is_installed && is_newer {
            check.updates.push(ModUpdateInfo {
                game: game.id().to_owned(),
                gb_mod_id,
                name,
                installed_ids: rows.iter().map(|r| r.id).collect(),
                installed_file_id,
                installed_file_date,
                newest: newest.clone(),
            });
        }
    }
    Ok(check)
}
