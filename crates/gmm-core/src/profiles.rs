// SPDX-License-Identifier: MIT OR Apache-2.0

//! Profiles (Phase 6): named sets of switched-on mods per game. Saving records which mods
//! are on; applying switches exactly those on and every other mod off (the usual `DISABLED `
//! rename, GMM's and hand-installed mods alike). A profile can be shared as a code: compact
//! JSON naming GMM's mods by GameBanana id (hand-installed mods can't be shared), which
//! another GMM turns into a profile of the mods it has, listing the ones it doesn't.
//!
//! GMM's mods are kept by folder name (stable, unique per game; database ids can be reused).
//! Hand-installed mods are kept by their path under `Mods/` without a `DISABLED` prefix, since
//! switching one renames it. Share codes are untrusted input: size, count and length limits,
//! and only the fields below are read.
//!
//! Not yet: 3DMigoto's saved in-game toggles (`d3dx_user.ini`) aren't part of a profile;
//! that file lives outside `Mods/` (security rule R3).

use gmm_adapters::{GameManifest, Registry};
use gmm_db::Db;
use gmm_fsguard::RootId;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::library::{Library, library, set_enabled, set_unmanaged_enabled_at};
use crate::workspace::Workspace;

/// Longest profile name.
pub const MAX_PROFILE_NAME: usize = 60;
/// Largest share code accepted.
pub const MAX_SHARE_CODE: usize = 64 * 1024;
const MAX_SHARED_MODS: usize = 1000;
const MAX_SHARED_TEXT: usize = 200;
const SHARE_FORMAT: u32 = 1;

/// One switched-on mod in a saved profile (stored as JSON in the database).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Entry {
    Managed {
        folder: String,
        name: String,
        gb_mod_id: Option<u64>,
        gb_file_id: Option<u64>,
        variant: Option<String>,
    },
    Unmanaged {
        /// Path under `Mods/`, without a `DISABLED` prefix on the last part.
        key: String,
        name: String,
    },
}

/// A saved profile, as lists show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProfileInfo {
    pub id: i64,
    pub game: String,
    pub name: String,
    /// How many mods it switches on.
    pub mods: usize,
    pub updated_at: i64,
}

/// What [`apply_profile`] changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProfileApplied {
    pub switched_on: Vec<String>,
    pub switched_off: Vec<String>,
    /// Mods in the profile that aren't in the Mods folder any more.
    pub missing: Vec<String>,
}

/// A mod in a share code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SharedMod {
    pub gb_mod_id: u64,
    pub gb_file_id: Option<u64>,
    pub variant: Option<String>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShareCode {
    gmm_profile: u32,
    game: String,
    name: String,
    mods: Vec<SharedMod>,
}

/// Outcome of [`import_profile`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProfileImported {
    pub profile: ProfileInfo,
    /// Shared mods this GMM doesn't have; install them, then import again.
    pub missing: Vec<SharedMod>,
}

/// The last part of an unmanaged mod's path without a `DISABLED` prefix (`DISABLED Foo`,
/// `DISABLED_Foo` and `disabledFoo` all become `Foo`), so a mod keeps one key on or off.
fn unmanaged_key(path: &str) -> String {
    let (dir, last) = path.rsplit_once('/').unwrap_or(("", path));
    let bare = if last.to_ascii_lowercase().starts_with("disabled") {
        last["disabled".len()..].trim_start_matches([' ', '_', '-'])
    } else {
        last
    };
    if dir.is_empty() {
        bare.to_owned()
    } else {
        format!("{dir}/{bare}")
    }
}

fn check_name(name: &str) -> Result<String, CoreError> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > MAX_PROFILE_NAME
        || name.chars().any(char::is_control)
    {
        return Err(CoreError::BadProfile(format!(
            "a profile name needs 1 to {MAX_PROFILE_NAME} characters, without control characters"
        )));
    }
    Ok(name.to_owned())
}

fn entries_on(lib: &Library) -> Vec<Entry> {
    let managed = lib
        .managed
        .iter()
        .filter(|m| m.row.enabled)
        .map(|m| Entry::Managed {
            folder: m.row.folder_name.clone(),
            name: m.row.name.clone(),
            gb_mod_id: m.row.gb_mod_id,
            gb_file_id: m.row.gb_file_id,
            variant: m.row.variant.clone(),
        });
    let unmanaged = lib
        .unmanaged
        .iter()
        .filter(|u| u.enabled)
        .map(|u| Entry::Unmanaged {
            key: unmanaged_key(&u.path),
            name: unmanaged_key(&u.name),
        });
    managed.chain(unmanaged).collect()
}

