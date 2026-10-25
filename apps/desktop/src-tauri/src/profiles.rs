// SPDX-License-Identifier: MIT OR Apache-2.0

//! Profiles (Phase 6): named sets of switched-on mods per game, and their share codes
//! (`gmm_core` profiles). Profiles are named by id; a share code is untrusted text that
//! gmm-core checks (size, counts, the game) before anything is saved.

use gmm_core::{ProfileApplied, ProfileImported, ProfileInfo};
use tauri::State;

use crate::AppState;
use crate::commands::{game_by_id, mods_target};
use crate::error::CmdResult;

/// A game's saved profiles.
#[tauri::command]
pub async fn profiles(state: State<'_, AppState>, game: String) -> CmdResult<Vec<ProfileInfo>> {
    let game = game_by_id(&game)?;
    Ok(gmm_core::profiles(&state.db, game).await?)
}

/// Saves which mods are on now as profile `name` (replacing one of that name).
#[tauri::command]
pub async fn profile_save(
    state: State<'_, AppState>,
    game: String,
    name: String,
) -> CmdResult<ProfileInfo> {
    let game = game_by_id(&game)?;
    let target = mods_target(&state, game).await?;
    Ok(gmm_core::save_profile(&state.db, &target.ws, target.mods, game, &name).await?)
}

/// Switches the profile's mods on and every other mod of the game off.
#[tauri::command]
pub async fn profile_apply(
    state: State<'_, AppState>,
    game: String,
    id: i64,
) -> CmdResult<ProfileApplied> {
    let game = game_by_id(&game)?;
    let target = mods_target(&state, game).await?;
    Ok(gmm_core::apply_profile(&state.db, &target.ws, target.mods, game, id).await?)
}

#[tauri::command]
pub async fn profile_rename(
    state: State<'_, AppState>,
    game: String,
    id: i64,
    name: String,
) -> CmdResult<()> {
    let game = game_by_id(&game)?;
    Ok(gmm_core::rename_profile(&state.db, game, id, &name).await?)
}

/// Deletes the profile; no mod is touched.
#[tauri::command]
pub async fn profile_delete(state: State<'_, AppState>, game: String, id: i64) -> CmdResult<()> {
    let game = game_by_id(&game)?;
    Ok(gmm_core::delete_profile(&state.db, game, id).await?)
}

/// The profile's share code (GameBanana ids of its GMM mods).
#[tauri::command]
pub async fn profile_export(
    state: State<'_, AppState>,
    game: String,
    id: i64,
) -> CmdResult<String> {
    let game = game_by_id(&game)?;
    Ok(gmm_core::export_profile(&state.db, game, id).await?)
}

/// Makes a profile from a share code someone pasted.
#[tauri::command]
pub async fn profile_import(
    state: State<'_, AppState>,
    game: String,
    code: String,
    name: Option<String>,
) -> CmdResult<ProfileImported> {
    let game = game_by_id(&game)?;
    let target = mods_target(&state, game).await?;
    Ok(gmm_core::import_profile(
        &state.db,
        &target.ws,
        target.mods,
        game,
        &code,
        name.as_deref(),
    )
    .await?)
}
