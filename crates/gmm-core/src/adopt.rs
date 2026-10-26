// SPDX-License-Identifier: MIT OR Apache-2.0

//! Adopting a hand-installed mod (Phase 6): GMM takes over a folder someone put in `Mods/`
//! by hand, once the user has said which GameBanana mod (and file) it is. From then on it is
//! one of GMM's mods: it shows its GameBanana page, gets update checks and can be updated
//! (which replaces its files with the GameBanana ones).
//!
//! Finding the match: a `gmm.json` left inside the folder (GMM's own folder whose database
//! row was lost) is exact; a GameBanana id in the folder name (`[gb-123]`, a 6+ digit number)
//! comes next; then a GameBanana search for the folder's name. The user picks; nothing is
//! adopted automatically.
//!
//! Adopting keeps the folder's name. A folder already in `Mods/GMM/` stays where it is; one
//! elsewhere in `Mods/` is moved into `Mods/GMM/` (one rename on the same volume), which
//! resets the in-game toggle state 3DMigoto saved for it (it's keyed by path). A `gmm.json`
//! is written into the folder and the database row is added; if that fails, the folder is
//! moved back. Loose `.ini` files can't be adopted (GMM's mods are folders).

use gmm_adapters::GameManifest;
use gmm_catalog::{CatalogSource, ModSummary, SearchQuery};
use gmm_db::{Db, NewInstalledMod};
use gmm_fsguard::RootId;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::install::{SIDECAR_FORMAT, Sidecar, listing_of};
use crate::library::{library, listed_unmanaged};
use crate::names::{GMM_DIR, SIDECAR_FILE, on_disk_name};
use crate::storage::unmanaged_size;
use crate::workspace::Workspace;

/// Search text is cut to this many characters.
const MAX_SEARCH_CHARS: usize = 60;

/// Why a GameBanana mod is offered as the match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum MatchReason {
    /// The folder's own `gmm.json` names it.
    Sidecar,
    /// The folder name contains its id.
    IdInName,
    /// GameBanana's search for the folder's name found it.
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AdoptCandidate {
    pub summary: ModSummary,
    pub reason: MatchReason,
    /// The file the folder's `gmm.json` names, for a [`MatchReason::Sidecar`] match.
    pub file_id: Option<u64>,
}

/// Outcome of [`adopt`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Adopted {
    pub id: i64,
    pub folder_name: String,
    /// It was moved into `Mods/GMM/` (its in-game toggle state starts over).
    pub moved: bool,
}

/// The fields GMM reads from a `gmm.json` found in a hand-installed folder.
#[derive(Debug, Deserialize)]
struct FoundSidecar {
    gb_mod_id: Option<u64>,
    gb_file_id: Option<u64>,
}

/// The folder's name without a `DISABLED` prefix.
fn bare_name(name: &str) -> &str {
    if name.to_ascii_lowercase().starts_with("disabled") {
        name["disabled".len()..].trim_start_matches([' ', '_', '-'])
    } else {
        name
    }
}

/// `[gb-123]`, or a run of at least six digits, in a folder name.
fn id_in_name(name: &str) -> Option<u64> {
    if let Some(start) = name.find("[gb-") {
        let digits: String = name[start + 4..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        return digits.parse().ok();
    }
    name.split(|c: char| !c.is_ascii_digit())
        .find(|run| (6..=9).contains(&run.len()))
        .and_then(|run| run.parse().ok())
}

/// What to search GameBanana for: the name without brackets, separators or the prefix.
fn search_text(name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in bare_name(name).chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            '_' | '-' | '.' | '+' => out.push(' '),
            c => out.push(c),
        }
    }
    let words: Vec<&str> = out.split_whitespace().collect();
    words.join(" ").chars().take(MAX_SEARCH_CHARS).collect()
}

/// GameBanana mods a hand-installed folder could be, best first.
pub async fn adopt_candidates<C: CatalogSource>(
    catalog: &C,
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    path: &str,
) -> Result<Vec<AdoptCandidate>, CoreError> {
    let lib = library(db, ws, mods, game).await?;
    let entry = listed_unmanaged(&lib, game, path)?;
    if !entry.is_dir {
        return Err(CoreError::CantAdopt(
            "only folders can be adopted".to_owned(),
        ));
    }
    let guard = ws.guard();
    let folder = guard.path(mods, &entry.path)?;
    let mut out: Vec<AdoptCandidate> = Vec::new();
    let mut add = |summary: ModSummary, reason, file_id| {
        let ours = summary.game_id.is_none_or(|g| g == game.gamebanana.game_id);
        if ours && !out.iter().any(|c| c.summary.id == summary.id) {
            out.push(AdoptCandidate {
                summary,
                reason,
                file_id,
            });
        }
    };

    let sidecar = guard
        .read(&folder.join(SIDECAR_FILE)?)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<FoundSidecar>(&bytes).ok());
    if let Some(found) = sidecar
        && let Some(id) = found.gb_mod_id
        && let Ok(profile) = catalog.mod_profile(id).await
    {
        add(profile.summary, MatchReason::Sidecar, found.gb_file_id);
    }
    if let Some(id) = id_in_name(&entry.name)
        && let Ok(profile) = catalog.mod_profile(id).await
    {
        add(profile.summary, MatchReason::IdInName, None);
    }
    let text = search_text(&entry.name);
    if !text.is_empty() {
        let query = SearchQuery {
            game_id: game.gamebanana.game_id,
            text,
            page: 1,
        };
        // A failed search still leaves the exact matches above.
        if let Ok(page) = catalog.search(&query).await {
            for summary in page.items {
                add(summary, MatchReason::Search, None);
            }
        }
    }
    Ok(out)
}