fn parse_entries(json: &str) -> Result<Vec<Entry>, CoreError> {
    serde_json::from_str(json)
        .map_err(|e| CoreError::BadProfile(format!("saved profile is unreadable: {e}")))
}

fn info(row: &gmm_db::ProfileRow) -> ProfileInfo {
    ProfileInfo {
        id: row.id,
        game: row.game_id.clone(),
        name: row.name.clone(),
        mods: parse_entries(&row.entries_json).map_or(0, |e| e.len()),
        updated_at: row.updated_at,
    }
}

async fn row_for(db: &Db, game: &GameManifest, id: i64) -> Result<gmm_db::ProfileRow, CoreError> {
    db.profile(id)
        .await?
        .filter(|p| p.game_id == game.id())
        .ok_or_else(|| CoreError::BadProfile(format!("no profile {id} for {}", game.game.name)))
}

/// A game's saved profiles, by name.
pub async fn profiles(db: &Db, game: &GameManifest) -> Result<Vec<ProfileInfo>, CoreError> {
    Ok(db.profiles(game.id()).await?.iter().map(info).collect())
}

/// Saves which mods are on now as profile `name` (replacing one of that name).
pub async fn save_profile(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    name: &str,
) -> Result<ProfileInfo, CoreError> {
    let name = check_name(name)?;
    let lib = library(db, ws, mods, game).await?;
    let json = serde_json::to_string(&entries_on(&lib))?;
    let id = db.save_profile(game.id(), &name, &json).await?;
    Ok(info(&row_for(db, game, id).await?))
}

pub async fn rename_profile(
    db: &Db,
    game: &GameManifest,
    id: i64,
    name: &str,
) -> Result<(), CoreError> {
    let name = check_name(name)?;
    row_for(db, game, id).await?;
    if !db.rename_profile(id, &name).await? {
        return Err(CoreError::BadProfile(format!(
            "a profile named {name:?} already exists"
        )));
    }
    Ok(())
}

pub async fn delete_profile(db: &Db, game: &GameManifest, id: i64) -> Result<(), CoreError> {
    row_for(db, game, id).await?;
    db.delete_profile(id).await?;
    Ok(())
}

/// Switches the profile's mods on and every other mod of the game off.
pub async fn apply_profile(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    id: i64,
) -> Result<ProfileApplied, CoreError> {
    let entries = parse_entries(&row_for(db, game, id).await?.entries_json)?;
    let lib = library(db, ws, mods, game).await?;
    let mut out = ProfileApplied::default();

    for m in &lib.managed {
        let want = entries.iter().any(
            |e| matches!(e, Entry::Managed { folder, .. } if folder.eq_ignore_ascii_case(&m.row.folder_name)),
        );
        if want != m.row.enabled {
            set_enabled(db, ws, mods, game, m.row.id, want).await?;
            let list = if want {
                &mut out.switched_on
            } else {
                &mut out.switched_off
            };
            list.push(m.row.name.clone());
        }
    }
    for u in &lib.unmanaged {
        let key = unmanaged_key(&u.path);
        let want = entries
            .iter()
            .any(|e| matches!(e, Entry::Unmanaged { key: k, .. } if k.eq_ignore_ascii_case(&key)));
        if want != u.enabled {
            set_unmanaged_enabled_at(db, ws, mods, game, &u.path, want).await?;
            let list = if want {
                &mut out.switched_on
            } else {
                &mut out.switched_off
            };
            list.push(unmanaged_key(&u.name));
        }
    }
    for e in &entries {
        let present = match e {
            Entry::Managed { folder, .. } => lib
                .managed
                .iter()
                .any(|m| m.row.folder_name.eq_ignore_ascii_case(folder)),
            Entry::Unmanaged { key, .. } => lib
                .unmanaged
                .iter()
                .any(|u| unmanaged_key(&u.path).eq_ignore_ascii_case(key)),
        };
        if !present {
            out.missing.push(match e {
                Entry::Managed { name, .. } | Entry::Unmanaged { name, .. } => name.clone(),
            });
        }
    }
    Ok(out)
}

