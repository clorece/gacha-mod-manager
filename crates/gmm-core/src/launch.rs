// SPDX-License-Identifier: MIT OR Apache-2.0

//! "Launch with XXMI" (docs/02-architecture.md, "Launch flow"; security rule R7).
//!
//! XXMI's launcher exe isn't code-signed (observed), so before GMM runs it for the first time
//! the user confirms the exact path, and GMM remembers it. If the path later changes (XXMI
//! moved, or a different folder is picked), it must be confirmed again. The game must be
//! `Ready`: any other state means mods wouldn't load, or the game would close itself.

use std::path::Path;

use gmm_adapters::GameManifest;
use gmm_db::Db;
use gmm_xxmi::{GameStatus, LaunchCommand, SetupState, XxmiInstall, launch, launch_command};

use crate::error::CoreError;
use crate::setup::Machine;

/// A checked launch, ready to run once confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub command: LaunchCommand,
    /// Whether the user already confirmed this exact exe.
    pub confirmed: bool,
}

/// Checks the game's state and the launcher exe, and whether the exe was confirmed.
pub async fn plan_launch(
    db: &Db,
    xxmi: &XxmiInstall,
    status: &GameStatus,
    game: &GameManifest,
) -> Result<LaunchPlan, CoreError> {
    if status.state != SetupState::Ready {
        return Err(CoreError::NotReady {
            game: status.game_name.clone(),
            reason: format!("{} (see `gmm status`)", status.summary),
        });
    }
    let command = launch_command(xxmi, game).map_err(CoreError::Launch)?;
    let confirmed = db
        .confirmed_xxmi_exe()
        .await?
        .is_some_and(|c| same_path(&c, &command.exe));
    Ok(LaunchPlan { command, confirmed })
}

impl Machine {
    /// [`plan_launch`] for `game` with what this scan found.
    pub async fn launch_plan(&self, db: &Db, game: &GameManifest) -> Result<LaunchPlan, CoreError> {
        let xxmi = self.xxmi.as_ref().ok_or_else(|| CoreError::NotReady {
            game: game.game.name.clone(),
            reason: "XXMI Launcher not found".to_owned(),
        })?;
        plan_launch(db, xxmi, &self.status(game), game).await
    }
}

/// Remembers that the user confirmed the plan's exe.
pub async fn confirm_exe(db: &Db, plan: &mut LaunchPlan) -> Result<(), CoreError> {
    db.set_confirmed_xxmi_exe(Some(&plan.command.exe)).await?;
    plan.confirmed = true;
    Ok(())
}

/// Starts XXMI, which then starts the game with mods. Returns the launcher's process id when
/// Windows reports one. On Windows the user may first see XXMI's administrator prompt.
/// Records the time as `game`'s "last played" (shown on the Dashboard).
pub async fn start(
    db: &Db,
    game: &GameManifest,
    plan: &LaunchPlan,
) -> Result<Option<u32>, CoreError> {
    if !plan.confirmed {
        return Err(CoreError::ExeNotConfirmed(plan.command.exe.clone()));
    }
    let pid = launch(&plan.command).map_err(CoreError::Launch)?;
    // The game is starting either way; a failed write only loses the timestamp.
    let _ = db.record_played(game.id()).await;
    Ok(pid)
}

/// Windows paths compare case-insensitively.
fn same_path(a: &Path, b: &Path) -> bool {
    let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
    if cfg!(windows) {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}
