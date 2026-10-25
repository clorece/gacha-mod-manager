// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashSet;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use gmm_adapters::GameManifest;
use gmm_archive::{Finding, Severity};
use gmm_catalog::{CatalogSource, ModFile, ModFiles};
use gmm_db::{Db, ModCategories, NewInstalledMod};
use gmm_fsguard::{Guard, RelPath, RootId, ScopedPath};
use gmm_net::Client;
use gmm_xxmi::{GameStatus, SetupState};
use serde::{Deserialize, Serialize};

use crate::analyze::{Analysis, Variant, analyze};
use crate::error::CoreError;
use crate::fetch::{FetchEvent, FetchedArchive, fetch_archive};
use crate::names::{GMM_DIR, SIDECAR_FILE, STAGING_DIR, mod_folder_name, on_disk_name};
use crate::stop::StopSignal;
use crate::workspace::{STALE_TEMP, Workspace, remove_stale, unique_id};

pub(crate) const SIDECAR_FORMAT: u32 = 1;

/// Which variants of an archive to install.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VariantChoice {
    /// The archive's only mod. Fails with [`CoreError::ChooseVariant`] if it holds several.
    #[default]
    Only,
    /// By name (case-insensitive) or by 1-based number in [`crate::Analysis::variants`].
    Named(Vec<String>),
    All,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallOptions {
    pub variants: VariantChoice,
    /// Install even though the mod contains blocked files (executables, scripts). Those
    /// files are left out; they stay only in the cached archive and are never run.
    pub skip_blocked: bool,
    /// Install switched off (`DISABLED ` folders) instead of on.
    pub start_disabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallRequest {
    pub mod_id: u64,
    /// Defaults to the newest file GameBanana scanned as clean.
    pub file_id: Option<u64>,
    pub options: InstallOptions,
}

/// Progress of [`install`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallEvent {
    Fetch(FetchEvent),
    Extracting,
    Installing { folder: String },
}

/// What the mod is, for its `gmm.json` and database row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModMeta {
    pub name: String,
    pub gb_mod_id: Option<u64>,
    pub gb_file_id: Option<u64>,
    pub file_name: Option<String>,
    pub file_date: Option<i64>,
    pub version: Option<String>,
    pub archive_sha256: String,
    /// GameBanana's categories, stored for the library cards and filters.
    pub categories: ModCategories,
}

impl ModMeta {
    pub fn from_gamebanana(
        name: &str,
        mod_version: Option<&str>,
        file: &ModFile,
        sha256: &str,
    ) -> Self {
        Self {
            name: name.to_owned(),
            gb_mod_id: None,
            gb_file_id: Some(file.id),
            file_name: Some(file.name.clone()),
            file_date: Some(file.date_added),
            version: file
                .version
                .clone()
                .or_else(|| mod_version.map(str::to_owned)),
            archive_sha256: sha256.to_owned(),
            categories: ModCategories::default(),
        }
    }
}

/// The `gmm.json` written into every installed mod folder. It lets GMM (or a person) tell
/// where a folder came from even without the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub format: u32,
    pub game: String,
    pub name: String,
    pub variant: Option<String>,
    pub gb_mod_id: Option<u64>,
    pub gb_file_id: Option<u64>,
    pub file_name: Option<String>,
    pub file_date: Option<i64>,
    pub version: Option<String>,
    pub archive_sha256: String,
    pub installed_at: i64,
    pub installed_by: String,
}

/// One mod folder that was installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InstalledFolder {
    /// Database id.
    pub id: i64,
    pub folder_name: String,
    pub variant: Option<String>,
    pub files: usize,
    pub bytes: u64,
    /// Blocked files that were left out (only with [`InstallOptions::skip_blocked`]).
    pub skipped: Vec<String>,
    /// Scan warnings for this folder, as `path: reason`.
    pub warnings: Vec<String>,
}

/// Outcome of [`install`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InstallReport {
    pub game: String,
    pub mod_id: u64,
    pub mod_name: String,
    pub file_id: u64,
    pub file_name: String,
    pub archive_sha256: String,
    pub from_cache: bool,
    pub installed: Vec<InstalledFolder>,
}

