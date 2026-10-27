// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app update (Phase 7): whether a newer GMM is on GitHub, and getting it. On Windows GMM
//! downloads the release's installer, checks its signature against GMM's update key
//! (`gmm_core::download_app_update`), hands it to Windows Installer and closes; the installer
//! shows its own progress and reopens GMM. Elsewhere the release page opens instead. The
//! webview can only ask for the version the last check found: the addresses are built from
//! that version, never taken from the webview or GitHub's answer.

use std::ops::ControlFlow;
use std::sync::PoisonError;
use std::time::Duration;

use gmm_core::{APP_VERSION, AppRelease, CoreError, check_app_update};
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime, State};

use crate::AppState;
use crate::error::{CmdError, CmdResult};
use crate::open::open_page_as;
use crate::views::UpdateProgress;

/// How long "Installing" shows before GMM quits for the installer.
const QUIT_AFTER: Duration = Duration::from_millis(800);

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

/// Refuses any version but the one the last check found.
fn require_found(state: &AppState, version: &str) -> CmdResult<()> {
    let known = state
        .newest_release
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if known.as_deref() != Some(version) {
        return Err(CmdError::other(
            "that isn't the GMM update the last check found",
        ));
    }
    Ok(())
}

/// Downloads the update the last check found (progress on `on_progress`), checks its
/// signature, starts Windows Installer and closes GMM; the installer reopens it. Any other
/// version is refused, and so is a signature that doesn't match. Windows only.
#[tauri::command]
pub async fn app_update_install<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    version: String,
    on_progress: Channel<UpdateProgress>,
) -> CmdResult<()> {
    require_found(&state, &version)?;
    if !gmm_core::SELF_UPDATES {
        return Err(CmdError::other(
            "GMM installs its updates itself on Windows only",
        ));
    }
    let ws = gmm_core::Workspace::open(&state.dirs.cache)?;
    // One message per percent (or per 256 KiB when the size isn't known), not per chunk.
    let mut last = None;
    let msi = gmm_core::download_app_update(&state.updates, &ws, &version, |p| {
        let step = p.total.map_or(p.downloaded / (256 * 1024), |t| {
            p.downloaded * 100 / t.max(1)
        });
        if last != Some(step) {
            last = Some(step);
            let _ = on_progress.send(UpdateProgress::Downloading {
                downloaded: p.downloaded,
                total: p.total,
            });
        }
        ControlFlow::Continue(())
    })
    .await?;
    gmm_xxmi::install_update(&msi).map_err(CmdError::other)?;
    let _ = on_progress.send(UpdateProgress::Installing);
    // Quit so Windows Installer can replace GMM's files; it reopens GMM when it's done.
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(QUIT_AFTER).await;
        app.exit(0);
    });
    Ok(())
}

/// Opens the page of the release the last check found, after a native confirmation. Any
/// other version is refused. Returns false when the user cancelled.
#[tauri::command]
pub async fn open_app_release<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    version: String,
) -> CmdResult<bool> {
    require_found(&state, &version)?;
    let url = gmm_core::release_page(&version);
    open_page_as(
        &app,
        &url,
        &url,
        &format!("GMM {version} is on GitHub. Its page has the installer and what changed."),
    )
    .await
}
