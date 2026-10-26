// SPDX-License-Identifier: MIT OR Apache-2.0

//! Report a Bug (Phase 6): the local error log (read, clear, open its folder, errors from the
//! webview) and the pre-filled GitHub issue. Rust builds the issue's text and address
//! (`gmm_core::bug_report`); the webview only sends what the user typed. Opening asks first
//! in a native dialog, like every other page (`open.rs`); GMM never connects to GitHub.

use std::sync::atomic::{AtomicU32, Ordering};

use gmm_core::{BugReport, LogEntry, Machine, SetupHints};
use tauri::{AppHandle, Runtime, State};

use crate::AppState;
use crate::commands::registry;
use crate::error::{CmdError, CmdResult};

/// Most entries `error_log` returns.
const MAX_ENTRIES: usize = 500;
/// Longest description accepted (the issue link holds far less; the rest is cut).
const MAX_DESCRIPTION_CHARS: usize = 20_000;
/// Longest error message the webview may log, and how many per run.
const MAX_UI_ERROR_CHARS: usize = 300;
const MAX_UI_ERRORS: u32 = 50;

static UI_ERRORS: AtomicU32 = AtomicU32::new(0);

/// The newest entries of the error log, oldest first.
#[tauri::command]
pub fn error_log(state: State<'_, AppState>, limit: u32) -> Vec<LogEntry> {
    state.errors.entries((limit as usize).min(MAX_ENTRIES))
}

/// Deletes the log for good; returns how many entries went.
#[tauri::command]
pub fn clear_error_log(state: State<'_, AppState>) -> usize {
    state.errors.clear()
}

/// Shows the log's folder in the file manager, for attaching `errors.log` to an issue.
#[tauri::command]
pub fn open_log_folder(state: State<'_, AppState>) -> CmdResult<()> {
    let dir = state.errors.folder()?;
    gmm_xxmi::open_folder(&dir).map_err(CmdError::other)
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

async fn draft(state: &AppState, title: &str, description: &str) -> CmdResult<BugReport> {
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
    Ok(gmm_core::bug_report(
        title,
        &state.errors.redact(description),
        &details,
    ))
}

/// What the issue will say and where it opens, for the preview.
#[tauri::command]
pub async fn bug_report_draft(
    state: State<'_, AppState>,
    title: String,
    description: String,
) -> CmdResult<BugReport> {
    draft(&state, &title, &description).await
}

/// Opens the pre-filled issue in the browser after the native confirmation. Returns false
/// when the user cancelled.
#[tauri::command]
pub async fn open_bug_report<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    title: String,
    description: String,
) -> CmdResult<bool> {
    let report = draft(&state, &title, &description).await?;
    gmm_xxmi::check_page_url(&report.url).map_err(CmdError::other)?;
    crate::open::open_page_as(
        &app,
        &report.url,
        &format!("{} (with your report filled in)", gmm_core::ISSUES_URL),
        "This opens a new issue on GMM's GitHub page with your report filled in. Nothing is \
         sent until you submit it there (you need a GitHub account).",
    )
    .await
}
