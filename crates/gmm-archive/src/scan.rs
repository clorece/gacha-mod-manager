// SPDX-License-Identifier: MIT OR Apache-2.0

//! Policy scan of an extracted mod (see "Policy scan" in `docs/security.md`).
//!
//! The scan never deletes or runs anything. It reports; the install pipeline decides.

use std::io::Read;

use gmm_fsguard::{FsError, Guard, RelPath};

use crate::extract::ExtractReport;
use crate::format::detect_format;

/// Extensions that are executable or scriptable on Windows. 3DMigoto mods never need them.
const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "dll", "bat", "cmd", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta",
    "sh", "scr", "msi", "msp", "jar", "com", "pif", "cpl", "lnk", "url", "scf", "reg",
];

/// Loader files that belong to XXMI itself. Inside a mod they would override XXMI's own
/// configuration (XXMI warns about these too).
const LOADER_FILES: &[&str] = &[
    "d3dx.ini",
    "d3dx_user.ini",
    "d3d11.dll",
    "d3dcompiler_47.dll",
    "dxgi.dll",
    "nvapi64.dll",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Must not be deployed into `Mods/`.
    Blocked,
    /// Shown to the user; deploying is allowed.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingKind {
    /// File extension is executable or a script.
    ExecutableExtension(String),
    /// File content starts like a Windows/Linux executable or a script, whatever its name.
    ExecutableContent,
    /// One of XXMI's own loader files.
    LoaderFile,
    /// An `.ini` refers to paths outside its mod folder (`..\` or `../`).
    IniEscapesFolder,
    /// An archive inside the archive. Not extracted.
    NestedArchive,
    LargeFile(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub path: RelPath,
    pub severity: Severity,
    pub kind: FindingKind,
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    /// Blocked findings first, then by path.
    pub findings: Vec<Finding>,
}

impl ScanReport {
    pub fn is_blocked(&self) -> bool {
        self.findings
            .iter()
            .any(|f| f.severity == Severity::Blocked)
    }

    pub fn blocked(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Blocked)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
    }
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Files above this size get a [`FindingKind::LargeFile`] warning.
    pub large_file_bytes: u64,
    /// `.ini` files above this size are not read for path checks.
    pub max_ini_bytes: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            large_file_bytes: 512 * 1024 * 1024,
            max_ini_bytes: 4 * 1024 * 1024,
        }
    }
}

pub fn scan(
    guard: &Guard,
    report: &ExtractReport,
    options: &ScanOptions,
) -> Result<ScanReport, FsError> {
    let mut findings = Vec::new();
    for entry in report.files() {
        let path = report.dest.join_rel(&entry.path);
        let name = entry
            .path
            .file_name()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let extension = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
        let mut push = |severity, kind| {
            findings.push(Finding {
                path: entry.path.clone(),
                severity,
                kind,
            })
        };

        let blocked_by_name = BLOCKED_EXTENSIONS.contains(&extension);
        if blocked_by_name {
            push(
                Severity::Blocked,
                FindingKind::ExecutableExtension(extension.to_owned()),
            );
        }
        if LOADER_FILES.contains(&name.as_str()) {
            push(Severity::Warning, FindingKind::LoaderFile);
        }

        let mut header = [0u8; 8];
        let n = guard.open_read(&path)?.read(&mut header).unwrap_or(0);
        let header = &header[..n];
        if !blocked_by_name && looks_executable(header) {
            push(Severity::Blocked, FindingKind::ExecutableContent);
        }
        if detect_format(header).is_some() {
            push(Severity::Warning, FindingKind::NestedArchive);
        }

        if extension == "ini" && entry.size <= options.max_ini_bytes {
            let text = String::from_utf8_lossy(&guard.read(&path)?).into_owned();
            if text.contains("..\\") || text.contains("../") {
                push(Severity::Warning, FindingKind::IniEscapesFolder);
            }
        }
        if entry.size > options.large_file_bytes {
            push(Severity::Warning, FindingKind::LargeFile(entry.size));
        }
    }
    findings.sort_by_key(|f| (f.severity, f.path.to_string()));
    Ok(ScanReport { findings })
}

/// PE (`MZ`), ELF, Mach-O and `#!` scripts.
fn looks_executable(header: &[u8]) -> bool {
    header.starts_with(b"MZ")
        || header.starts_with(b"\x7fELF")
        || header.starts_with(b"#!")
        || header.starts_with(&[0xCF, 0xFA, 0xED, 0xFE])
        || header.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_magic() {
        assert!(looks_executable(b"MZ\x90\x00\x03"));
        assert!(looks_executable(b"\x7fELF\x02"));
        assert!(looks_executable(b"#!/bin/sh"));
        assert!(!looks_executable(b"DDS \x7c\x00"));
        assert!(!looks_executable(b"[TextureOverride"));
        assert!(!looks_executable(b""));
    }
}
