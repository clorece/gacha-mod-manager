// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm fetch`: catalogue → download → safe extraction → policy scan. Downloads are kept in
//! the download cache and recorded in the database. Installing into XXMI's `Mods/` folder is
//! the install pipeline's job (Phase 2).

use std::io::Write;
use std::ops::ControlFlow;

use gmm_archive::{Limits, ScanOptions, Severity, extract, scan};
use gmm_catalog::{CatalogSource, GameBanana};
use gmm_db::{Db, NewCachedDownload};
use gmm_fsguard::{Access, Guard};
use gmm_net::{Client, DownloadRequest, Url};
use serde::Serialize;

use crate::AnyError;
use crate::format::bytes;

/// Downloads above this are refused outright, whatever the catalogue says.
const MAX_DOWNLOAD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Serialize)]
struct FetchReport {
    mod_id: u64,
    file_id: u64,
    archive: String,
    archive_bytes: u64,
    sha256: Option<String>,
    extracted_to: String,
    files: usize,
    extracted_bytes: u64,
    top_level: Vec<String>,
    blocked: Vec<String>,
    warnings: Vec<String>,
}

pub async fn run(
    gb: &GameBanana,
    net: &Client,
    db: &Db,
    mod_id: u64,
    file_id: Option<u64>,
    json: bool,
) -> Result<(), AnyError> {
    let files = gb.mod_files(mod_id).await?;
    let file = match file_id {
        Some(id) => files
            .files
            .iter()
            .chain(&files.archived_files)
            .find(|f| f.id == id)
            .ok_or_else(|| format!("mod {mod_id} has no file {id}"))?,
        None => files
            .default_choice()
            .ok_or("this mod has no file that GameBanana has scanned as clean")?,
    };
    if !file.is_clean() {
        return Err(format!(
            "file {} has not been cleared by GameBanana's virus scan ({}); refusing to download",
            file.id,
            file.av_result.as_deref().unwrap_or("pending")
        )
        .into());
    }

    let base = dirs::data_local_dir()
        .ok_or("could not find the local app data folder")?
        .join("GMM")
        .join("cache");
    let mut guard = Guard::new();
    let cache = guard.ensure_root("cache", &base, Access::ReadWrite)?;
    guard.create_dir_all(&guard.path(cache, "downloads")?)?;
    guard.create_dir_all(&guard.path(cache, "extract")?)?;

    // GameBanana file names are untrusted; fall back to the id if one isn't a safe name.
    let dest = guard
        .path(cache, &format!("downloads/{}-{}", file.id, file.name))
        .or_else(|_| guard.path(cache, &format!("downloads/{}.bin", file.id)))?;

    let path_rel = dest.rel().to_string();
    let recorded = db
        .cached_download_for_file(file.id)
        .await?
        .filter(|d| d.path_rel == path_rel);
    let mut sha256 = None;
    if guard.exists(&dest)? {
        if !json {
            eprintln!("using cached download {}", guard.display(&dest));
        }
        if let Some(d) = recorded {
            db.touch_download(&d.sha256).await?;
            sha256 = Some(d.sha256);
        }
    } else {
        if let Some(stale) = recorded {
            // The file was deleted behind GMM's back; forget it and download again.
            db.remove_download(&stale.sha256).await?;
        }
        if !json {
            eprintln!("downloading {} ({})", file.name, bytes(file.size));
        }
        let request = DownloadRequest {
            url: Url::parse(&file.download_url)?,
            dest: dest.clone(),
            expected_size: Some(file.size),
            expected_md5: file.md5.clone(),
            max_bytes: MAX_DOWNLOAD_BYTES,
        };
        let mut last_shown = 0;
        let outcome = net
            .download(&guard, &request, |p| {
                if !json && let Some(total) = p.total.filter(|t| *t > 0) {
                    let percent = p.downloaded * 100 / total;
                    if percent >= last_shown + 10 || p.downloaded == total {
                        last_shown = percent;
                        eprint!("\r  {percent:>3}%  {}", bytes(p.downloaded));
                        let _ = std::io::stderr().flush();
                    }
                }
                ControlFlow::Continue(())
            })
            .await?;
        if !json {
            eprintln!(
                "\r  done: {}{}",
                bytes(outcome.bytes),
                if outcome.resumed { " (resumed)" } else { "" }
            );
        }
        db.record_download(&NewCachedDownload {
            sha256: outcome.sha256.clone(),
            gb_mod_id: Some(mod_id),
            gb_file_id: Some(file.id),
            path_rel,
            size_bytes: outcome.bytes,
        })
        .await?;
        sha256 = Some(outcome.sha256);
    }

    let out_dir = guard.path(cache, &format!("extract/{}", file.id))?;
    if guard.exists(&out_dir)? {
        guard.remove_dir_all(&out_dir)?;
    }
    let report = extract(&guard, &dest, &out_dir, &Limits::default())?;
    let findings = scan(&guard, &report, &ScanOptions::default())?;

    let mut top_level: Vec<String> = report
        .entries
        .iter()
        .filter_map(|e| e.path.components().first().cloned())
        .collect();
    top_level.dedup();
    let describe = |severity| {
        findings
            .findings
            .iter()
            .filter(|f| f.severity == severity)
            .map(|f| format!("{}: {:?}", f.path, f.kind))
            .collect::<Vec<_>>()
    };
    let result = FetchReport {
        mod_id,
        file_id: file.id,
        archive: guard.display(&dest),
        archive_bytes: guard.metadata(&dest)?.len(),
        sha256,
        extracted_to: guard.display(&out_dir),
        files: report.files().count(),
        extracted_bytes: report.total_bytes,
        top_level,
        blocked: describe(Severity::Blocked),
        warnings: describe(Severity::Warning),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    println!(
        "extracted {} files ({}) from {:?} to {}",
        result.files,
        bytes(result.extracted_bytes),
        report.format,
        result.extracted_to
    );
    if let Some(hash) = &result.sha256 {
        println!("sha256 {hash}");
    }
    println!("top-level folders/files: {}", result.top_level.join(", "));
    if result.blocked.is_empty() && result.warnings.is_empty() {
        println!("scan: clean");
    }
    for line in &result.blocked {
        println!("BLOCKED  {line}");
    }
    for line in &result.warnings {
        println!("warning  {line}");
    }
    Ok(())
}
