// SPDX-License-Identifier: MIT OR Apache-2.0

//! The per-game setup state machine (docs/game-adapters.md, "Per-importer state"). Checks
//! run in order and the first failing one decides the state, so the user always gets the one
//! next step that matters. Wording of the steps is GMM's own; quoted names are XXMI's labels.

use std::path::PathBuf;

use gmm_adapters::GameManifest;
use gmm_detect::{GameInstall, InstallSource, is_game_folder, resolve_game_folder, same_path};
use serde::Serialize;

use crate::find::XxmiInstall;
use crate::importer::{ImporterInfo, inspect_importer};

/// XXMI Launcher's official releases page (the setup steps and the app link to it).
pub const XXMI_RELEASES: &str = "https://github.com/SpectrumQT/XXMI-Launcher/releases";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SetupState {
    XxmiNotFound,
    ImporterMissing,
    ImporterBroken,
    GamePathNotSet,
    PathMismatch,
    LaunchModeMismatch,
    Ready,
}

impl SetupState {
    pub fn label(self) -> &'static str {
        match self {
            Self::XxmiNotFound => "XXMI not found",
            Self::ImporterMissing => "importer not installed",
            Self::ImporterBroken => "importer damaged",
            Self::GamePathNotSet => "game folder not set in XXMI",
            Self::PathMismatch => "game folder in XXMI is wrong",
            Self::LaunchModeMismatch => "XXMI launch setting doesn't fit this install",
            Self::Ready => "ready",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GameStatus {
    pub game_id: String,
    pub game_name: String,
    pub state: SetupState,
    /// One sentence for the user.
    pub summary: String,
    /// What to do next, in order. Empty when ready.
    pub steps: Vec<String>,
    /// Extra information that doesn't block anything.
    pub notes: Vec<String>,
    pub detected_installs: Vec<GameInstall>,
    pub xxmi_root: Option<PathBuf>,
    pub xxmi_version: Option<String>,
    pub importer: String,
    pub importer_info: Option<ImporterInfo>,
    /// The game folder XXMI is configured with.
    pub xxmi_game_folder: Option<String>,
    /// Which detected install that folder is, if any.
    pub xxmi_game_folder_source: Option<InstallSource>,
    /// XXMI's "Game Launch" setting, raw (`DIRECT`, `STEAM`, ...).
    pub game_launch: Option<String>,
    pub launch_count: Option<i64>,
    /// Executable and arguments GMM would run to start the game with mods (only when ready).
    pub launch_command: Option<Vec<String>>,
}

/// Works out `game`'s setup state. `installs` may include other games' installs; only this
/// game's are used.
pub fn evaluate(
    game: &GameManifest,
    installs: &[GameInstall],
    xxmi: Option<&XxmiInstall>,
) -> GameStatus {
    let name = game.game.name.clone();
    let importer = game.xxmi.importer.as_str();
    let detected: Vec<GameInstall> = installs
        .iter()
        .filter(|i| i.game_id == game.id())
        .cloned()
        .collect();
    let mut s = GameStatus {
        game_id: game.id().to_owned(),
        game_name: name.clone(),
        state: SetupState::Ready,
        summary: String::new(),
        steps: Vec::new(),
        notes: Vec::new(),
        detected_installs: detected.clone(),
        xxmi_root: None,
        xxmi_version: None,
        importer: importer.to_owned(),
        importer_info: None,
        xxmi_game_folder: None,
        xxmi_game_folder_source: None,
        game_launch: None,
        launch_count: None,
        launch_command: None,
    };
    if detected.is_empty() {
        s.notes.push(format!(
            "GMM didn't find {name} on this PC (checked Steam, HoYoPlay, Epic)."
        ));
    }

    // 1. XXMI itself.
    let Some(xxmi) = xxmi else {
        return s.with(
            SetupState::XxmiNotFound,
            "XXMI Launcher isn't installed (or GMM can't find it).".to_owned(),
            vec![
                format!("Download XXMI Launcher from its official releases page: {XXMI_RELEASES}"),
                "Install it (the .msi), or extract the portable .zip somewhere outside Program \
                 Files, then start it once."
                    .to_owned(),
                "Run `gmm status` again. For a portable copy, add `--xxmi <its folder>`."
                    .to_owned(),
            ],
        );
    };
    s.xxmi_root = Some(xxmi.root.clone());
    s.xxmi_version = xxmi.launcher_version().map(str::to_owned);
    if let Some(e) = &xxmi.config_error {
        s.notes.push(format!("Couldn't read XXMI's settings: {e}"));
    }
    let config = xxmi.config.importer(importer);
    s.xxmi_game_folder = config.and_then(|c| c.game_folder.clone());
    s.game_launch = config.and_then(|c| c.game_launch.clone());
    s.launch_count = config.and_then(|c| c.launch_count);

    // 2. The importer folder.
    let info = inspect_importer(xxmi, game);
    s.importer_info = Some(info.clone());
    if !info.folder_exists || !info.missing_files.is_empty() {
        if info.folder_exists {
            s.notes.push(format!(
                "{importer} folder is incomplete, missing: {}",
                info.missing_files.join(", ")
            ));
        }
        return s.with(
            SetupState::ImporterMissing,
            format!("XXMI's {importer} (the {name} mod loader) isn't installed."),
            vec![
                "Open XXMI Launcher.".to_owned(),
                format!(
                    "Under \"Select Games To Mod:\", tick {name} ({importer}) and press \"Install\"."
                ),
                "Run `gmm status` again.".to_owned(),
            ],
        );
    }
    let target_ok = info
        .d3dx_target
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case(&game.xxmi.d3dx_target));
    if !target_ok || !info.d3dx_loads_mods {
        let problem = if target_ok {
            "doesn't load the Mods folder".to_owned()
        } else {
            format!(
                "targets {:?} instead of {}",
                info.d3dx_target.as_deref().unwrap_or("nothing"),
                game.xxmi.d3dx_target
            )
        };
        return s.with(
            SetupState::ImporterBroken,
            format!("XXMI's {importer} configuration looks changed: its d3dx.ini {problem}."),
            vec![
                format!("Open XXMI Launcher and reinstall or update {importer}."),
                "Run `gmm status` again.".to_owned(),
            ],
        );
    }

    // 3. The game folder XXMI uses.
    let suggestion = detected.first().map(|i| i.folder.display().to_string());
    let folder_steps = |intro: String| {
        let mut steps = vec![
            format!(
                "Open XXMI Launcher, select {name}, open the settings and the \"General\" tab."
            ),
            intro,
        ];
        steps.push(match &suggestion {
            Some(path) => format!("Set \"Game Folder\" to: {path}"),
            None => format!(
                "Press \"Detect Installations\", or set \"Game Folder\" to the folder that contains {}.",
                game.detect.exe_names[0]
            ),
        });
        steps.push("Press \"Start\" once, then run `gmm status` again.".to_owned());
        steps
    };
    let Some(folder) = s.xxmi_game_folder.clone() else {
        let steps = folder_steps(format!("XXMI doesn't know where {name} is yet."));
        return s.with(
            SetupState::GamePathNotSet,
            format!("XXMI doesn't have a game folder for {name} yet."),
            steps,
        );
    };
    let folder_path = PathBuf::from(&folder);
    if !is_game_folder(game, &folder_path) {
        let steps = folder_steps(format!(
            "The current \"Game Folder\" ({folder}) doesn't contain {name}."
        ));
        return s.with(
            SetupState::PathMismatch,
            format!("XXMI's game folder for {name} doesn't contain the game: {folder}"),
            steps,
        );
    }
    // Detected installs are canonical; XXMI's setting may name the same folder differently
    // (8.3 short names, `/` separators, a trailing separator).
    let canonical = resolve_game_folder(game, &folder_path).unwrap_or(folder_path);
    let source = detected
        .iter()
        .find(|i| same_path(&i.folder, &canonical))
        .map(|i| i.source);
    s.xxmi_game_folder_source = source;
    for other in detected
        .iter()
        .filter(|i| !same_path(&i.folder, &canonical))
    {
        s.notes.push(format!(
            "Another {name} install was found ({}): {}. XXMI uses {folder}.",
            other.source.label(),
            other.folder.display()
        ));
    }

    // 4. Does XXMI's "Game Launch" fit how this copy must be started?
    let game_launch = s.game_launch.clone();
    match (source, game_launch.as_deref()) {
        (Some(source), Some(launch)) => {
            let steam_direct_ok = game.detect.steam.as_ref().is_some_and(|s| s.direct_launch);
            if let Some(expected) = launch_mismatch(source, launch, steam_direct_ok) {
                return s.with(
                    SetupState::LaunchModeMismatch,
                    format!(
                        "{name} here is a {} install, but XXMI's \"Game Launch\" is set to {}. \
                         The game will close itself when XXMI starts it this way.",
                        source.label(),
                        launch_label(launch)
                    ),
                    vec![
                        format!(
                            "In XXMI Launcher: select {name}, open the settings, \"General\" tab, \
                             set \"Game Launch\" to \"{expected}\"."
                        ),
                        "Run `gmm status` again.".to_owned(),
                    ],
                );
            }
        }
        (None, _) => s.notes.push(format!(
            "XXMI's game folder ({folder}) isn't one GMM detected, so its launch setting wasn't \
             checked."
        )),
        (Some(_), None) => {}
    }

    // 5. Ready.
    if s.launch_count == Some(0) {
        s.notes.push(format!(
            "XXMI hasn't started {name} yet. The first start also applies the game settings \
             mods need."
        ));
    }
    let mut command = vec![xxmi.launcher_exe.display().to_string()];
    command.extend(game.xxmi_launch_args());
    s.launch_command = Some(command);
    s.with(
        SetupState::Ready,
        format!("Ready: start {name} through XXMI and mods load."),
        Vec::new(),
    )
}

impl GameStatus {
    fn with(mut self, state: SetupState, summary: String, steps: Vec<String>) -> Self {
        self.state = state;
        self.summary = summary;
        self.steps = steps;
        self
    }
}

/// The label XXMI's "Game Launch" should be set to, if the current value can't work for this
/// kind of install. Manual (XXMI waits for the game) works for every source. `steam_direct_ok`
/// is the game's manifest flag `[detect.steam] direct_launch`: some Steam copies (WuWa) also
/// run when XXMI starts them directly, others (ZZZ's, which wraps HoYoPlay) close themselves.
fn launch_mismatch(
    source: InstallSource,
    launch: &str,
    steam_direct_ok: bool,
) -> Option<&'static str> {
    let launch = launch.to_ascii_uppercase();
    match source {
        InstallSource::Steam => {
            let ok = matches!(launch.as_str(), "STEAM" | "MANUAL")
                || (steam_direct_ok && launch == "DIRECT");
            (!ok).then_some("Steam")
        }
        InstallSource::Epic => {
            (!matches!(launch.as_str(), "EPIC_GAMES" | "EPIC" | "MANUAL")).then_some("Epic Games")
        }
        InstallSource::HoYoPlay | InstallSource::Manual => {
            matches!(launch.as_str(), "STEAM" | "EPIC_GAMES" | "EPIC").then_some("Direct")
        }
    }
}

