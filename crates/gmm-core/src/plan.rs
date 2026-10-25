// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the install confirmation shows (docs/08, "Install confirmation"), built from a
//! [`Prepared`] mod: the file, the mod's other files, every version in the archive with its
//! blocked files and warnings, which versions are already installed ("Add versions" mode),
//! which ones replace the same things, and where it all goes.

use gmm_adapters::GameManifest;
use gmm_archive::Severity;
use gmm_catalog::ModFile;
use gmm_db::Db;
use serde::Serialize;

use crate::analyze::VersionConflict;
use crate::error::CoreError;
use crate::install::Prepared;
use crate::names::{GMM_DIR, mod_folder_name};

/// One version (variant) of the archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionPlan {
    /// Empty for an archive with one mod; otherwise what `install` takes as a version name.
    pub name: String,
    /// The folder it gets under `Mods/GMM/`.
    pub folder: String,
    pub files: usize,
    pub bytes: u64,
    /// The installed copy's id, when this version is already installed.
    pub installed: Option<i64>,
    /// Files that are never installed (executables, scripts), relative to the version.
    pub blocked: Vec<String>,
    /// Scan warnings, as `path: reason`.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InstallPlan {
    pub game: String,
    pub mod_id: u64,
    pub mod_name: String,
    /// The file that was fetched and analysed.
    pub file: ModFile,
    /// The mod's other current files, newest first (for "Choose a file").
    pub other_files: Vec<ModFile>,
    pub archive_bytes: u64,
    pub from_cache: bool,
    /// The mod kind from the game's manifest, or `None` when nothing in the archive looks
    /// like a mod for this game (installing would fail).
    pub kind: Option<String>,
    pub versions: Vec<VersionPlan>,
    /// Versions that replace the same things; only one of each pair should be on.
    pub version_conflicts: Vec<VersionConflict>,
    /// Where it installs, relative to XXMI: `ZZMI\Mods\GMM`.
    pub install_to: String,
}

/// Builds the confirmation for `prepared`, marking versions that are already installed.
pub async fn install_plan(
    db: &Db,
    game: &GameManifest,
    prepared: &Prepared,
) -> Result<InstallPlan, CoreError> {
    let analysis = &prepared.analysis;
    let mut versions = Vec::with_capacity(analysis.variants.len());
    for v in &analysis.variants {
        let variant = (!v.name.is_empty()).then_some(v.name.as_str());
        let folder = mod_folder_name(&prepared.mod_name, Some(prepared.mod_id), variant);
        let installed = db
            .installed_mod_by_folder(game.id(), &folder)
            .await?
            .map(|m| m.id);
        let mine = |path: &gmm_fsguard::RelPath| path.starts_with(&v.root);
        let relative = |path: &gmm_fsguard::RelPath| {
            path.strip_prefix(&v.root)
                .map_or_else(|| path.to_string(), |p| p.to_string())
        };
        let blocked = analysis
            .scan
            .blocked()
            .filter(|f| mine(&f.path))
            .map(|f| relative(&f.path))
            .collect();
        let warnings = analysis
            .scan
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Warning && mine(&f.path))
            .map(|f| format!("{}: {:?}", relative(&f.path), f.kind))
            .collect();
        versions.push(VersionPlan {
            name: v.name.clone(),
            folder,
            files: v.files,
            bytes: v.bytes,
            installed,
            blocked,
            warnings,
        });
    }
    let file = prepared.fetched.file.clone();
    let mut other_files: Vec<ModFile> = prepared
        .files
        .files
        .iter()
        .filter(|f| f.id != file.id)
        .cloned()
        .collect();
    other_files.sort_by_key(|f| std::cmp::Reverse(f.date_added));
    Ok(InstallPlan {
        game: game.id().to_owned(),
        mod_id: prepared.mod_id,
        mod_name: prepared.mod_name.clone(),
        file,
        other_files,
        archive_bytes: prepared.fetched.bytes,
        from_cache: prepared.fetched.from_cache,
        kind: analysis.kind.clone(),
        versions,
        version_conflicts: analysis.version_conflicts.clone(),
        install_to: format!("{}\\Mods\\{GMM_DIR}", game.xxmi.importer.as_str()),
    })
}
