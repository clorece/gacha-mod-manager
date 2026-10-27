// SPDX-License-Identifier: MIT OR Apache-2.0

//! Report a Bug (Phase 6): the log file to attach (the newest, or one the user picks from the
//! log folder), errors from the webview, and the pre-filled GitHub issue. Rust builds the
//! issue's text and address (`gmm_core::bug_report`); the webview only sends what the user
//! typed. Opening asks first in a native dialog, like every other page (`open.rs`); GMM never
//! connects to GitHub. A link can't carry files, so after the issue opens GMM shows the log
//! folder for the user to drag the file in.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};

use gmm_core::{LogFile, Machine, SetupHints};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::AppState;
use crate::commands::registry;
use crate::error::{CmdError, CmdResult};

/// Longest description accepted (the issue link holds far less; the rest is cut).
const MAX_DESCRIPTION_CHARS: usize = 20_000;
/// Longest error message the webview may log, and how many per run.
const MAX_UI_ERROR_CHARS: usize = 300;
const MAX_UI_ERRORS: u32 = 50;

static UI_ERRORS: AtomicU32 = AtomicU32::new(0);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The log file the report attaches: the one the user picked while it's still there, else the
/// newest. `None` when nothing has been logged.
fn chosen_log(state: &AppState) -> Option<LogFile> {
    let picked = lock(&state.report_log).clone();
    picked
        .and_then(|name| state.errors.file(&name).ok())
        .or_else(|| state.errors.newest_file())
}

/// The log file the report attaches.
#[tauri::command]
pub fn report_log(state: State<'_, AppState>) -> Option<LogFile> {
    chosen_log(&state)
}

/// A file dialog in the log folder for another log file to attach. Returns it, or `None` when
/// cancelled; a file from anywhere else is refused.
#[tauri::command]
pub async fn pick_report_log<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> CmdResult<Option<LogFile>> {
    let folder = state.errors.folder()?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose a log file")
        .set_directory(folder)
        .add_filter("Log files", &["log"])
        .pick_file(move |file| {
            let _ = tx.send(file.and_then(|f| f.into_path().ok()));
        });
    let Some(path) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let file = state.errors.file_at(&path)?;
    *lock(&state.report_log) = Some(file.name.clone());
    Ok(Some(file))
}

/// An error in the webview (an exception or a failed promise nobody handled). Untrusted
/// text: cut short, and only so many per run.
#[tauri::command]
pub fn log_ui_error(state: State<'_, AppState>, message: String) {
    if UI_ERRORS.fetch_add(1, Ordering::Relaxed) >= MAX_UI_ERRORS {
        return;
    }
    let short: String = message.chars().take(MAX_UI_ERROR_CHARS).collect();
    state.errors.record("ui", &short);
}

/// Opens the pre-filled issue in the browser after the native confirmation, then shows the
/// log folder so the chosen log can be dragged in. Returns false when the user cancelled.
#[tauri::command]
pub async fn open_bug_report<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    title: String,
    description: String,
) -> CmdResult<bool> {
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(CmdError::other("that description is too long for a report"));
    }
    let machine = Machine::scan(&state.db, SetupHints::default()).await?;
    let statuses: Vec<_> = registry()?
        .games()
        .iter()
        .map(|g| machine.status(g))
        .collect();
    let details = state.errors.redact(&gmm_core::system_details(&statuses));
    let log = chosen_log(&state);
    let report = gmm_core::bug_report(
        &title,
        &state.errors.redact(&description),
        &details,
        log.as_ref().map(|f| f.name.as_str()),
    );
    gmm_xxmi::check_page_url(&report.url).map_err(CmdError::other)?;
    let log_note = match &log {
        Some(f) => format!(
            " GMM then shows its log folder, so you can drag {} into the issue.",
            f.name
        ),
        None => String::new(),
    };
    let opened = crate::open::open_page_as(
        &app,
        &report.url,
        &format!("{} (with your report filled in)", gmm_core::ISSUES_URL),
        &format!(
            "This opens a new issue on GMM's GitHub page with your report filled in. Nothing is \
             sent until you submit it there (you need a GitHub account).{log_note}"
        ),
    )
    .await?;
    if opened && log.is_some() {
        let dir = state.errors.folder()?;
        gmm_xxmi::open_folder(&dir).map_err(CmdError::other)?;
    }
    Ok(opened)
}
