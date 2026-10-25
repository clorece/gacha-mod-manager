// SPDX-License-Identifier: MIT OR Apache-2.0

//! Versions as one mod (docs/08, "Mod settings"): the folders GMM installed from the same
//! GameBanana file are versions of one mod. Switching one on switches off the others that
//! replace the same things; versions that don't clash are left alone.

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
    let siblings: Vec<i64> = match row.gb_mod_id {
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
    };

    // What it clashes with among the mods that are on, worked out before switching it on.
    let report = conflicts_for(db, ws, mods, game, id).await?;
    let clashing: Vec<i64> = report
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

    let mut changes = vec![set_enabled(db, ws, mods, game, id, true).await?];
    for other in clashing {
        if changes.iter().any(|c| c.id == other) {
            continue;
        }
        changes.push(set_enabled(db, ws, mods, game, other, false).await?);
    }
    Ok(changes)
}
