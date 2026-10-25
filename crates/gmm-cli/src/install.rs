// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm install`.

use std::path::PathBuf;

use gmm_adapters::GameManifest;
use gmm_catalog::GameBanana;
use gmm_core::{
    CoreError, InstallEvent, InstallOptions, InstallRequest, ModsTarget, SetupHints, StopSignal,
    VariantChoice, conflicts_after_install, install,
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
            start_disabled: false,
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
        // The CLI has no pause button; Ctrl+C ends the process (a download resumes next time).
        &StopSignal::new(),
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
    let clashes =
        conflicts_after_install(db, &target.ws, target.mods, game, &report.installed).await?;

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

pub struct LocalArgs {
    pub file: PathBuf,
    pub game: &'static GameManifest,
    pub variants: Vec<String>,
    pub all_variants: bool,
    pub skip_blocked: bool,
    pub xxmi: Option<PathBuf>,
    pub folders: Vec<(String, PathBuf)>,
}

/// `gmm install-file`: install from an archive on this computer (same checks as a download).
pub async fn run_local(db: &Db, args: LocalArgs, json: bool) -> Result<(), AnyError> {
    let game = args.game;
    let file = std::path::absolute(&args.file)?;
    let hints = SetupHints {
        xxmi: args.xxmi,
        folders: args.folders,
    };
    let target = ModsTarget::open(db, game, hints).await?;
    if !json {
        eprintln!("copying into the cache, extracting and scanning");
    }
    let prepared = gmm_core::prepare_local(&target.ws, game, &file)?;
    let options = InstallOptions {
        variants: if args.all_variants {
            VariantChoice::All
        } else if args.variants.is_empty() {
            VariantChoice::Only
        } else {
            VariantChoice::Named(args.variants)
        },
        skip_blocked: args.skip_blocked,
        start_disabled: false,
    };
    let result = gmm_core::install_local(
        db,
        &target.ws,
        target.mods,
        game,
        &prepared,
        &options,
        |_| {},
    )
    .await;
    prepared.discard(&target.ws);
    let installed = match result {
        Ok(installed) => installed,
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
                "the archive contains executables or scripts, which GMM never installs:\n  {}\n\
                 Re-run with --skip-blocked to install it without them.",
                files.join("\n  ")
            )
            .into());
        }
        Err(e) => return Err(e.into()),
    };
    let clashes = conflicts_after_install(db, &target.ws, target.mods, game, &installed).await?;
    crate::output(json, &installed, || {
        let mut text = String::new();
        for folder in &installed {
            text.push_str(&format!(
                "installed {:?} as #{} ({} files, {})\n",
                folder.folder_name,
                folder.id,
                folder.files,
                bytes(folder.bytes)
            ));
            for skipped in &folder.skipped {
                text.push_str(&format!("  left out (blocked): {skipped}\n"));
            }
        }
        for c in &clashes {
            text.push_str(&format!("conflict: {}\n", describe(c)));
        }
        text
    })
}
