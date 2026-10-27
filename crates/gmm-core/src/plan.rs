// SPDX-License-Identifier: MIT OR Apache-2.0

//! The background check before a queued install goes ahead, built
//! from a [`Prepared`] mod: every version in the archive with its blocked files and warnings,
//! which versions are already installed ("Add versions"), which ones replace the same things,
//! and what the user has to look at first ([`review`]). The local install shows the same
//! versions (`local.rs`).

use gmm_adapters::GameManifest;
use gmm_archive::Severity;
use gmm_db::Db;
use serde::Serialize;

use crate::analyze::{Analysis, VersionConflict};
use crate::error::CoreError;
use crate::install::{InstallOptions, Prepared, VariantChoice};
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

/// Where mods install, relative to XXMI: `ZZMI\Mods\GMM`.
pub(crate) fn install_to(game: &GameManifest) -> String {
    format!("{}\\Mods\\{GMM_DIR}", game.xxmi.importer.as_str())
}

/// The folder a version installs to under `Mods/GMM/`: the mod's name and id, plus the
/// version's name inside the archive (`variant`, empty for an archive with one mod). Some mods
/// ship each variant as a GameBanana file of its own, each holding one mod (or the same
/// version names), so every file would get the same folder; when that folder already holds
/// another of the mod's files, this file's name (without its extension) is added. `file` is
/// the GameBanana file's id and name, when the version comes from one.
pub(crate) async fn version_folder(
    db: &Db,
    game: &GameManifest,
    mod_name: &str,
    gb_mod_id: Option<u64>,
    file: Option<(u64, &str)>,
    variant: &str,
) -> Result<String, CoreError> {
    let plain = mod_folder_name(
        mod_name,
        gb_mod_id,
        (!variant.is_empty()).then_some(variant),
    );
    let Some((file_id, file_name)) = file else {
        return Ok(plain);
    };
    match db.installed_mod_by_folder(game.id(), &plain).await? {
        Some(row) if row.gb_file_id != Some(file_id) => {
            let label = file_name
                .rsplit_once('.')
                .map_or(file_name, |(stem, _)| stem)
                .trim();
            let label = if label.is_empty() { file_name } else { label };
            let extra = if variant.is_empty() {
                label.to_owned()
            } else {
                format!("{variant}, {label}")
            };
            Ok(mod_folder_name(mod_name, gb_mod_id, Some(&extra)))
        }
        _ => Ok(plain),
    }
}

/// Every version of an analysed archive: its folder, size, blocked files and warnings, and
/// whether it's installed already. `file` as in [`version_folder`].
pub(crate) async fn version_plans(
    db: &Db,
    game: &GameManifest,
    analysis: &Analysis,
    mod_name: &str,
    gb_mod_id: Option<u64>,
    file: Option<(u64, &str)>,
) -> Result<Vec<VersionPlan>, CoreError> {
    let mut versions = Vec::with_capacity(analysis.variants.len());
    for v in &analysis.variants {
        let folder = version_folder(db, game, mod_name, gb_mod_id, file, &v.name).await?;
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
    Ok(versions)
}

/// Why a queued install stopped after its background check and waits for the user (the
/// Downloads page's "Needs action"). Each one can be accepted ("Install anyway").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CheckIssue {
    /// The archive holds several versions and none were chosen yet.
    ChooseVersions { count: usize },
    /// Programs, scripts or shortcuts GMM never installs; installing goes on without them.
    Blocked { files: Vec<String> },
    /// Scan warnings (odd files that are installed but worth a look), as `path: reason`.
    Warnings { items: Vec<String> },
    /// The file is in the mod's archived (old) files on GameBanana; `newest` is the name of
    /// the newest current file, when there is one.
    Outdated { newest: Option<String> },
}

/// What the user sees when an install needs action: the issues, and the archive's versions
/// for choosing (as in the install confirmation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Review {
    pub issues: Vec<CheckIssue>,
    pub versions: Vec<VersionPlan>,
    pub version_conflicts: Vec<VersionConflict>,
}

/// The background check of a fetched and analysed mod before it installs: `None` when it
/// can install as asked, otherwise what the user has to look at first. An archive with no
/// mod in it isn't an issue to accept; installing fails on it with its own error.
pub async fn review(
    db: &Db,
    game: &GameManifest,
    prepared: &Prepared,
    options: &InstallOptions,
) -> Result<Option<Review>, CoreError> {
    let analysis = &prepared.analysis;
    if analysis.kind.is_none() || analysis.variants.is_empty() {
        return Ok(None);
    }
    let file = &prepared.fetched.file;
    let versions = version_plans(
        db,
        game,
        analysis,
        &prepared.mod_name,
        Some(prepared.mod_id),
        Some((file.id, file.name.as_str())),
    )
    .await?;
    let multi = versions.len() > 1;
    let mut issues = Vec::new();
    if multi && options.variants == VariantChoice::Only {
        issues.push(CheckIssue::ChooseVersions {
            count: versions.len(),
        });
    }
    // The versions that would install: the chosen ones, or every one still to choose from.
    let chosen = |v: &VersionPlan| match &options.variants {
        VariantChoice::Named(names) => names.iter().any(|n| n.eq_ignore_ascii_case(&v.name)),
        VariantChoice::Only | VariantChoice::All => true,
    };
    let label = |v: &VersionPlan, item: &str| {
        if multi {
            format!("{}: {item}", v.name)
        } else {
            item.to_owned()
        }
    };
    let blocked: Vec<String> = versions
        .iter()
        .filter(|v| chosen(v))
        .flat_map(|v| v.blocked.iter().map(|b| label(v, b)))
        .collect();
    if !blocked.is_empty() && !options.skip_blocked {
        issues.push(CheckIssue::Blocked { files: blocked });
    }
    let warnings: Vec<String> = versions
        .iter()
        .filter(|v| chosen(v))
        .flat_map(|v| v.warnings.iter().map(|w| label(v, w)))
        .collect();
    if !warnings.is_empty() {
        issues.push(CheckIssue::Warnings { items: warnings });
    }
    if prepared.fetched.file.archived {
        issues.push(CheckIssue::Outdated {
            newest: prepared.files.default_choice().map(|f| f.name.clone()),
        });
    }
    if issues.is_empty() {
        return Ok(None);
    }
    Ok(Some(Review {
        issues,
        versions,
        version_conflicts: analysis.version_conflicts.clone(),
    }))
}
