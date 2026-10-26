// SPDX-License-Identifier: MIT OR Apache-2.0

//! Versions as one mod: the folders GMM installed from the same
//! GameBanana file are versions of one mod. Any number of them can be on together (a mod
//! split into pieces, say); switching one on switches off only the others that replace the
//! same things.

use gmm_adapters::GameManifest;
use gmm_db::Db;
use gmm_fsguard::RootId;

use crate::conflicts::{ModRef, conflicts_for};
use crate::error::CoreError;
use crate::library::{ToggleOutcome, set_enabled};
use crate::workspace::Workspace;

/// The key that groups versions in the library: GameBanana mod and file.
pub fn version_group(gb_mod_id: Option<u64>, gb_file_id: Option<u64>) -> Option<String> {
    Some(format!("{}:{}", gb_mod_id?, gb_file_id?))
}

/// Switches version `id` on and every other version of the same mod that clashes with it
/// off. Returns each change, `id` first.
pub async fn switch_version(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    id: i64,
) -> Result<Vec<ToggleOutcome>, CoreError> {
    let siblings = siblings(db, game, id).await?;
    let clashing = clashing_siblings(db, ws, mods, game, id, &siblings).await?;
    let mut changes = vec![set_enabled(db, ws, mods, game, id, true).await?];
    for other in clashing {
        if changes.iter().any(|c| c.id == other) {
            continue;
        }
        changes.push(set_enabled(db, ws, mods, game, other, false).await?);
    }
    Ok(changes)
}

/// After installing several versions of one mod switched on, in the order the user picked
/// them: the first stays on (switching off the versions already installed that clash with
/// it, as [`switch_version`] does), and each later one stays on unless it clashes with a
/// version that is on. Every version that can work alongside the others ends up on.
/// Returns the ids switched off.
pub async fn settle_versions(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    ids: &[i64],
) -> Result<Vec<i64>, CoreError> {
    let Some((&first, rest)) = ids.split_first() else {
        return Ok(Vec::new());
    };
    let mut off: Vec<i64> = switch_version(db, ws, mods, game, first)
        .await?
        .into_iter()
        .filter(|c| !c.enabled && c.changed)
        .map(|c| c.id)
        .collect();
    for (at, &id) in rest.iter().enumerate() {
        if off.contains(&id) {
            continue;
        }
        // Only versions already settled (or installed before) count; later picks are still
        // on and wait their turn.
        let later = &rest[at + 1..];
        let siblings: Vec<i64> = siblings(db, game, id)
            .await?
            .into_iter()
            .filter(|s| !later.contains(s))
            .collect();
        if !clashing_siblings(db, ws, mods, game, id, &siblings)
            .await?
            .is_empty()
        {
            set_enabled(db, ws, mods, game, id, false).await?;
            off.push(id);
        }
    }
    Ok(off)
}

/// The other versions of `id` (same GameBanana mod and file, same game).
async fn siblings(db: &Db, game: &GameManifest, id: i64) -> Result<Vec<i64>, CoreError> {
    let not_a_version = || CoreError::NotAVersion {
        id,
        game: game.game.name.clone(),
    };
    let row = db
        .installed_mod(id)
        .await?
        .filter(|m| m.game_id == game.id())
        .ok_or_else(not_a_version)?;
    let group = version_group(row.gb_mod_id, row.gb_file_id).ok_or_else(not_a_version)?;
    Ok(match row.gb_mod_id {
        Some(gb) => db
            .installed_mods_for_gb(gb)
            .await?
            .into_iter()
            .filter(|m| {
                m.id != id
                    && m.game_id == game.id()
                    && version_group(m.gb_mod_id, m.gb_file_id).as_deref() == Some(&group)
            })
            .map(|m| m.id)
            .collect(),
        None => Vec::new(),
    })
}

/// The versions among `siblings` that are on and replace the same things as `id`.
async fn clashing_siblings(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    id: i64,
    siblings: &[i64],
) -> Result<Vec<i64>, CoreError> {
    let report = conflicts_for(db, ws, mods, game, id).await?;
    let mut clashing: Vec<i64> = report
        .conflicts
        .iter()
        .flat_map(|c| [&c.a, &c.b])
        .filter_map(|r| match r {
            ModRef::Managed { id: other, .. } if *other != id && siblings.contains(other) => {
                Some(*other)
            }
            _ => None,
        })
        .collect();
    clashing.dedup();
    Ok(clashing)
}