#[derive(Serialize)]
struct ScanEntry {
    path: String,
    severity: &'static str,
    kind: String,
}

/// Whether installing for this game makes sense now. Without XXMI or the game's importer
/// there's no `Mods/` folder to install into; every other problem only stops mods from
/// loading, so installing goes ahead and the returned warnings are shown.
pub fn install_gate(status: &GameStatus) -> Result<Vec<String>, CoreError> {
    match status.state {
        SetupState::Ready => Ok(Vec::new()),
        SetupState::XxmiNotFound | SetupState::ImporterMissing => Err(CoreError::NotReady {
            game: status.game_name.clone(),
            reason: status.summary.clone(),
        }),
        other => Ok(vec![format!(
            "{} ({}): {} Mods won't load until that's fixed (see `gmm status`).",
            status.game_name,
            other.label(),
            status.summary
        )]),
    }
}

/// A GameBanana mod fetched into the download cache and analysed, before anything is
/// installed. The install confirmation is built from it ([`crate::install_plan`]);
/// [`install_prepared`] installs from it. Its extraction folder stays in the cache until
/// [`Prepared::discard`].
#[derive(Debug, Clone)]
pub struct Prepared {
    pub mod_id: u64,
    pub mod_name: String,
    pub mod_version: Option<String>,
    pub categories: ModCategories,
    /// The mod's files on GameBanana, for choosing another one.
    pub files: ModFiles,
    pub fetched: FetchedArchive,
    pub analysis: Analysis,
}

impl Prepared {
    /// Removes the extraction folder (the archive stays in the download cache).
    pub fn discard(&self, ws: &Workspace) {
        self.analysis.cleanup(ws);
    }
}

/// Step one of an install: catalogue checks, download (or the cached copy), extraction, scan,
/// variants and the conflicts between them. Nothing is installed.
#[allow(clippy::too_many_arguments)]
pub async fn prepare<C: CatalogSource>(
    catalog: &C,
    net: &Client,
    db: &Db,
    ws: &Workspace,
    game: &GameManifest,
    mod_id: u64,
    file_id: Option<u64>,
    stop: &StopSignal,
    mut on_event: impl FnMut(InstallEvent),
) -> Result<Prepared, CoreError> {
    let profile = catalog.mod_profile(mod_id).await?;
    if profile.is_trashed {
        return Err(CoreError::NotInstallable {
            mod_id,
            reason: "it was removed from GameBanana",
        });
    }
    if profile.is_withheld {
        return Err(CoreError::NotInstallable {
            mod_id,
            reason: "GameBanana is withholding it",
        });
    }
    if profile.summary.obsolete {
        return Err(CoreError::NotInstallable {
            mod_id,
            reason: "its author marked it obsolete",
        });
    }
    if let Some(found) = profile.summary.game_id
        && found != game.gamebanana.game_id
    {
        return Err(CoreError::WrongGame {
            mod_id,
            expected: game.gamebanana.game_id,
            found,
        });
    }

    let fetched = fetch_archive(catalog, net, db, ws, mod_id, file_id, stop, |e| {
        on_event(InstallEvent::Fetch(e));
    })
    .await?;
    stop.check()?;
    on_event(InstallEvent::Extracting);
    let analysis = analyze(ws, game, &fetched.archive)?;
    if let Err(e) = stop.check() {
        analysis.cleanup(ws);
        return Err(e);
    }
    Ok(Prepared {
        mod_id,
        mod_name: profile.summary.name,
        categories: ModCategories {
            root_category_id: profile.summary.root_category.as_ref().and_then(|c| c.id),
            root_category: profile.summary.root_category.map(|c| c.name),
            category_id: profile.summary.category.as_ref().and_then(|c| c.id),
            category: profile.summary.category.map(|c| c.name),
        },
        mod_version: profile.summary.version,
        files: profile.files,
        fetched,
        analysis,
    })
}

