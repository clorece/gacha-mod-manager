// SPDX-License-Identifier: MIT OR Apache-2.0

//! What's in an archive: extraction, policy scan, mod kind and variants.
//!
//! A folder is a **mod root** when it directly holds an `.ini` with one of the game's mod
//! markers (`[TextureOverride`, ...) and no folder above it does. Ini files further down
//! belong to that mod (toggles, parts). One mod root means one mod; several mean the archive
//! holds variants, and the user picks which to install (each becomes its own folder).
//! Files 3DMigoto skips (`DISABLED*` names) and XXMI's own loader files don't count.

use std::collections::BTreeSet;

use gmm_adapters::GameManifest;
use gmm_archive::{ExtractReport, Limits, ScanOptions, ScanReport, extract, scan};
use gmm_fsguard::{Guard, RelPath, ScopedPath};
use gmm_migoto::{Override, parse};
use serde::Serialize;

use crate::error::CoreError;
use crate::workspace::Workspace;

/// `.ini` files above this size aren't read for markers.
const MAX_INI_BYTES: u64 = 4 * 1024 * 1024;

/// One installable mod inside an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Empty when the archive holds a single mod; otherwise the variant's folder path below
    /// the folder all variants share, e.g. `Variant B` or `Red - Long Hair`.
    pub name: String,
    /// The mod's folder, relative to the extraction folder.
    pub root: RelPath,
    pub files: usize,
    pub bytes: u64,
}

/// Two versions in one archive that replace the same things, so only one should be on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionConflict {
    /// Positions in [`Analysis::variants`].
    pub a: usize,
    pub b: usize,
    /// How many hashes both replace.
    pub hashes: usize,
}

/// An extracted, scanned and classified archive. The extraction folder stays in the cache
/// until [`Analysis::cleanup`] (which [`crate::install`] calls).
#[derive(Debug, Clone)]
pub struct Analysis {
    pub extract: ExtractReport,
    pub scan: ScanReport,
    /// The matching mod kind from the game's manifest (`migoto`), or `None` if no ini had a
    /// marker.
    pub kind: Option<String>,
    /// In path order. Empty when `kind` is `None`.
    pub variants: Vec<Variant>,
    /// Pairs of variants that replace the same things (same rule as conflicts between
    /// installed mods).
    pub version_conflicts: Vec<VersionConflict>,
}

impl Analysis {
    /// Removes the extraction folder.
    pub fn cleanup(&self, ws: &Workspace) {
        let _ = ws.guard().remove_dir_all(&self.extract.dest);
    }
}

/// Extracts `archive` into a fresh folder in the cache, scans it and classifies it.
pub fn analyze(
    ws: &Workspace,
    game: &GameManifest,
    archive: &ScopedPath,
) -> Result<Analysis, CoreError> {
    let guard = ws.guard();
    let dest = ws.new_extract_dir()?;
    let report = extract(guard, archive, &dest, &Limits::default())?;
    let classified = scan(guard, &report, &ScanOptions::default())
        .map_err(CoreError::from)
        .and_then(|scan| {
            let (kind, variants) = classify(guard, &report, game)?;
            let version_conflicts = version_conflicts(guard, &report, &variants);
            Ok((scan, kind, variants, version_conflicts))
        });
    match classified {
        Ok((scan, kind, variants, version_conflicts)) => Ok(Analysis {
            extract: report,
            scan,
            kind,
            variants,
            version_conflicts,
        }),
        Err(e) => {
            let _ = guard.remove_dir_all(&dest);
            Err(e)
        }
    }
}

