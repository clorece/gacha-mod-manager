// SPDX-License-Identifier: MIT OR Apache-2.0

//! HoYoPlay (the native launcher for ZZZ and other HoYoverse games):
//! `%APPDATA%\Cognosphere\HYP\<version>\data\gamedata.dat`.
//!
//! The file is HoYoPlay's own binary format with JSON records embedded (observed 2026-10-23).
//! Rather than depend on that undocumented layout, only `"installPath":"..."` values are pulled
//! out, and each one counts only if it passes a game's folder markers. Most records hold default
//! paths for games that aren't installed; those fail the marker check on their own. No other
//! field is read or kept, and the file is never logged.

use std::path::Path;

use gmm_adapters::Registry;
use gmm_fsguard::{Access, Guard};

use crate::env::Environment;
use crate::{DetectReport, InstallSource, add_candidate};

const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PATHS: usize = 128;
const MAX_PATH_CHARS: usize = 1024;

pub(crate) fn detect(registry: &Registry, env: &dyn Environment, report: &mut DetectReport) {
    let Some(app_data) = env.roaming_app_data() else {
        return;
    };
    let base = app_data.join("Cognosphere").join("HYP");
    let mut guard = Guard::new();
    let Ok(root) = guard.register_root("hoyoplay", &base, Access::ReadOnly) else {
        return; // HoYoPlay isn't installed.
    };
    let Ok(top) = guard.path(root, "") else {
        return;
    };
    let Ok(versions) = guard.read_dir(&top) else {
        return;
    };
    for version in versions.into_iter().filter(|e| e.is_dir) {
        let Ok(data) = version.path.join("data/gamedata.dat") else {
            continue;
        };
        if !guard.exists(&data).unwrap_or(false) {
            continue;
        }
        if guard
            .metadata(&data)
            .is_ok_and(|m| m.len() > MAX_FILE_BYTES)
        {
            continue;
        }
        // Rust opens files in shared mode, so this works while HoYoPlay is running.
        let bytes = match guard.read(&data) {
            Ok(bytes) => bytes,
            Err(e) => {
                report.notes.push(format!("HoYoPlay data unreadable: {e}"));
                continue;
            }
        };
        for path in install_paths(&bytes) {
            for game in registry.games() {
                add_candidate(
                    report,
                    game,
                    InstallSource::HoYoPlay,
                    Path::new(&path),
                    None,
                );
            }
        }
    }
}

/// Every `"installPath": "..."` value, JSON-unescaped, de-duplicated.
pub(crate) fn install_paths(bytes: &[u8]) -> Vec<String> {
    const KEY: &str = "\"installPath\"";
    let text = String::from_utf8_lossy(bytes);
    let mut out: Vec<String> = Vec::new();
    let mut rest = text.as_ref();
    while let Some(pos) = rest.find(KEY) {
        rest = &rest[pos + KEY.len()..];
        let after_colon = rest.trim_start().strip_prefix(':').map(str::trim_start);
        let Some(value) = after_colon.and_then(|s| s.strip_prefix('"')) else {
            continue;
        };
        let mut path = String::new();
        let mut chars = value.chars();
        let mut closed = false;
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    closed = true;
                    break;
                }
                '\\' => match chars.next() {
                    Some('/') => path.push('/'),
                    Some('\\') => path.push('\\'),
                    Some('"') => path.push('"'),
                    _ => break,
                },
                c if c.is_control() => break,
                c => path.push(c),
            }
            if path.len() > MAX_PATH_CHARS {
                break;
            }
        }
        if closed && !path.is_empty() && !out.contains(&path) {
            out.push(path);
            if out.len() >= MAX_PATHS {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_install_paths_from_binary_noise() {
        let mut data = vec![0x83, 0x70, 0x00, 0x00, 0xFF, 0xFF];
        data.extend_from_slice(
            br#"{"gameBiz":"nap_global","installPath":"D:\\HoYoverse\\ZenlessZoneZero Game","x":1}"#,
        );
        data.extend_from_slice(&[0x00, 0x01, 0x02]);
        data.extend_from_slice(
            br#"{"installPath" : "C:\\Program Files\\HoYoPlay\\games\\Genshin Impact game"}"#,
        );
        data.extend_from_slice(
            br#"{"installPath":""}{"installPath":"D:\\HoYoverse\\ZenlessZoneZero Game"}"#,
        );
        data.extend_from_slice(br#"{"installPath":"unterminated"#);
        assert_eq!(
            install_paths(&data),
            [
                r"D:\HoYoverse\ZenlessZoneZero Game",
                r"C:\Program Files\HoYoPlay\games\Genshin Impact game",
            ]
        );
    }

    #[test]
    fn tolerates_garbage() {
        assert!(install_paths(b"").is_empty());
        assert!(install_paths(b"\"installPath\"").is_empty());
        assert!(install_paths(b"\"installPath\":12").is_empty());
        assert!(install_paths(&[0xFF; 1000]).is_empty());
    }
}
