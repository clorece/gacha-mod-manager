// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm fetch`: catalogue → download (or the cached copy) → safe extraction → policy scan →
//! what kind of mod it is and which variants it holds. Nothing is installed; the extraction
//! folder is removed afterwards. `gmm install` runs the same steps and then installs.

use std::io::Write;

use gmm_adapters::GameManifest;
use gmm_archive::Severity;
use gmm_catalog::{CatalogSource, GameBanana};
use gmm_core::{AppDirs, FetchEvent, StopSignal, Workspace, analyze, fetch_archive};
use gmm_db::Db;
use gmm_net::Client;
use serde::Serialize;

use crate::AnyError;
use crate::format::bytes;

#[derive(Serialize)]
struct FetchReport {
    mod_id: u64,
    file_id: u64,
    file_name: String,
    archive_bytes: u64,
    sha256: String,
    from_cache: bool,
    files: usize,
    extracted_bytes: u64,
    kind: Option<String>,
    variants: Vec<VariantRow>,
    blocked: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct VariantRow {
    name: String,
    folder: String,
    files: usize,
    bytes: u64,
}

/// Prints download progress to stderr (nothing in JSON mode).
pub fn progress(json: bool) -> impl FnMut(FetchEvent) {
    let mut last_shown = 0;
    move |event| {
        if json {
            return;
        }
        match event {
            FetchEvent::UsingCache => eprintln!("using the cached download"),
            FetchEvent::Downloading { size } => eprintln!("downloading ({})", bytes(size)),
            FetchEvent::Progress(p) => {
                if let Some(total) = p.total.filter(|t| *t > 0) {
                    let percent = p.downloaded * 100 / total;
                    if percent >= last_shown + 10 || p.downloaded == total {
                        last_shown = percent;
                        eprint!("\r  {percent:>3}%  {}", bytes(p.downloaded));
                        if p.downloaded == total {
                            eprintln!();
                        }
                        let _ = std::io::stderr().flush();
                    }
                }
            }
        }
    }
}

/// The game a GameBanana mod belongs to, from its mod page.
pub async fn game_of(gb: &GameBanana, mod_id: u64) -> Result<&'static GameManifest, AnyError> {
    Ok(gmm_core::game_for_mod(&gb.mod_profile(mod_id).await?)?)
}

pub async fn run(
    gb: &GameBanana,
    net: &Client,
    db: &Db,
    mod_id: u64,
    file_id: Option<u64>,
    json: bool,
) -> Result<(), AnyError> {
    let game = game_of(gb, mod_id).await?;
    let ws = Workspace::open(&AppDirs::system()?.cache)?;
    let stop = StopSignal::new();
    let fetched = fetch_archive(gb, net, db, &ws, mod_id, file_id, &stop, progress(json)).await?;
    let analysis = analyze(&ws, game, &fetched.archive)?;
    analysis.cleanup(&ws);

    let describe = |severity| {
        analysis
            .scan
            .findings
            .iter()
            .filter(|f| f.severity == severity)
            .map(|f| format!("{}: {:?}", f.path, f.kind))
            .collect::<Vec<_>>()
    };
    let result = FetchReport {
        mod_id,
        file_id: fetched.file.id,
        file_name: fetched.file.name.clone(),
        archive_bytes: fetched.bytes,
        sha256: fetched.sha256.clone(),
        from_cache: fetched.from_cache,
        files: analysis.extract.files().count(),
        extracted_bytes: analysis.extract.total_bytes,
        kind: analysis.kind.clone(),
        variants: analysis
            .variants
            .iter()
            .map(|v| VariantRow {
                name: v.name.clone(),
                folder: if v.root.is_root() {
                    "(archive root)".to_owned()
                } else {
                    v.root.to_string()
                },
                files: v.files,
                bytes: v.bytes,
            })
            .collect(),
        blocked: describe(Severity::Blocked),
        warnings: describe(Severity::Warning),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    println!(
        "{} ({}): {} files, {} extracted, {:?}",
        result.file_name,
        bytes(result.archive_bytes),
        result.files,
        bytes(result.extracted_bytes),
        analysis.extract.format
    );
    println!("sha256 {}", result.sha256);
    match &result.kind {
        Some(kind) => println!("kind: {kind} mod for {}", game.game.name),
        None => println!("kind: not recognised as a {} mod", game.game.name),
    }
    if result.variants.len() > 1 {
        println!("variants (install with --variant N or --all-variants):");
        for (n, v) in result.variants.iter().enumerate() {
            println!(
                "  {}. {}  ({} files, {})",
                n + 1,
                v.name,
                v.files,
                bytes(v.bytes)
            );
        }
    } else if let Some(v) = result.variants.first() {
        println!("mod folder: {} ({} files)", v.folder, v.files);
    }
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
