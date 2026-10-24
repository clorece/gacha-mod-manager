// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm install`.

use std::path::PathBuf;

use gmm_adapters::GameManifest;
use gmm_catalog::GameBanana;
use gmm_core::{
    CoreError, InstallEvent, InstallOptions, InstallRequest, ModsTarget, SetupHints, VariantChoice,
    conflicts_for, install,
};
use gmm_db::Db;
use gmm_net::Client;
use serde::Serialize;

use crate::AnyError;
use crate::fetch::{game_of, progress};
use crate::format::bytes;
use crate::library::describe;

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

    let hints = SetupHints {
        xxmi: args.xxmi,
        folders: args.folders,
    };
    let target = ModsTarget::open(db, game, hints).await?;
    let warnings = &target.warnings;
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
        &target.ws,
        target.mods,
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

    // New mods start enabled; say right away if they clash with something already on.
    let mut clashes = Vec::new();
    for folder in &report.installed {
        let found = conflicts_for(db, &target.ws, target.mods, game, folder.id).await?;
        clashes.extend(found.conflicts);
    }
    clashes.dedup();

    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            #[serde(flatten)]
            report: &'a gmm_core::InstallReport,
            setup_warnings: &'a [String],
            conflicts: &'a [gmm_core::Conflict],
        }
        let out = Out {
            report: &report,
            setup_warnings: warnings,
            conflicts: &clashes,
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
            target.mods_dir.join(gmm_core::GMM_DIR).display()
        );
        for skipped in &folder.skipped {
            println!("  left out (blocked): {skipped}");
        }
        for warning in &folder.warnings {
            println!("  warning: {warning}");
        }
    }
    for c in &clashes {
        println!("conflict: {}", describe(c));
    }
    if !clashes.is_empty() {
        println!("Both stay on; switch one off with `gmm disable <id>`.");
    }
    for warning in warnings {
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