fn launch_label(raw: &str) -> String {
    match raw.to_ascii_uppercase().as_str() {
        "DIRECT" => "\"Direct\"".to_owned(),
        "STEAM" => "\"Steam\"".to_owned(),
        "EPIC_GAMES" | "EPIC" => "\"Epic Games\"".to_owned(),
        "CUSTOM" => "\"Custom\"".to_owned(),
        "MANUAL" => "\"Manual\"".to_owned(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_rules() {
        use InstallSource::{Epic, HoYoPlay, Manual, Steam};
        // A Steam copy that only runs when Steam starts it (ZZZ).
        assert_eq!(launch_mismatch(Steam, "DIRECT", false), Some("Steam"));
        assert_eq!(launch_mismatch(Steam, "STEAM", false), None);
        assert_eq!(launch_mismatch(Steam, "MANUAL", false), None);
        // One that XXMI may start directly (WuWa).
        assert_eq!(launch_mismatch(Steam, "DIRECT", true), None);
        assert_eq!(launch_mismatch(Steam, "STEAM", true), None);
        assert_eq!(launch_mismatch(Steam, "EPIC_GAMES", true), Some("Steam"));
        assert_eq!(launch_mismatch(Epic, "DIRECT", false), Some("Epic Games"));
        assert_eq!(launch_mismatch(Epic, "EPIC_GAMES", false), None);
        assert_eq!(launch_mismatch(HoYoPlay, "DIRECT", false), None);
        assert_eq!(launch_mismatch(HoYoPlay, "STEAM", false), Some("Direct"));
        assert_eq!(launch_mismatch(Manual, "CUSTOM", false), None);
    }
}