/// The share code for a profile: compact JSON with the GameBanana ids of its GMM mods.
/// Hand-installed mods are left out (another computer can't have them).
pub async fn export_profile(db: &Db, game: &GameManifest, id: i64) -> Result<String, CoreError> {
    let row = row_for(db, game, id).await?;
    let mods = parse_entries(&row.entries_json)?
        .into_iter()
        .filter_map(|e| match e {
            Entry::Managed {
                name,
                gb_mod_id: Some(gb_mod_id),
                gb_file_id,
                variant,
                ..
            } => Some(SharedMod {
                gb_mod_id,
                gb_file_id,
                variant,
                name,
            }),
            _ => None,
        })
        .collect();
    let code = ShareCode {
        gmm_profile: SHARE_FORMAT,
        game: game.id().to_owned(),
        name: row.name,
        mods,
    };
    Ok(serde_json::to_string(&code)?)
}

/// Reads a share code (untrusted) for `game` into a profile of the mods this GMM has.
/// `name` overrides the code's name.
pub async fn import_profile(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    code: &str,
    name: Option<&str>,
) -> Result<ProfileImported, CoreError> {
    let bad = |why: &str| CoreError::BadProfile(format!("that isn't a GMM profile code: {why}"));
    if code.len() > MAX_SHARE_CODE {
        return Err(bad("it's too long"));
    }
    let shared: ShareCode = serde_json::from_str(code.trim()).map_err(|_| bad("unreadable"))?;
    if shared.gmm_profile != SHARE_FORMAT {
        return Err(bad("made by a different GMM version"));
    }
    if Registry::builtin()
        .ok()
        .and_then(|r| r.get(&shared.game))
        .is_none()
    {
        return Err(bad("unknown game"));
    }
    if shared.game != game.id() {
        return Err(CoreError::BadProfile(format!(
            "this profile is for another game ({})",
            shared.game
        )));
    }
    if shared.mods.len() > MAX_SHARED_MODS
        || shared.mods.iter().any(|m| {
            m.name.len() > MAX_SHARED_TEXT
                || m.variant
                    .as_ref()
                    .is_some_and(|v| v.len() > MAX_SHARED_TEXT)
        })
    {
        return Err(bad("too many mods or names too long"));
    }
    let name = check_name(name.unwrap_or(&shared.name))?;

    let lib = library(db, ws, mods, game).await?;
    let mut entries = Vec::new();
    let mut missing = Vec::new();
    for m in shared.mods {
        let found = lib.managed.iter().find(|row| {
            row.row.gb_mod_id == Some(m.gb_mod_id)
                && row.row.variant.as_deref().map(str::to_ascii_lowercase)
                    == m.variant.as_deref().map(str::to_ascii_lowercase)
        });
        match found {
            Some(row) => entries.push(Entry::Managed {
                folder: row.row.folder_name.clone(),
                name: row.row.name.clone(),
                gb_mod_id: row.row.gb_mod_id,
                gb_file_id: row.row.gb_file_id,
                variant: row.row.variant.clone(),
            }),
            None => missing.push(m),
        }
    }
    let id = db
        .save_profile(game.id(), &name, &serde_json::to_string(&entries)?)
        .await?;
    Ok(ProfileImported {
        profile: info(&row_for(db, game, id).await?),
        missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmanaged_keys_ignore_the_disabled_prefix() {
        assert_eq!(unmanaged_key("Phoenix"), "Phoenix");
        assert_eq!(unmanaged_key("DISABLED Phoenix"), "Phoenix");
        assert_eq!(unmanaged_key("DISABLED_Old"), "Old");
        assert_eq!(unmanaged_key("GMM/disabledThing"), "GMM/Thing");
        assert_eq!(unmanaged_key("GMM/Disabled - Hair.ini"), "GMM/Hair.ini");
    }

    #[test]
    fn names_are_checked() {
        assert!(check_name("  Casual  ").is_ok_and(|n| n == "Casual"));
        assert!(check_name("").is_err());
        assert!(check_name("a\nb").is_err());
        assert!(check_name(&"x".repeat(MAX_PROFILE_NAME + 1)).is_err());
    }
}
