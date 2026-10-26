// SPDX-License-Identifier: MIT OR Apache-2.0

//! Starting XXMI, and handing a web page or a folder to the OS shell: the only place in GMM
//! that launches a process (security rule R7).
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
//! the default verb, which shows the usual UAC prompt.
//!
//! [`open_page`] and [`open_folder`] hand an address to the user's browser or a folder to the
//! file manager (Windows: `ShellExecuteExW` with the `open` / `explore` verb; elsewhere
//! `xdg-open` with the one argument). They never run a file: a page must be an `http(s)`
//! address, a folder must be an existing directory.
//!
//! [`game_running`] looks at running process names (Windows ToolHelp snapshot), so GMM can
//! hold off writing `d3dx_user.ini` while the game is open.

use std::io::Read;
use std::path::{Path, PathBuf};

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

/// Longest address [`open_page`] accepts.
pub const MAX_PAGE_URL: usize = 2048;

/// Opens a web page in the user's default browser. The caller has shown the address and the
/// user agreed. Only `https://` and `http://` addresses, without spaces, quotes or control
/// characters, up to [`MAX_PAGE_URL`] long.
pub fn open_page(url: &str) -> Result<(), String> {
    check_page_url(url)?;
    platform::shell_open(Shell::Page, url.as_ref())
}

/// What [`open_page`] accepts, without opening anything (bug report addresses are checked
/// against it when they're built).
pub fn check_page_url(url: &str) -> Result<(), String> {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    let scheme_ok = lower.starts_with("https://") || lower.starts_with("http://");
    let host_ok = url
        .split_once("://")
        .is_some_and(|(_, rest)| !rest.is_empty() && !rest.starts_with('/'));
    if !scheme_ok
        || !host_ok
        || url.len() > MAX_PAGE_URL
        || url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '<' || c == '>')
    {
        return Err("only plain web addresses can be opened".to_owned());
    }
    Ok(())
}

/// Opens a folder in the file manager. Refuses anything but an absolute path to an existing
/// directory (security rule R7: folders only, never a file).
pub fn open_folder(dir: &Path) -> Result<(), String> {
    if !dir.is_absolute() || !dir.is_dir() {
        return Err(format!("{} is not a folder", dir.display()));
    }
    platform::shell_open(Shell::Folder, dir.as_os_str())
}

/// Whether one of the game's executables is running. Only process image names are
/// compared (nothing else about other processes is read or kept). GMM writes 3DMigoto's
/// `d3dx_user.ini` only while this is false, because the running game rewrites that file
/// from memory. Outside Windows it is always false for now (Linux support comes later).
pub fn game_running(game: &GameManifest) -> bool {
    platform::any_running(&game.detect.exe_names)
}

#[derive(Clone, Copy)]
enum Shell {
    Page,
    Folder,
}

#[cfg(windows)]
mod platform {
    // The single exception to `unsafe_code = "deny"` (CI checks that no other file allows it).
    #![allow(unsafe_code)]

    use std::ffi::OsStr;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::GetProcessId;
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    use super::{LaunchCommand, Shell};

    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    }

    pub fn shell_open(what: Shell, target: &OsStr) -> Result<(), String> {
        // `explore` only applies to folders, so a file swapped in can't be run.
        let verb = wide(OsStr::new(match what {
            Shell::Page => "open",
            Shell::Folder => "explore",
        }));
        let file = wide(target);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
            fMask: SEE_MASK_NOASYNC,
            lpVerb: verb.as_ptr(),
            lpFile: file.as_ptr(),
            nShow: SW_SHOWNORMAL,
            ..SHELLEXECUTEINFOW::default()
        };
        // SAFETY: as in `start`: `info` is fully initialised, `cbSize` matches, and the
        // string pointers refer to NUL-terminated buffers that outlive the call. No process
        // handle is requested, so there is nothing to close.
        let ok = unsafe {
            let _ = CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            );
            ShellExecuteExW(&mut info)
        };
        if ok == 0 {
            return Err(format!("couldn't open it: {}", io::Error::last_os_error()));
        }
        Ok(())
    }

    /// Walks a snapshot of the running processes and compares each image name.
    pub fn any_running(exe_names: &[String]) -> bool {
        // SAFETY: the snapshot handle is checked, used only with the ToolHelp calls below and
        // closed exactly once; `entry` is zero-initialised with `dwSize` set as the API
        // requires, and `szExeFile` is read only up to its NUL within the fixed buffer.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return false;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0);
            let mut found = false;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more && !found {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                found = exe_names.iter().any(|n| n.eq_ignore_ascii_case(&name));
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            found
        }
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

    use std::ffi::OsStr;
    use std::path::Path;
    use std::process::{Command, Stdio};

    use super::{LaunchCommand, Shell};

    /// `xdg-open` with the address or folder as its only argument (both start with
    /// `http` or `/`, so neither reads as an option).
    pub fn shell_open(_what: Shell, target: &OsStr) -> Result<(), String> {
        Command::new("xdg-open")
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|mut child| {
                // Reap it when it exits (xdg-open hands off and quits).
                std::thread::spawn(move || child.wait());
            })
            .map_err(|e| format!("couldn't open it: {e}"))
    }

    pub fn any_running(_exe_names: &[String]) -> bool {
        false
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_web_addresses_are_opened() {
        for bad in [
            "",
            "file:///C:/Windows/System32/calc.exe",
            r"C:\Windows\notepad.exe",
            "javascript:alert(1)",
            "ms-settings:privacy",
            "https://",
            "https:///etc/passwd",
            "https://gamebanana.com/mods/1 --flag",
            "https://gamebanana.com/\"quoted\"",
            "https://gamebanana.com/
",
            "-https://gamebanana.com",
        ] {
            assert!(open_page(bad).is_err(), "{bad:?}");
        }
        let long = format!("https://gamebanana.com/{}", "a".repeat(MAX_PAGE_URL));
        assert!(open_page(&long).is_err());
    }

    #[test]
    #[allow(clippy::disallowed_methods)] // writing the test file
    fn folders_only() {
        assert!(open_folder(Path::new("relative/dir")).is_err());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("mod.ini");
        std::fs::write(&file, b"x").unwrap();
        assert!(open_folder(&file).is_err());
        assert!(open_folder(&dir.path().join("missing")).is_err());
    }
}
