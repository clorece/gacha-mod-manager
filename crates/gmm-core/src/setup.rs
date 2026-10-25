// SPDX-License-Identifier: MIT OR Apache-2.0

//! Finding a game's setup with what the user saved: the XXMI folder they picked and the game
//! folders they added (database), plus anything given for this one call. Shared by the CLI
//! and the app so both see the same machine.

use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::ModProfile;
use gmm_db::Db;
use gmm_detect::{DetectReport, SystemEnvironment, detect};
use gmm_fsguard::RootId;
use gmm_xxmi::{GameStatus, XxmiInstall, evaluate, find_xxmi};

use crate::error::CoreError;
use crate::install::install_gate;
use crate::workspace::{AppDirs, Workspace};

/// Extra hints for one call: an XXMI folder (overrides the saved one) and game folders
/// (added to the saved ones).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupHints {
    pub xxmi: Option<PathBuf>,
    pub folders: Vec<(String, PathBuf)>,
}

/// The XXMI folder to try first and every manual game folder, saved and given.
pub async fn saved_setup(
    db: &Db,
    hints: SetupHints,
) -> Result<(Option<PathBuf>, Vec<(String, PathBuf)>), CoreError> {
    let xxmi = match hints.xxmi {
        Some(dir) => Some(dir),
        None => db.xxmi_root().await?,
    };
    let mut folders: Vec<(String, PathBuf)> = db
        .manual_game_folders()
        .await?
        .into_iter()
        .map(|f| (f.game_id, f.folder))
        .collect();
    folders.extend(hints.folders);
    Ok((xxmi, folders))
}

/// The supported game a GameBanana mod belongs to (from its mod page).
pub fn game_for_mod(profile: &ModProfile) -> Result<&'static GameManifest, CoreError> {
    let registry = Registry::builtin().map_err(|e| CoreError::Io(e.to_string()))?;
    let game_id = profile.summary.game_id;
    game_id
        .and_then(|id| registry.by_gamebanana_id(id))
        .ok_or(CoreError::UnsupportedGame {
            mod_id: profile.summary.id,
            game_id,
        })
}

/// What detection found on this machine.
#[derive(Debug)]
pub struct Machine {
    pub detected: DetectReport,
    pub xxmi: Option<XxmiInstall>,
}

impl Machine {
    /// Runs detection with the saved setup and `hints`. Read-only.
    pub async fn scan(db: &Db, hints: SetupHints) -> Result<Self, CoreError> {
        let (xxmi_dir, folders) = saved_setup(db, hints).await?;
        let env = SystemEnvironment;
        let registry = Registry::builtin().map_err(|e| CoreError::Io(e.to_string()))?;
        Ok(Self {
            detected: detect(registry, &env, &folders),
            xxmi: find_xxmi(&env, xxmi_dir.as_deref()),
        })
    }

    pub fn status(&self, game: &GameManifest) -> GameStatus {
        evaluate(game, &self.detected.installs, self.xxmi.as_ref())
    }
}

/// A game's `Mods/` folder, registered for writing, after the setup gate.
#[derive(Debug)]
pub struct ModsTarget {
    pub ws: Workspace,
    pub mods: RootId,
    pub mods_dir: PathBuf,
    /// Setup problems that don't stop GMM from writing to `Mods/` (see [`install_gate`]).
    pub warnings: Vec<String>,
}

impl ModsTarget {
    /// Finds XXMI and checks the game's setup. Fails when there's no `Mods/` folder to work
    /// with (no XXMI, or the importer isn't installed).
    pub async fn open(db: &Db, game: &GameManifest, hints: SetupHints) -> Result<Self, CoreError> {
        let machine = Machine::scan(db, hints).await?;
        Self::from_machine(&machine, game)
    }

    pub fn from_machine(machine: &Machine, game: &GameManifest) -> Result<Self, CoreError> {
        let warnings = install_gate(&machine.status(game))?;
        let xxmi = machine.xxmi.as_ref().ok_or_else(|| CoreError::NotReady {
            game: game.game.name.clone(),
            reason: "XXMI Launcher not found".to_owned(),
        })?;
        let mods_dir = xxmi.mods_dir(game.xxmi.importer);
        let mut ws = Workspace::open(&AppDirs::system()?.cache)?;
        let mods = ws.add_mods_root(&mods_dir)?;
        Ok(Self {
            ws,
            mods,
            mods_dir,
            warnings,
        })
    }
}
