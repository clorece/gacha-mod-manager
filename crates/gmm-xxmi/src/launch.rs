// SPDX-License-Identifier: MIT OR Apache-2.0

//! Starting XXMI: the only place in GMM that launches a process (security rule R7).
//!
//! The program is always `<XXMI root>/Resources/Bin/XXMI Launcher.exe` of a validated XXMI
//! install, and the arguments are always `--nogui --xxmi <importer>`, built in code from the
//! importer id (the same command XXMI's own "Quick Start" shortcuts use). Nothing from a
//! mod, the catalogue or the database chooses what runs. GMM doesn't wait for, watch or
//! touch the process afterwards. The caller is responsible for the user having confirmed
//! the exe path once (`gmm launch` stores that confirmation in the database).

// The one exception to the `Command::new` ban in clippy.toml.
#![allow(clippy::disallowed_methods)]

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use gmm_adapters::GameManifest;

use crate::find::{LAUNCHER_EXE, XxmiInstall};

/// What [`launch`] would run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    pub exe: PathBuf,
    pub args: [String; 3],
}

/// The command that starts `game` through `xxmi`, after checking that the launcher exe is
/// still there, still a regular file inside the XXMI root, and still starts like a Windows
/// executable.
pub fn launch_command(xxmi: &XxmiInstall, game: &GameManifest) -> Result<LaunchCommand, String> {
    let guard = &xxmi.guard;
    let exe = guard
        .path(xxmi.root_id, LAUNCHER_EXE)
        .map_err(|e| e.to_string())?;
    let meta = guard
        .metadata(&exe)
        .map_err(|_| format!("XXMI Launcher.exe is missing from {}", xxmi.root.display()))?;
    if !meta.is_file() {
        return Err("XXMI Launcher.exe is not a regular file".to_owned());
    }
    let unreadable = |e: &dyn std::fmt::Display| format!("couldn't read XXMI Launcher.exe: {e}");
    let mut head = [0u8; 2];
    guard
        .open_read(&exe)
        .map_err(|e| unreadable(&e))?
        .read_exact(&mut head)
        .map_err(|e| unreadable(&e))?;
    if &head != b"MZ" {
        return Err("XXMI Launcher.exe doesn't look like a Windows program".to_owned());
    }
    Ok(LaunchCommand {
        exe: xxmi.launcher_exe.clone(),
        args: game.xxmi_launch_args(),
    })
}

/// Starts the command and returns the process id. Doesn't wait.
pub fn launch(command: &LaunchCommand) -> Result<u32, String> {
    let dir = command
        .exe
        .parent()
        .map(PathBuf::from)
        .ok_or("XXMI Launcher.exe has no folder")?;
    let child = Command::new(&command.exe)
        .args(&command.args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("couldn't start XXMI Launcher: {e}"))?;
    Ok(child.id())
}
