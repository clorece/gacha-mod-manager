// SPDX-License-Identifier: MIT OR Apache-2.0

//! Installing from an archive on the user's computer (Phase 6). The path comes from the OS
//! only: a file dropped on the window (Tauri hands the drop to Rust, not to the webview) or a
//! native file dialog opened here. Rust keeps the path; the webview learns the file's name
//! and gets the confirmation (\`gmm_core::LocalPlan\`), then says which versions to install.
//! One archive waits at a time; a new drop replaces it.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use gmm_core::{InstallOptions, InstalledFolder, LocalPlan, LocalPrepared, ModsTarget};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::AppState;
use crate::commands::{game_by_id, mods_target};
use crate::error::{CmdError, CmdResult};
use crate::views::VariantPick;

struct Pending {
    game: &'static gmm_adapters::GameManifest,
    target: ModsTarget,
    prepared: LocalPrepared,
}

#[derive(Default)]
pub struct Local {
    /// The file dropped or picked last, waiting for the user to say which game it's for.
    chosen: Mutex<Option<PathBuf>>,
    /// The archive copied and analysed for the open confirmation, with its game and the
    /// workspace its paths belong to.
    pending: Mutex<Option<Pending>>,
    watchers: Mutex<Vec<Channel<String>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Called from the window's drop event: remembers the first file and tells the webview its
/// name, which opens the local install confirmation.
pub fn dropped(app: &AppHandle, paths: &[PathBuf]) {
    let Some(path) = paths.first() else {
        return;
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let state = app.state::<AppState>();
    *lock(&state.local.chosen) = Some(path.clone());
    lock(&state.local.watchers).retain(|w| w.send(name.clone()).is_ok());
}

/// Sends the name of each file dropped on the window.
#[tauri::command]
pub fn drop_watch(state: State<'_, AppState>, on_drop: Channel<String>) {
    lock(&state.local.watchers).push(on_drop);
}

/// Opens a file dialog for an archive; returns its name, or `None` when cancelled.
#[tauri::command]
pub async fn pick_archive(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose a mod archive")
        .add_filter("Mod archives", &gmm_core::LOCAL_ARCHIVE_TYPES)
        .pick_file(move |file| {
            let _ = tx.send(file.and_then(|f| f.into_path().ok()));
        });
    let Some(path) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    *lock(&state.local.chosen) = Some(path);
    Ok(Some(name))
}

/// Copies the chosen file into the cache and looks inside it for `game`: the confirmation.
#[tauri::command]
pub async fn prepare_local(state: State<'_, AppState>, game: String) -> CmdResult<LocalPlan> {
    let game = game_by_id(&game)?;
    let path = lock(&state.local.chosen)
        .clone()
        .ok_or_else(|| CmdError::other("no file was dropped or picked"))?;
    if let Some(old) = lock(&state.local.pending).take() {
        old.prepared.discard(&old.target.ws);
    }
    let target = mods_target(&state, game).await?;
    // Copying and extracting can take a while; keep it off the async workers.
    let (target, prepared) = tauri::async_runtime::spawn_blocking(move || {
        let prepared = gmm_core::prepare_local(&target.ws, game, &path);
        (target, prepared)
    })
    .await
    .map_err(|e| CmdError::other(e.to_string()))?;
    let prepared = prepared?;
    let plan = gmm_core::local_plan(&state.db, game, &prepared).await;
    *lock(&state.local.pending) = Some(Pending {
        game,
        target,
        prepared,
    });
    Ok(plan?)
}

/// Installs the chosen versions of the prepared archive, then forgets it.
#[tauri::command]
pub async fn install_local(
    state: State<'_, AppState>,
    variants: VariantPick,
    skip_blocked: bool,
    start_disabled: bool,
) -> CmdResult<Vec<InstalledFolder>> {
    let Pending {
        game,
        target,
        prepared,
    } = lock(&state.local.pending)
        .take()
        .ok_or_else(|| CmdError::other("the archive is no longer prepared; drop it again"))?;
    let options = InstallOptions {
        variants: variants.into(),
        skip_blocked,
        start_disabled,
    };
    let result = gmm_core::install_local(
        &state.db,
        &target.ws,
        target.mods,
        game,
        &prepared,
        &options,
        |_| {},
    )
    .await;
    prepared.discard(&target.ws);
    let installed = result?;
    if !start_disabled && installed.len() > 1 {
        gmm_core::switch_version(&state.db, &target.ws, target.mods, game, installed[0].id).await?;
    }
    *lock(&state.local.chosen) = None;
    Ok(installed)
}

/// Closes the confirmation without installing: removes the cached copy.
#[tauri::command]
pub async fn discard_local(state: State<'_, AppState>) -> CmdResult<()> {
    let pending = lock(&state.local.pending).take();
    *lock(&state.local.chosen) = None;
    if let Some(p) = pending {
        p.prepared.discard(&p.target.ws);
    }
    Ok(())
}