/// Takes over the hand-installed folder at `path` (as the library listed it) as GameBanana
/// mod `gb_mod_id`, file `file_id` (default: the newest clean one).
#[allow(clippy::too_many_arguments)]
pub async fn adopt<C: CatalogSource>(
    catalog: &C,
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    path: &str,
    gb_mod_id: u64,
    file_id: Option<u64>,
) -> Result<Adopted, CoreError> {
    let lib = library(db, ws, mods, game).await?;
    let entry = listed_unmanaged(&lib, game, path)?.clone();
    if !entry.is_dir {
        return Err(CoreError::CantAdopt(
            "only folders can be adopted".to_owned(),
        ));
    }
    let profile = catalog.mod_profile(gb_mod_id).await?;
    if let Some(found) = profile.summary.game_id
        && found != game.gamebanana.game_id
    {
        return Err(CoreError::WrongGame {
            mod_id: gb_mod_id,
            expected: game.gamebanana.game_id,
            found,
        });
    }
    let files = catalog.mod_files(gb_mod_id).await?;
    let file = match file_id {
        Some(id) => files
            .files
            .iter()
            .chain(&files.archived_files)
            .find(|f| f.id == id)
            .ok_or(CoreError::NoSuchFile {
                mod_id: gb_mod_id,
                file_id: id,
            })?,
        None => files.default_choice().ok_or_else(|| {
            CoreError::CantAdopt("GameBanana lists no clean file for this mod".to_owned())
        })?,
    };

    let guard = ws.guard();
    let folder_name = bare_name(&entry.name).to_owned();
    if folder_name.is_empty() || folder_name.starts_with('.') {
        return Err(CoreError::CantAdopt(format!(
            "{:?} can't be a GMM folder name",
            entry.name
        )));
    }
    if db
        .installed_mod_by_folder(game.id(), &folder_name)
        .await?
        .is_some()
    {
        return Err(CoreError::AlreadyInstalled {
            game: game.game.name.clone(),
            folder: folder_name,
        });
    }
    let gmm_dir = guard.path(mods, GMM_DIR)?;
    guard.create_dir_all(&gmm_dir)?;
    let from = guard.path(mods, &entry.path)?;
    let to = gmm_dir.join(&on_disk_name(&folder_name, entry.enabled))?;
    let moved = from.rel() != to.rel();
    if moved {
        for on in [true, false] {
            if guard.exists(&gmm_dir.join(&on_disk_name(&folder_name, on))?)? {
                return Err(CoreError::FolderTaken {
                    folder: folder_name,
                });
            }
        }
        guard.rename(&from, &to)?;
    }
    let undo = || {
        if moved {
            let _ = guard.rename(&to, &from);
        }
    };

    let size = match unmanaged_size(
        ws,
        mods,
        &crate::library::UnmanagedMod {
            path: to.rel().to_string(),
            ..entry.clone()
        },
    ) {
        Ok(size) => size,
        Err(e) => {
            undo();
            return Err(e);
        }
    };
    let version = file
        .version
        .clone()
        .or_else(|| profile.summary.version.clone());
    let sidecar = Sidecar {
        format: SIDECAR_FORMAT,
        game: game.id().to_owned(),
        name: profile.summary.name.clone(),
        variant: None,
        gb_mod_id: Some(gb_mod_id),
        gb_file_id: Some(file.id),
        file_name: Some(file.name.clone()),
        file_date: Some(file.date_added),
        version: version.clone(),
        // Adopted, not installed from an archive GMM checked.
        archive_sha256: String::new(),
        installed_at: crate::install::now(),
        installed_by: format!("GMM {} (adopted)", env!("CARGO_PKG_VERSION")),
    };
    let row = NewInstalledMod {
        game_id: game.id().to_owned(),
        folder_name: folder_name.clone(),
        name: profile.summary.name.clone(),
        gb_mod_id: Some(gb_mod_id),
        gb_file_id: Some(file.id),
        variant: None,
        version,
        file_date: Some(file.date_added),
        archive_sha256: None,
        size_bytes: size.bytes,
        scan_report_json: None,
        listing: listing_of(&profile.summary),
    };
    let recorded = async {
        let json = serde_json::to_vec_pretty(&sidecar)?;
        guard.write_atomic(&to.join(SIDECAR_FILE)?, &json)?;
        let id = db.insert_installed_mod(&row).await?;
        if !entry.enabled {
            db.set_mod_enabled(id, false).await?;
        }
        Ok::<i64, CoreError>(id)
    }
    .await;
    match recorded {
        Ok(id) => Ok(Adopted {
            id,
            folder_name,
            moved,
        }),
        Err(e) => {
            undo();
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_to_ids_and_search_text() {
        assert_eq!(id_in_name("Casual Velina [gb-688491]"), Some(688_491));
        assert_eq!(id_in_name("phoenix_731970_v2"), Some(731_970));
        assert_eq!(id_in_name("Ellen v1.2 2024"), None);
        assert_eq!(
            search_text("DISABLED Aemeath_Nanami (v2) [fix]"),
            "Aemeath Nanami"
        );
        assert_eq!(search_text("Phoenix-Maid.Outfit"), "Phoenix Maid Outfit");
        assert_eq!(bare_name("DISABLED_Old"), "Old");
    }
}
