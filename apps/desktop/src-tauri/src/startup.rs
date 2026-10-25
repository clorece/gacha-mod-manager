// SPDX-License-Identifier: MIT OR Apache-2.0

//! The startup check (docs/08, "Startup check"): the GMM app step (skipped until the release
//! updater exists), the newest catalogue page per game, and an update check of every
//! GMM-installed mod. Each step is reported on the channel as it starts and ends; a failing
//! step (offline, say) doesn't stop the others, and the app works on with cached data.

use gmm_core::{check_updates, refresh_catalog};
use tauri::State;
use tauri::ipc::Channel;

use crate::AppState;
use crate::commands::registry;
use crate::error::CmdResult;
use crate::views::{StartupReport, StartupStep, StepState};

#[tauri::command]
pub async fn startup_check(
    state: State<'_, AppState>,
    on_step: Channel<StartupStep>,
) -> CmdResult<StartupReport> {
    let games = registry()?.games();
    let mut step = 0u32;
    let send = |step: u32, label: &str, state: StepState, detail: String| {
        // The window may be gone; the check carries on regardless.
        let _ = on_step.send(StartupStep {
            step,
            label: label.to_owned(),
            state,
            detail,
        });
    };

    send(
        step,
        "GMM app",
        StepState::Skipped,
        "Update checks arrive with the signed release builds".to_owned(),
    );

    for game in games {
        step += 1;
        let label = format!("{} mods", game.game.name);
        send(
            step,
            &label,
            StepState::Working,
            "Reading the newest pages".to_owned(),
        );
        match refresh_catalog(&state.gb, game).await {
            Ok(n) => send(
                step,
                &label,
                StepState::Done,
                format!("{n} newest mods pulled"),
            ),
            Err(e) => send(step, &label, StepState::Failed, e.to_string()),
        }
    }

    step += 1;
    let label = "Your installed mods";
    send(
        step,
        label,
        StepState::Working,
        "Comparing with GameBanana".to_owned(),
    );
    let mut report = StartupReport::default();
    for game in games {
        match check_updates(&state.gb, &state.db, game).await {
            Ok(check) => {
                report.checked += check.checked;
                report.updates.extend(check.updates);
                report.notes.extend(check.notes);
            }
            Err(e) => report.notes.push(format!("{}: {e}", game.game.name)),
        }
    }
    let detail = match report.updates.len() {
        0 => format!("{} checked, all up to date", report.checked),
        1 => "1 update available".to_owned(),
        n => format!("{n} updates available"),
    };
    send(step, label, StepState::Done, detail);
    Ok(report)
}
