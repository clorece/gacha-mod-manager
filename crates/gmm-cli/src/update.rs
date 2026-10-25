// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm update` and `gmm rollback`: move an installed GameBanana mod to its newest file, or
//! back to one still in the download cache (`gmm_core::change_file`).

use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::GameBanana;
use gmm_core::{
    DeleteForever, InstallEvent, ModsTarget, SetupHints, StopSignal, change_file, check_updates,
    rollback_choices,
};
use gmm_db::Db;
use gmm_net::Client;

use crate::fetch::{game_of, progress};
use crate::format::bytes;
use crate::{AnyError, output};

pub struct ChangeArgs {
    pub gb_mod_id: Option<u64>,
    pub file: Option<u64>,
    pub rollback: bool,
    pub xxmi: Option<PathBuf>,
    pub folders: Vec<(String, PathBuf)>,
}

pub async fn run(
    gb: &GameBanana,
    net: &Client,
    db: &Db,
    args: ChangeArgs,
    json: bool,
) -> Result<(), AnyError> {
    let Some(gb_mod_id) = args.gb_mod_id else {
        return list_updates(gb, db, json).await;
    };
    let game: &'static GameManifest = game_of(gb, gb_mod_id).await?;
    let file = if args.rollback && args.file.is_none() {
        let choices = rollback_choices(db, game, gb_mod_id).await?;
        let Some(&newest) = choices.first() else {
            return Err(format!(
                "no other file of mod {gb_mod_id} is in the download cache to go back to"
            )
            .into());
        };
        Some(newest)
    } else {
        args.file
    };
    let hints = SetupHints {
        xxmi: args.xxmi,
        folders: args.folders,
    };
    let target = ModsTarget::open(db, game, hints).await?;
    let mut show_fetch = progress(json);
    let report = change_file(
        gb,
        net,
        db,
        &target.ws,
        target.mods,
        game,
        gb_mod_id,
        file,
        &DeleteForever,
        &StopSignal::new(),
        |event| match event {
            InstallEvent::Fetch(e) => show_fetch(e),
            InstallEvent::Extracting if !json => eprintln!("extracting and scanning"),
            InstallEvent::Installing { folder } if !json => eprintln!("replacing {folder:?}"),
            _ => {}
        },
    )
    .await?;
    output(json, &report, || {
        let mut text = String::new();
        if report.changed.is_empty() {
            text.push_str(&format!(
                "{} is already on {} (nothing changed)\n",
                report.mod_name, report.file.name
            ));
        }
        for f in &report.changed {
            text.push_str(&format!(
                "{} {:?} is now on {} ({} files, {}; {})\n",
                if args.rollback {
                    "rolled back:"
                } else {
                    "updated:"
                },
                f.folder_name,
                report.file.name,
                f.files,
                bytes(f.bytes),
                if f.enabled { "on" } else { "off" }
            ));
            for skipped in &f.skipped {
                text.push_str(&format!("  left out (blocked): {skipped}\n"));
            }
        }
        for kept in &report.kept {
            text.push_str(&format!(
                "kept as it was: {kept:?} (its version isn't in this file, or it's already on it)\n"
            ));
        }
        for folder in &report.not_removed {
            text.push_str(&format!(
                "note: the previous {folder:?} couldn't be deleted; GMM removes it within an hour\n"
            ));
        }
        if !report.changed.is_empty() {
            text.push_str("The previous file stays in the download cache (`gmm rollback`).\n");
        }
        text
    })
}

/// Every installed GameBanana mod with a newer clean file.
async fn list_updates(gb: &GameBanana, db: &Db, json: bool) -> Result<(), AnyError> {
    let mut updates = Vec::new();
    let mut notes = Vec::new();
    for game in Registry::builtin()?.games() {
        let check = check_updates(gb, db, game).await?;
        updates.extend(check.updates);
        notes.extend(check.notes);
    }
    output(json, &updates, || {
        let mut text = String::new();
        if updates.is_empty() {
            text.push_str("All installed GameBanana mods are on their newest file.\n");
        }
        for u in &updates {
            text.push_str(&format!(
                "{} (GameBanana {}): {} is newer · `gmm update {}`\n",
                u.name, u.gb_mod_id, u.newest.name, u.gb_mod_id
            ));
        }
        for note in &notes {
            text.push_str(&format!("note: {note}\n"));
        }
        text
    })
}
