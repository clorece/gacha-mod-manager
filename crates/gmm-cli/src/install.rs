// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm install` and `gmm list`.

use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::GameBanana;
use gmm_core::{
    AppDirs, CoreError, InstallEvent, InstallOptions, InstallRequest, VariantChoice, Workspace,
    install, install_gate, on_disk_name,
};
use gmm_db::Db;
use gmm_detect::{SystemEnvironment, detect};
use gmm_net::Client;
use gmm_xxmi::{evaluate, find_xxmi};
use serde::Serialize;

use crate::fetch::{game_of, progress};
use crate::format::bytes;
use crate::{AnyError, with_saved_setup};

pub struct InstallArgs {
    pub mod_id: u64,
    pub file: Option<u64>,
    pub game: Option<&'static GameManifest>,
    pub variants: Vec<String>,
    pub all_variants: bool,
    pub skip_blocked: bool,
    pub xxmi: Option<PathBuf>,
    pub folders: Vec<(String, PathBuf)>,
}

pub async fn run(
    gb: &GameBanana,
    net: &Client,
    db: &Db,
    args: InstallArgs,
    json: bool,
) -> Result<(), AnyError> {
    let game = match args.game {
        Some(g) => g,
        None => game_of(gb, args.mod_id).await?,
    };

    // Where XXMI loads this game's mods from, and whether the setup is fit to use.
    let (xxmi_dir, folders) = with_saved_setup(db, args.xxmi, args.folders).await?;
    let env = SystemEnvironment;
    let detected = detect(Registry::builtin()?, &env, &folders);
    let xxmi = find_xxmi(&env, xxmi_dir.as_deref());
    let status = evaluate(game, &detected.installs, xxmi.as_ref());
    let warnings = install_gate(&status)?;
    let xxmi = xxmi.ok_or("XXMI Launcher not found")?;
    let mods_dir = xxmi.mods_dir(game.xxmi.importer);

    let mut ws = Workspace::open(&AppDirs::system()?.cache)?;
    let mods = ws.add_mods_root(&mods_dir)?;
    let request = InstallRequest {
        mod_id: args.mod_id,
        file_id: args.file,
        options: InstallOptions {
            variants: if args.all_variants {
                VariantChoice::All
            } else if args.variants.is_empty() {
                VariantChoice::Only
            } else {
                VariantChoice::Named(args.variants)
            },
            skip_blocked: args.skip_blocked,
        },
    };
    let mut show_fetch = progress(json);
    let result = install(
        gb,
        net,
        db,
        &ws,
        mods,
        game,
        &request,
        |event| match event {
            InstallEvent::Fetch(e) => show_fetch(e),
            InstallEvent::Extracting if !json => eprintln!("extracting and scanning"),
            InstallEvent::Installing { folder } if !json => eprintln!("installing {folder:?}"),
            _ => {}
        },
    )
    .await;
    let report = match result {
        Ok(report) => report,
        Err(CoreError::ChooseVariant(list)) => {
            return Err(format!(
                "this archive holds several mods. Pick with --variant N (repeatable) or \
                 --all-variants:\n  {}",
                list.join("\n  ")
            )
            .into());
        }
        Err(CoreError::Blocked(files)) => {
            return Err(format!(
                "the mod contains executables or scripts, which GMM never installs:\n  {}\n\
                 Re-run with --skip-blocked to install it without them.",
                files.join("\n  ")
            )
            .into());
        }
        Err(e) => return Err(e.into()),
    };

    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            #[serde(flatten)]
            report: &'a gmm_core::InstallReport,
            setup_warnings: &'a [String],
        }
        let out = Out {
            report: &report,
            setup_warnings: &warnings,
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    for folder in &report.installed {
        println!(
            "installed {:?} ({} files, {}) into {}",
            folder.folder_name,
            folder.files,
            bytes(folder.bytes),
            mods_dir.join(gmm_core::GMM_DIR).display()
        );
        for skipped in &folder.skipped {
            println!("  left out (blocked): {skipped}");
        }
        for warning in &folder.warnings {
            println!("  warning: {warning}");
        }
    }
    for warning in &warnings {
        println!("note: {warning}");
    }
    if warnings.is_empty() {
        println!(
            "Start {} through XXMI to use it (see `gmm status` for the command).",
            game.game.name
        );
    }
    Ok(())
}

#[derive(Serialize)]
struct ListRow {
    id: i64,
    game: String,
    name: String,
    folder: String,
    enabled: bool,
    variant: Option<String>,
    gb_mod_id: Option<u64>,
    gb_file_id: Option<u64>,
    version: Option<String>,
    size_bytes: u64,
    installed_at: i64,
}

pub async fn list(
    db: &Db,
    game: Option<&'static GameManifest>,
    json: bool,
) -> Result<(), AnyError> {
    let rows: Vec<ListRow> = db
        .installed_mods(game.map(GameManifest::id))
        .await?
        .into_iter()
        .map(|m| ListRow {
            folder: on_disk_name(&m.folder_name, m.enabled),
            id: m.id,
            game: m.game_id,
            name: m.name,
            enabled: m.enabled,
            variant: m.variant,
            gb_mod_id: m.gb_mod_id,
            gb_file_id: m.gb_file_id,
            version: m.version,
            size_bytes: m.size_bytes,
            installed_at: m.installed_at,
        })
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    if rows.is_empty() {
        println!("no mods installed by GMM yet (`gmm install <mod id>`)");
        return Ok(());
    }
    for r in &rows {
        println!(
            "{:>4}  {:<5} {:<3}  {:<9} {}{}",
            r.id,
            r.game,
            if r.enabled { "on" } else { "off" },
            bytes(r.size_bytes),
            r.folder,
            r.version
                .as_deref()
                .map(|v| format!("  v{v}"))
                .unwrap_or_default()
        );
    }
    Ok(())
}
