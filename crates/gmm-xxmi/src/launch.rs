// SPDX-License-Identifier: MIT OR Apache-2.0

//! Starting XXMI: the only place in GMM that launches a process (security rule R7).
//!
//! The program is always `<XXMI root>/Resources/Bin/XXMI Launcher.exe` of a validated XXMI
//! install, and the arguments are always `--nogui --xxmi <importer>`, built in code from the
//! importer id (the same command XXMI's own "Quick Start" shortcuts use). Nothing from a
//! mod, the catalogue or the database chooses what runs. GMM doesn't wait for, watch or
//! touch the process afterwards. The caller is responsible for the user having confirmed
//! the exe path once (`gmm launch` stores that confirmation in the database).
//!
//! XXMI's launcher asks to run as administrator (its manifest says `requireAdministrator`,
//! observed 2026-10-24), and a plain process start can't do that (Windows error 740). On
//! Windows it is therefore started the way Explorer starts a shortcut: `ShellExecuteExW` with
//! the default verb, which shows the usual UAC prompt. This is the one `unsafe` block in GMM.

use std::io::Read;
use std::path::PathBuf;

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

/// Starts the command and returns the process id (if Windows reports one). Doesn't wait.
pub fn launch(command: &LaunchCommand) -> Result<Option<u32>, String> {
    let dir = command
        .exe
        .parent()
        .map(PathBuf::from)
        .ok_or("XXMI Launcher.exe has no folder")?;
    // The arguments are fixed tokens from code; refuse anything that would need quoting.
    if command
        .args
        .iter()
        .any(|a| a.is_empty() || a.chars().any(|c| c.is_whitespace() || c == '"'))
    {
        return Err("unexpected XXMI launch arguments".to_owned());
    }
    platform::start(command, &dir)
}

#[cfg(windows)]
mod platform {
    // The single exception to `unsafe_code = "deny"` (CI checks that no other file allows it).
    #![allow(unsafe_code)]

    use std::ffi::OsStr;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx,
    };
    use windows_sys::Win32::System::Threading::GetProcessId;
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    use super::LaunchCommand;

    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    }

    pub fn start(command: &LaunchCommand, dir: &Path) -> Result<Option<u32>, String> {
        let file = wide(command.exe.as_os_str());
        let params = wide(OsStr::new(&command.args.join(" ")));
        let directory = wide(dir.as_os_str());
        let mut info = SHELLEXECUTEINFOW {
            cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
            fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
            lpFile: file.as_ptr(),
            lpParameters: params.as_ptr(),
            lpDirectory: directory.as_ptr(),
            nShow: SW_SHOWNORMAL,
            ..SHELLEXECUTEINFOW::default()
        };
        // SAFETY: `info` is fully initialised, `cbSize` matches the struct, and every string
        // pointer refers to a NUL-terminated buffer that outlives the call. COM must be
        // initialised on this thread for ShellExecuteExW; a second initialisation (or one in
        // another mode) is harmless and its result is ignored.
        let ok = unsafe {
            let _ = CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            );
            ShellExecuteExW(&mut info)
        };
        if ok == 0 {
            let err = io::Error::last_os_error();
            return Err(match err.raw_os_error() {
                Some(code) if code == ERROR_CANCELLED as i32 => {
                    "XXMI Launcher wasn't started: the administrator prompt was declined".to_owned()
                }
                _ => format!("couldn't start XXMI Launcher: {err}"),
            });
        }
        if info.hProcess.is_null() {
            return Ok(None);
        }
        // SAFETY: `hProcess` is a valid process handle returned because of
        // SEE_MASK_NOCLOSEPROCESS; it is closed exactly once here.
        let pid = unsafe {
            let pid = GetProcessId(info.hProcess);
            CloseHandle(info.hProcess);
            pid
        };
        Ok((pid != 0).then_some(pid))
    }
}

#[cfg(not(windows))]
mod platform {
    // The one exception to the `Command::new` ban in clippy.toml.
    #![allow(clippy::disallowed_methods)]

    use std::path::Path;
    use std::process::{Command, Stdio};

    use super::LaunchCommand;

    pub fn start(command: &LaunchCommand, dir: &Path) -> Result<Option<u32>, String> {
        let child = Command::new(&command.exe)
            .args(&command.args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("couldn't start XXMI Launcher: {e}"))?;
        Ok(Some(child.id()))
    }
}