/// Step two: installs the chosen variant(s) of a prepared mod. Doesn't discard `prepared`.
pub async fn install_prepared(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    prepared: &Prepared,
    options: &InstallOptions,
    on_event: impl FnMut(InstallEvent),
) -> Result<InstallReport, CoreError> {
    let fetched = &prepared.fetched;
    let meta = ModMeta {
        gb_mod_id: Some(prepared.mod_id),
        categories: prepared.categories.clone(),
        ..ModMeta::from_gamebanana(
            &prepared.mod_name,
            prepared.mod_version.as_deref(),
            &fetched.file,
            &fetched.sha256,
        )
    };
    let installed = install_analyzed(
        db,
        ws,
        mods,
        game,
        &meta,
        &prepared.analysis,
        options,
        on_event,
    )
    .await?;
    Ok(InstallReport {
        game: game.id().to_owned(),
        mod_id: prepared.mod_id,
        mod_name: prepared.mod_name.clone(),
        file_id: fetched.file.id,
        file_name: fetched.file.name.clone(),
        archive_sha256: fetched.sha256.clone(),
        from_cache: fetched.from_cache,
        installed,
    })
}

/// The whole pipeline for one GameBanana mod ([`prepare`], then [`install_prepared`]):
/// catalogue checks, download (or cache), extraction, scan, variants, staging, rename into
/// `<Mods>/GMM/`, database. `mods` must be the game's `Mods/` root, registered in `ws`.
#[allow(clippy::too_many_arguments)]
pub async fn install<C: CatalogSource>(
    catalog: &C,
    net: &Client,
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    request: &InstallRequest,
    stop: &StopSignal,
    mut on_event: impl FnMut(InstallEvent),
) -> Result<InstallReport, CoreError> {
    let prepared = prepare(
        catalog,
        net,
        db,
        ws,
        game,
        request.mod_id,
        request.file_id,
        stop,
        &mut on_event,
    )
    .await?;
    let result = match stop.check() {
        Ok(()) => {
            install_prepared(
                db,
                ws,
                mods,
                game,
                &prepared,
                &request.options,
                &mut on_event,
            )
            .await
        }
        Err(e) => Err(e),
    };
    prepared.discard(ws);
    result
}

/// Installs the chosen variant(s) of an analysed archive into `<Mods>/GMM/`. Each one is
/// assembled under the staging folder, given its `gmm.json`, recorded in the database and
/// renamed into place, so a mod folder appears complete or not at all. Doesn't remove the
/// analysis' extraction folder.
#[allow(clippy::too_many_arguments)]
pub async fn install_analyzed(
    db: &Db,
    ws: &Workspace,
    mods: RootId,
    game: &GameManifest,
    meta: &ModMeta,
    analysis: &Analysis,
    options: &InstallOptions,
    mut on_event: impl FnMut(InstallEvent),
) -> Result<Vec<InstalledFolder>, CoreError> {
    let guard = ws.guard();
    if analysis.kind.is_none() {
        let markers: Vec<&str> = game
            .mod_kinds
            .iter()
            .flat_map(|k| k.ini_markers.iter().map(String::as_str))
            .collect();
        return Err(CoreError::NotAMod {
            markers: markers.join(" or "),
        });
    }
    let chosen = select_variants(analysis, &options.variants)?;

    // Plan every folder before touching the Mods folder.
    let gmm_dir = guard.path(mods, GMM_DIR)?;
    let mut plans = Vec::with_capacity(chosen.len());
    let mut names_seen = HashSet::new();
    for variant in chosen {
        let variant_name = (!variant.name.is_empty()).then_some(variant.name.as_str());
        let folder = mod_folder_name(&meta.name, meta.gb_mod_id, variant_name);
        if !names_seen.insert(folder.to_ascii_lowercase()) {
            return Err(CoreError::FolderTaken { folder });
        }
        if db
            .installed_mod_by_folder(game.id(), &folder)
            .await?
            .is_some()
        {
            return Err(CoreError::AlreadyInstalled {
                game: game.game.name.clone(),
                folder,
            });
        }
        for on in [true, false] {
            if guard.exists(&gmm_dir.join(&on_disk_name(&folder, on))?)? {
                return Err(CoreError::FolderTaken { folder });
            }
        }
        let (blocked, findings) = variant_findings(analysis, variant);
        if !blocked.is_empty() && !options.skip_blocked {
            return Err(CoreError::Blocked(
                blocked.iter().map(ToString::to_string).collect(),
            ));
        }
        plans.push((variant, folder, blocked, findings));
    }

    guard.create_dir_all(&gmm_dir)?;
    let staging_root = gmm_dir.join(STAGING_DIR)?;
    guard.create_dir_all(&staging_root)?;
    remove_stale(guard, &staging_root, STALE_TEMP);
    let stage = staging_root.join(&unique_id())?;
    guard.create_dir_all(&stage)?;

    let result = async {
        // Assemble every folder first, so a copy failure leaves nothing installed.
        let mut staged = Vec::with_capacity(plans.len());
        for (index, (variant, folder, blocked, findings)) in plans.into_iter().enumerate() {
            on_event(InstallEvent::Installing {
                folder: folder.clone(),
            });
            let dir = stage.join(&index.to_string())?;
            let assembled = assemble_variant(
                guard, game, meta, analysis, variant, &folder, &blocked, &findings, &dir,
            )?;
            staged.push((dir, assembled.row, assembled.folder));
        }

        // Record, then rename. If the rename fails the row is removed again.
        let enabled = !options.start_disabled;
        let mut installed = Vec::with_capacity(staged.len());
        for (dir, row, mut folder) in staged {
            let id = db.insert_installed_mod(&row).await?;
            if !enabled && let Err(e) = db.set_mod_enabled(id, false).await {
                let _ = db.delete_installed_mod(id).await;
                return Err(e.into());
            }
            let target = gmm_dir.join(&on_disk_name(&row.folder_name, enabled))?;
            if let Err(e) = guard.rename(&dir, &target) {
                let _ = db.delete_installed_mod(id).await;
                return Err(e.into());
            }
            folder.id = id;
            installed.push(folder);
        }
        Ok(installed)
    }
    .await;
    let _ = guard.remove_dir_all(&stage);
    result
}

