// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm install`, and the helper every command that writes to `Mods/` uses.

use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::GameBanana;
use gmm_core::{
    AppDirs, CoreError, InstallEvent, InstallOptions, InstallRequest, VariantChoice, Workspace,
    conflicts_for, install, install_gate,
};
use gmm_db::Db;
use gmm_detect::{SystemEnvironment, detect};
use gmm_fsguard::RootId;
use gmm_net::Client;
use gmm_xxmi::{evaluate, find_xxmi};
use serde::Serialize;

use crate::fetch::{game_of, progress};
use crate::format::bytes;
use crate::library::describe;
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

    let target = ModsTarget::open(db, game, args.xxmi, args.folders).await?;
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

/// A game's `Mods/` folder, registered for writing, after the setup gate.
pub struct ModsTarget {
    pub ws: Workspace,
    pub mods: RootId,
    pub mods_dir: PathBuf,
    /// Setup problems that don't stop GMM from writing to `Mods/` (see `install_gate`).
    pub warnings: Vec<String>,
}

impl ModsTarget {
    /// Finds XXMI (the `--xxmi` folder, else the saved one, else automatically) and checks
    /// the game's setup. Fails when there's no `Mods/` folder to work with.
    pub async fn open(
        db: &Db,
        game: &GameManifest,
        xxmi_dir: Option<PathBuf>,
        folders: Vec<(String, PathBuf)>,
    ) -> Result<Self, AnyError> {
        let (xxmi_dir, folders) = with_saved_setup(db, xxmi_dir, folders).await?;
        let env = SystemEnvironment;
        let detected = detect(Registry::builtin()?, &env, &folders);
        let xxmi = find_xxmi(&env, xxmi_dir.as_deref());
        let status = evaluate(game, &detected.installs, xxmi.as_ref());
        let warnings = install_gate(&status)?;
        let xxmi = xxmi.ok_or("XXMI Launcher not found")?;
        let mods_dir = xxmi.mods_dir(game.xxmi.importer);
        let mut ws = Workspace::open(&AppDirs::system()?.cache)?;
        let mods = ws.add_mods_root(&mods_dir)?;
        Ok(Self {
            ws,
            mods,
            mods_dir,
            warnings,
        })
    }
}
