// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm detect` and `gmm status`: the setup check (Phase 0b). Read-only.

use std::fmt::Write;
use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_detect::{DetectReport, SystemEnvironment, detect};
use gmm_xxmi::{GameStatus, SetupState, XxmiInstall, evaluate, find_xxmi};
use serde::Serialize;

use crate::AnyError;
use crate::format::bytes;

/// `game=PATH`, for installs detection can't find on its own.
pub fn parse_folder(arg: &str) -> Result<(String, PathBuf), String> {
    let (game, path) = arg
        .split_once('=')
        .ok_or_else(|| format!("expected GAME=FOLDER, got {arg:?}"))?;
    let registry = Registry::builtin().map_err(|e| e.to_string())?;
    let game = registry
        .get(game.trim())
        .ok_or_else(|| format!("unknown game {game:?}"))?;
    Ok((game.id().to_owned(), PathBuf::from(path.trim())))
}

#[derive(Serialize)]
struct DetectOutput<'a> {
    #[serde(flatten)]
    report: &'a DetectReport,
    xxmi: Option<&'a XxmiInstall>,
}

pub fn run_detect(
    json: bool,
    xxmi_dir: Option<PathBuf>,
    folders: &[(String, PathBuf)],
) -> Result<(), AnyError> {
    let registry = Registry::builtin()?;
    let env = SystemEnvironment;
    let report = detect(registry, &env, folders);
    let xxmi = find_xxmi(&env, xxmi_dir.as_deref());
    if json {
        let out = DetectOutput {
            report: &report,
            xxmi: xxmi.as_ref(),
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    let mut out = String::new();
    for game in registry.games() {
        let _ = writeln!(out, "{} ({})", game.game.name, game.id());
        let installs: Vec<_> = report.for_game(game.id()).collect();
        if installs.is_empty() {
            let _ = writeln!(out, "  not found");
        }
        for i in installs {
            let _ = writeln!(out, "  {:<10} {}", i.source.label(), i.folder.display());
        }
    }
    match &xxmi {
        Some(x) => {
            let _ = writeln!(
                out,
                "XXMI Launcher {}\n  {}",
                x.launcher_version().unwrap_or("(version unknown)"),
                x.root.display()
            );
        }
        None => {
            let _ = writeln!(out, "XXMI Launcher\n  not found");
        }
    }
    for note in &report.notes {
        let _ = writeln!(out, "note: {note}");
    }
    print!("{out}");
    Ok(())
}

pub fn run_status(
    json: bool,
    game: Option<&'static GameManifest>,
    xxmi_dir: Option<PathBuf>,
    folders: &[(String, PathBuf)],
) -> Result<(), AnyError> {
    let registry = Registry::builtin()?;
    let env = SystemEnvironment;
    let report = detect(registry, &env, folders);
    let xxmi = find_xxmi(&env, xxmi_dir.as_deref());
    let games: Vec<&GameManifest> = match game {
        Some(g) => vec![g],
        None => registry.games().iter().collect(),
    };
    let statuses: Vec<GameStatus> = games
        .iter()
        .map(|g| evaluate(g, &report.installs, xxmi.as_ref()))
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&statuses)?);
    } else {
        let text: Vec<String> = statuses.iter().map(render).collect();
        print!("{}", text.join("\n"));
    }
    Ok(())
}

fn render(s: &GameStatus) -> String {
    let mut out = String::new();
    let mark = if s.state == SetupState::Ready {
        "OK"
    } else {
        "!!"
    };
    let _ = writeln!(
        out,
        "[{mark}] {} ({}): {}",
        s.game_name,
        s.game_id,
        s.state.label()
    );
    let _ = writeln!(out, "     {}", s.summary);
    if let Some(folder) = &s.xxmi_game_folder {
        let source = s
            .xxmi_game_folder_source
            .map(|src| format!(" ({})", src.label()))
            .unwrap_or_default();
        let _ = writeln!(out, "     game:   {folder}{source}");
    }
    if let Some(root) = &s.xxmi_root {
        let mut line = format!(
            "     xxmi:   {} (v{})",
            root.display(),
            s.xxmi_version.as_deref().unwrap_or("?")
        );
        if let Some(info) = s.importer_info.as_ref().filter(|i| i.folder_exists) {
            if let Some(v) = &info.importer_version {
                let _ = write!(line, " · {} {v}", s.importer);
            }
            if let Some(v) = &info.dll_version {
                let _ = write!(line, " · XXMI DLL {v}");
            }
        }
        let _ = writeln!(out, "{line}");
        if let Some(launch) = &s.game_launch {
            let _ = writeln!(
                out,
                "     launch: XXMI \"Game Launch\" = {launch}, started {} time(s)",
                s.launch_count.unwrap_or(0)
            );
        }
    }
    if let Some(info) = s.importer_info.as_ref().filter(|i| i.folder_exists) {
        let _ = writeln!(
            out,
            "     mods:   {} enabled, {} disabled, {}{}",
            info.mods.enabled,
            info.mods.disabled,
            bytes(info.mods.total_bytes),
            if info.mods.truncated { "+" } else { "" }
        );
    }
    if let Some(cmd) = &s.launch_command {
        let _ = writeln!(out, "     start:  \"{}\" {}", cmd[0], cmd[1..].join(" "));
    }
    if !s.steps.is_empty() {
        let _ = writeln!(out, "     next steps:");
        for (n, step) in s.steps.iter().enumerate() {
            let _ = writeln!(out, "       {}. {step}", n + 1);
        }
    }
    for note in &s.notes {
        let _ = writeln!(out, "     note: {note}");
    }
    out
}