/// A chosen variant's blocked files (left out) and every scan finding inside it.
pub(crate) fn variant_findings<'a>(
    analysis: &'a Analysis,
    variant: &Variant,
) -> (Vec<RelPath>, Vec<&'a Finding>) {
    let in_variant = |f: &&Finding| f.path.starts_with(&variant.root);
    let blocked = analysis
        .scan
        .blocked()
        .filter(in_variant)
        .map(|f| f.path.clone())
        .collect();
    let findings = analysis.scan.findings.iter().filter(in_variant).collect();
    (blocked, findings)
}

/// One variant copied into `dir` with its `gmm.json`: the row to record and the report.
pub(crate) struct Assembled {
    pub row: NewInstalledMod,
    pub folder: InstalledFolder,
}

/// Copies a variant (minus `blocked`) into `dir` and writes its sidecar. Nothing in the
/// Mods folder changes yet; `dir` is under the staging folder.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assemble_variant(
    guard: &Guard,
    game: &GameManifest,
    meta: &ModMeta,
    analysis: &Analysis,
    variant: &Variant,
    folder: &str,
    blocked: &[RelPath],
    findings: &[&Finding],
    dir: &ScopedPath,
) -> Result<Assembled, CoreError> {
    let (files, bytes) = copy_variant(guard, analysis, variant, blocked, dir)?;
    let variant_name = (!variant.name.is_empty()).then(|| variant.name.clone());
    let sidecar = Sidecar {
        format: SIDECAR_FORMAT,
        game: game.id().to_owned(),
        name: meta.name.clone(),
        variant: variant_name.clone(),
        gb_mod_id: meta.gb_mod_id,
        gb_file_id: meta.gb_file_id,
        file_name: meta.file_name.clone(),
        file_date: meta.file_date,
        version: meta.version.clone(),
        archive_sha256: meta.archive_sha256.clone(),
        installed_at: now(),
        installed_by: format!("GMM {}", env!("CARGO_PKG_VERSION")),
    };
    guard.write_atomic(
        &dir.join(SIDECAR_FILE)?,
        &serde_json::to_vec_pretty(&sidecar)?,
    )?;
    let report: Vec<ScanEntry> = findings
        .iter()
        .map(|f| ScanEntry {
            path: relative_to(&f.path, &variant.root),
            severity: match f.severity {
                Severity::Blocked => "blocked",
                Severity::Warning => "warning",
            },
            kind: format!("{:?}", f.kind),
        })
        .collect();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .map(|f| format!("{}: {:?}", relative_to(&f.path, &variant.root), f.kind))
        .collect();
    Ok(Assembled {
        row: NewInstalledMod {
            game_id: game.id().to_owned(),
            folder_name: folder.to_owned(),
            name: meta.name.clone(),
            gb_mod_id: meta.gb_mod_id,
            gb_file_id: meta.gb_file_id,
            variant: variant_name.clone(),
            version: meta.version.clone(),
            file_date: meta.file_date,
            archive_sha256: Some(meta.archive_sha256.clone()),
            size_bytes: bytes,
            scan_report_json: Some(serde_json::to_string(&report)?),
            categories: meta.categories.clone(),
        },
        folder: InstalledFolder {
            id: 0,
            folder_name: folder.to_owned(),
            variant: variant_name,
            files,
            bytes,
            skipped: blocked
                .iter()
                .map(|p| relative_to(p, &variant.root))
                .collect(),
            warnings,
        },
    })
}

