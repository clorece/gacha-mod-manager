// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::Read;
use std::path::{Path, PathBuf};

use gmm_detect::Environment;
use gmm_fsguard::{Access, Guard, RootId};
use serde::Serialize;

use crate::config::XxmiConfig;

const CONFIG_FILE: &str = "XXMI Launcher Config.json";
const LAUNCHER_EXE: &str = "Resources/Bin/XXMI Launcher.exe";
const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

/// A validated XXMI Launcher folder.
#[derive(Debug, Serialize)]
pub struct XxmiInstall {
    /// Canonical root folder.
    pub root: PathBuf,
    /// `<root>/Resources/Bin/XXMI Launcher.exe`. Not code-signed (observed), so the user
    /// confirms it before GMM ever launches it.
    pub launcher_exe: PathBuf,
    /// Set when the config file couldn't be read; the root is still valid.
    pub config_error: Option<String>,
    pub config: XxmiConfig,
    #[serde(skip)]
    pub(crate) guard: Guard,
    #[serde(skip)]
    pub(crate) root_id: RootId,
}

impl XxmiInstall {
    pub fn launcher_version(&self) -> Option<&str> {
        self.config.config_version.as_deref()
    }
}

/// Finds XXMI: `explicit` (a folder the user picked) first, then the default
/// `%APPDATA%\XXMI Launcher`, then the MSI install record. The first folder that has both
/// the config file and the launcher exe wins.
pub fn find_xxmi(env: &dyn Environment, explicit: Option<&Path>) -> Option<XxmiInstall> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = explicit {
        candidates.push(p.to_path_buf());
    }
    if let Some(app_data) = env.roaming_app_data() {
        candidates.push(app_data.join("XXMI Launcher"));
    }
    candidates.extend(env.xxmi_install_locations());
    candidates.iter().find_map(|c| open_xxmi(c))
}

/// Validates one folder as an XXMI Launcher root (config file plus launcher exe), without
/// trying any other location.
pub fn open_xxmi(root: &Path) -> Option<XxmiInstall> {
    if !root.is_absolute() {
        return None;
    }
    let mut guard = Guard::new();
    let root_id = guard.register_root("xxmi", root, Access::ReadOnly).ok()?;
    let config_path = guard.path(root_id, CONFIG_FILE).ok()?;
    let exe_path = guard.path(root_id, LAUNCHER_EXE).ok()?;
    if !guard.metadata(&config_path).ok()?.is_file() {
        return None;
    }
    // The exe must exist and start like a Windows executable.
    let mut magic = [0u8; 2];
    guard
        .open_read(&exe_path)
        .ok()?
        .read_exact(&mut magic)
        .ok()?;
    if &magic != b"MZ" {
        return None;
    }

    let (config, config_error) = match guard.metadata(&config_path) {
        Ok(m) if m.len() > MAX_CONFIG_BYTES => (
            XxmiConfig::default(),
            Some("XXMI config is unusually large; not read".to_owned()),
        ),
        _ => match guard
            .read(&config_path)
            .map_err(|e| e.to_string())
            .and_then(|b| XxmiConfig::parse(&b))
        {
            Ok(config) => (config, None),
            Err(e) => (XxmiConfig::default(), Some(e)),
        },
    };
    let canonical = plain(guard.root_path(root_id)?.to_path_buf());
    Some(XxmiInstall {
        launcher_exe: canonical
            .join("Resources")
            .join("Bin")
            .join("XXMI Launcher.exe"),
        root: canonical,
        config_error,
        config,
        guard,
        root_id,
    })
}

/// Drops Windows' `\\?\` verbatim prefix for display.
pub(crate) fn plain(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}