fn classify(
    guard: &Guard,
    report: &ExtractReport,
    game: &GameManifest,
) -> Result<(Option<String>, Vec<Variant>), CoreError> {
    for kind in &game.mod_kinds {
        let markers: Vec<String> = kind
            .ini_markers
            .iter()
            .map(|m| m.to_ascii_lowercase())
            .collect();
        let mut marker_dirs: BTreeSet<Vec<String>> = BTreeSet::new();
        for entry in report.files() {
            if !counts_as_mod_ini(&entry.path) || entry.size > MAX_INI_BYTES {
                continue;
            }
            let text = guard.read(&report.dest.join_rel(&entry.path))?;
            let text = String::from_utf8_lossy(&text).to_ascii_lowercase();
            if markers.iter().any(|m| text.contains(m.as_str())) {
                let dir = entry.path.parent().unwrap_or_default();
                marker_dirs.insert(dir.components().to_vec());
            }
        }
        if marker_dirs.is_empty() {
            continue;
        }
        let roots: Vec<&Vec<String>> = marker_dirs
            .iter()
            .filter(|d| {
                !marker_dirs
                    .iter()
                    .any(|other| other.len() < d.len() && d.starts_with(other))
            })
            .collect();
        let shared = common_prefix(&roots);
        let mut variants = Vec::with_capacity(roots.len());
        for components in roots {
            let root = RelPath::parse(&components.join("/"))?;
            let name = if components.len() > shared {
                components[shared..].join(" - ")
            } else {
                String::new()
            };
            let (files, bytes) = report
                .files()
                .filter(|e| e.path.starts_with(&root))
                .fold((0, 0), |(n, b), e| (n + 1, b + e.size));
            variants.push(Variant {
                name,
                root,
                files,
                bytes,
            });
        }
        // A single mod has no variant name, even when it sits in a sub-folder.
        if let [only] = variants.as_mut_slice() {
            only.name.clear();
        }
        return Ok((Some(kind.id.clone()), variants));
    }
    Ok((None, Vec::new()))
}

/// Which variants replace the same resources. Only computed when there are several.
fn version_conflicts(
    guard: &Guard,
    report: &ExtractReport,
    variants: &[Variant],
) -> Vec<VersionConflict> {
    if variants.len() < 2 {
        return Vec::new();
    }
    let overrides: Vec<Vec<Override>> = variants
        .iter()
        .map(|v| {
            report
                .files()
                .filter(|e| {
                    e.path.starts_with(&v.root)
                        && counts_as_mod_ini(&e.path)
                        && e.size <= MAX_INI_BYTES
                })
                .filter_map(|e| guard.read(&report.dest.join_rel(&e.path)).ok())
                .flat_map(|bytes| parse(&String::from_utf8_lossy(&bytes)).overrides)
                .filter(|o| o.replaces.any())
                .collect()
        })
        .collect();
    let mut out = Vec::new();
    for a in 0..variants.len() {
        for b in a + 1..variants.len() {
            let mut shared: BTreeSet<&str> = BTreeSet::new();
            for oa in &overrides[a] {
                if overrides[b]
                    .iter()
                    .any(|ob| ob.hash == oa.hash && oa.overlaps(ob))
                {
                    shared.insert(oa.hash.as_str());
                }
            }
            if !shared.is_empty() {
                out.push(VersionConflict {
                    a,
                    b,
                    hashes: shared.len(),
                });
            }
        }
    }
    out
}

/// `.ini` files 3DMigoto would load: not XXMI's own config, and no `DISABLED*` component
/// (XXMI's `exclude_recursive = DISABLED*`).
fn counts_as_mod_ini(path: &RelPath) -> bool {
    let Some(name) = path.file_name().map(str::to_ascii_lowercase) else {
        return false;
    };
    if !name.ends_with(".ini") || name == "d3dx.ini" || name == "d3dx_user.ini" {
        return false;
    }
    !path
        .components()
        .iter()
        .any(|c| c.to_ascii_lowercase().starts_with("disabled"))
}

fn common_prefix(paths: &[&Vec<String>]) -> usize {
    let Some((first, rest)) = paths.split_first() else {
        return 0;
    };
    let mut len = first.len();
    for p in rest {
        len = len.min(
            first
                .iter()
                .zip(p.iter())
                .take_while(|(a, b)| a == b)
                .count(),
        );
    }
    len
}