fn select_variants<'a>(
    analysis: &'a Analysis,
    choice: &VariantChoice,
) -> Result<Vec<&'a Variant>, CoreError> {
    let all = &analysis.variants;
    let names = || {
        all.iter()
            .enumerate()
            .map(|(i, v)| format!("{}. {}", i + 1, v.name))
            .collect::<Vec<_>>()
    };
    match choice {
        VariantChoice::All => Ok(all.iter().collect()),
        VariantChoice::Only => match all.as_slice() {
            [only] => Ok(vec![only]),
            _ => Err(CoreError::ChooseVariant(names())),
        },
        VariantChoice::Named(wanted) => {
            let mut out: Vec<&Variant> = Vec::new();
            for want in wanted {
                let want_trimmed = want.trim();
                let found = want_trimmed
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|i| all.get(i))
                    .or_else(|| {
                        all.iter()
                            .find(|v| v.name.eq_ignore_ascii_case(want_trimmed))
                    })
                    .ok_or_else(|| CoreError::UnknownVariant {
                        name: want.clone(),
                        available: names(),
                    })?;
                if !out.iter().any(|v| v.root == found.root) {
                    out.push(found);
                }
            }
            if out.is_empty() {
                return Err(CoreError::ChooseVariant(names()));
            }
            Ok(out)
        }
    }
}

/// Copies a variant's files from the extraction folder into `dest`, leaving out `skip`.
fn copy_variant(
    guard: &Guard,
    analysis: &Analysis,
    variant: &Variant,
    skip: &[RelPath],
    dest: &ScopedPath,
) -> Result<(usize, u64), CoreError> {
    guard.create_dir_all(dest)?;
    let mut files = 0;
    let mut bytes = 0;
    for entry in analysis.extract.files() {
        let Some(rel) = entry.path.strip_prefix(&variant.root) else {
            continue;
        };
        if skip.contains(&entry.path) {
            continue;
        }
        let target = dest.join_rel(&rel);
        if let Some(parent) = target.parent() {
            guard.create_dir_all(&parent)?;
        }
        let mut src = guard.open_read(&analysis.extract.dest.join_rel(&entry.path))?;
        let mut dst = guard.create_new(&target)?;
        let copied = io::copy(&mut src, &mut dst)
            .map_err(|e| CoreError::Io(format!("copying {rel}: {e}")))?;
        if copied != entry.size {
            return Err(CoreError::Io(format!(
                "copying {rel}: expected {} bytes, copied {copied}",
                entry.size
            )));
        }
        files += 1;
        bytes += copied;
    }
    Ok((files, bytes))
}

fn relative_to(path: &RelPath, root: &RelPath) -> String {
    path.strip_prefix(root)
        .unwrap_or_else(|| path.clone())
        .to_string()
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}
