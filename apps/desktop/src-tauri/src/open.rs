// SPDX-License-Identifier: MIT OR Apache-2.0

//! Opening things outside GMM: a web page in the user's browser, a mod's folder in the file
//! manager (security rule R7; the launching itself is in `gmm_xxmi::open_page` /
//! `open_folder`).
//!
//! The webview never passes an address. Pages are built here from ids (a GameBanana mod id,
//! the fixed XXMI releases page) or looked up from a link key that a rewritten description
//! handed out (`html::Links`). Before a page opens, a native dialog from Rust shows the full
//! address and asks; the webview can't skip or answer it. Folders need no question: they are
//! GMM's own Mods folders, opened as folders only.

use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::AppState;
use crate::commands::{game_by_id, mods_target};
use crate::error::{CmdError, CmdResult};
use crate::views::ModKey;

/// Asks before leaving the app; true when the user chose to open the page.
async fn confirm_page<R: Runtime>(app: &AppHandle<R>, url: &str, what: &str) -> bool {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(format!("{what}\n\n{url}"))
        .title("Open in your browser?")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Open".to_owned(),
            "Cancel".to_owned(),
        ))
        .show(move |yes| {
            let _ = tx.send(yes);
        });
    rx.await.unwrap_or(false)
}

/// Confirms, then opens. Returns false when the user said no.
async fn open_page<R: Runtime>(app: &AppHandle<R>, url: &str, what: &str) -> CmdResult<bool> {
    open_page_as(app, url, url, what).await
}

/// The same, showing `shown` instead of the address: for a long address GMM built itself
/// from a fixed base (the bug report's pre-filled issue), where the base is what matters.
pub(crate) async fn open_page_as<R: Runtime>(
    app: &AppHandle<R>,
    url: &str,
    shown: &str,
    what: &str,
) -> CmdResult<bool> {
    if !confirm_page(app, shown, what).await {
        return Ok(false);
    }
    gmm_xxmi::open_page(url).map_err(CmdError::other)?;
    Ok(true)
}

/// A mod's page on GameBanana. Returns false when the user cancelled.
#[tauri::command]
pub async fn open_mod_page<R: Runtime>(app: AppHandle<R>, mod_id: u64) -> CmdResult<bool> {
    let url = format!("https://gamebanana.com/mods/{mod_id}");
    open_page(&app, &url, "This opens the mod's page on GameBanana.").await
}

/// XXMI Launcher's official releases page (setup guide, "XXMI not found").
#[tauri::command]
pub async fn open_xxmi_releases<R: Runtime>(app: AppHandle<R>) -> CmdResult<bool> {
    open_page(
        &app,
        gmm_xxmi::XXMI_RELEASES,
        "This opens XXMI Launcher's official releases page on GitHub.",
    )
    .await
}

/// A link from a mod description, by the key the rewritten HTML carries. The dialog shows
/// where it really goes, since a description's link text can say anything.
#[tauri::command]
pub async fn open_link<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    key: String,
) -> CmdResult<bool> {
    let url = state
        .links
        .get(&key)
        .ok_or_else(|| CmdError::other("that link is no longer known; reopen the page"))?;
    open_page(
        &app,
        &url,
        "This link from the mod's description leaves GMM. Only open it if you trust where it goes.",
    )
    .await
}

/// Shows a mod's folder in the file manager: a GMM mod's folder, a hand-installed mod's
/// folder, or the folder a loose `.ini` sits in.
#[tauri::command]
pub async fn open_mod_folder(
    state: State<'_, AppState>,
    game: String,
    key: ModKey,
) -> CmdResult<()> {
    let game = game_by_id(&game)?;
    let target = mods_target(&state, game).await?;
    let which = match &key {
        ModKey::Managed { id } => gmm_core::ModFolder::Managed(*id),
        ModKey::Unmanaged { path } => gmm_core::ModFolder::Unmanaged(path),
    };
    let dir = gmm_core::mod_folder(&state.db, &target.ws, target.mods, game, which).await?;
    gmm_xxmi::open_folder(&dir).map_err(CmdError::other)
}
