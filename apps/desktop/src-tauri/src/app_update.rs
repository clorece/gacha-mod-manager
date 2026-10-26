// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app update check (Phase 7): whether a newer GMM is on GitHub, and opening its release
//! page. GMM never downloads or runs an update; the user installs the new MSI themselves.
//! The webview can only ask to open the version the last check found: the address is built
//! here from that version and shown in a native dialog first (`open.rs`).

use std::sync::PoisonError;

use gmm_core::{APP_VERSION, AppRelease, CoreError, check_app_update};
use tauri::{AppHandle, Runtime, State};

use crate::AppState;
use crate::error::{CmdError, CmdResult};
use crate::open::open_page_as;

/// Checks GitHub and remembers what it found, for [`open_app_release`].
pub(crate) async fn check(state: &AppState) -> Result<Option<AppRelease>, CoreError> {
    let found = check_app_update(&state.releases, APP_VERSION).await?;
    *state
        .newest_release
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = found.as_ref().map(|r| r.version.clone());
    Ok(found)
}

/// The newer GMM release on GitHub, if there is one (Settings › About, "Check for updates").
#[tauri::command]
pub async fn app_update_check(state: State<'_, AppState>) -> CmdResult<Option<AppRelease>> {
    Ok(check(&state).await?)
}

/// Opens the page of the release the last check found, after a native confirmation. Any
/// other version is refused. Returns false when the user cancelled.
#[tauri::command]
pub async fn open_app_release<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    version: String,
) -> CmdResult<bool> {
    let known = state
        .newest_release
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if known.as_deref() != Some(version.as_str()) {
        return Err(CmdError::other(
            "that isn't the GMM update the last check found",
        ));
    }
    let url = gmm_core::release_page(&version);
    open_page_as(
        &app,
        &url,
        &url,
        &format!("GMM {version} is on GitHub. Its page has the installer and what changed."),
    )
    .await
}
