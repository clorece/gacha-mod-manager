// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where things live on this machine. Behind a trait so tests can point every source at a
//! temporary folder. This is the only module that reads the Windows registry, and it reads only
//! the values listed in docs/security.md (R3).

// The registry key type is banned elsewhere (clippy.toml); this module is the exception.
#![allow(clippy::disallowed_types)]

use std::path::PathBuf;

pub trait Environment {
    /// Steam's install folder.
    fn steam_root(&self) -> Option<PathBuf>;
    /// The roaming app data folder (`%APPDATA%`): HoYoPlay's data, XXMI's default location.
    fn roaming_app_data(&self) -> Option<PathBuf>;
    /// `%PROGRAMDATA%`: Epic's install manifests.
    fn program_data(&self) -> Option<PathBuf>;
    /// `InstallLocation` of uninstall entries named "XXMI Launcher".
    fn xxmi_install_locations(&self) -> Vec<PathBuf>;
}

/// The real machine.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn steam_root(&self) -> Option<PathBuf> {
        registry::steam_path().or_else(default_steam_root)
    }

    fn roaming_app_data(&self) -> Option<PathBuf> {
        dirs::config_dir()
    }

    fn program_data(&self) -> Option<PathBuf> {
        std::env::var_os("PROGRAMDATA").map(PathBuf::from)
    }

    fn xxmi_install_locations(&self) -> Vec<PathBuf> {
        registry::xxmi_install_locations()
    }
}

#[cfg(windows)]
fn default_steam_root() -> Option<PathBuf> {
    Some(PathBuf::from(r"C:\Program Files (x86)\Steam"))
}

#[cfg(not(windows))]
fn default_steam_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".steam").join("steam"))
}

#[cfg(windows)]
mod registry {
    use std::path::PathBuf;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};

    /// `HKCU\Software\Valve\Steam\SteamPath` (stored with forward slashes).
    pub(super) fn steam_path() -> Option<PathBuf> {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(r"Software\Valve\Steam", KEY_READ)
            .ok()?;
        let value: String = key.get_value("SteamPath").ok()?;
        let value = value.trim();
        (!value.is_empty()).then(|| PathBuf::from(value.replace('/', "\\")))
    }

    /// Uninstall entries whose `DisplayName` is exactly "XXMI Launcher". Only
    /// `InstallLocation` is read from them.
    pub(super) fn xxmi_install_locations() -> Vec<PathBuf> {
        let mut out = Vec::new();
        for (hive, path) in [
            (
                HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
        ] {
            let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ) else {
                continue;
            };
            for name in root.enum_keys().flatten() {
                let Ok(entry) = root.open_subkey_with_flags(&name, KEY_READ) else {
                    continue;
                };
                let display: String = entry.get_value("DisplayName").unwrap_or_default();
                if display != "XXMI Launcher" {
                    continue;
                }
                let location: String = entry.get_value("InstallLocation").unwrap_or_default();
                let location = PathBuf::from(location.trim());
                if !location.as_os_str().is_empty() && !out.contains(&location) {
                    out.push(location);
                }
            }
        }
        out
    }
}

#[cfg(not(windows))]
mod registry {
    use std::path::PathBuf;

    pub(super) fn steam_path() -> Option<PathBuf> {
        None
    }

    pub(super) fn xxmi_install_locations() -> Vec<PathBuf> {
        Vec::new()
    }
}
