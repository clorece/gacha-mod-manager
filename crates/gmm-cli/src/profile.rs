// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm profile`: named sets of switched-on mods per game (`gmm_core` profiles).

use std::io::Read;

use clap::Subcommand;
use gmm_adapters::GameManifest;
use gmm_core::{ModsTarget, ProfileInfo, SetupHints};
use gmm_db::Db;

use crate::{AnyError, SetupArgs, output, parse_game};

#[derive(Subcommand)]
pub enum ProfileCommand {
    /// A game's saved profiles.
    List {
        #[arg(value_parser = parse_game)]
        game: &'static GameManifest,
    },
    /// Save which mods are on now as a profile (replacing one of that name).
    Save {
        #[arg(value_parser = parse_game)]
        game: &'static GameManifest,
        name: String,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Switch the profile's mods on and every other mod of the game off.
    Apply {
        #[arg(value_parser = parse_game)]
        game: &'static GameManifest,
        name: String,
        #[command(flatten)]
        setup: SetupArgs,
    },
    Rename {
        #[arg(value_parser = parse_game)]
        game: &'static GameManifest,
        name: String,
        new_name: String,
    },
    Delete {
        #[arg(value_parser = parse_game)]
        game: &'static GameManifest,
        name: String,
    },
    /// Print the profile's share code (GameBanana ids of its GMM mods).
    Export {
        #[arg(value_parser = parse_game)]
        game: &'static GameManifest,
        name: String,
    },
    /// Make a profile from a share code (`-` reads it from standard input).
    Import {
        #[arg(value_parser = parse_game)]
        game: &'static GameManifest,
        code: String,
        /// Save it under this name instead of the code's.
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        setup: SetupArgs,
    },
}

async fn by_name(db: &Db, game: &GameManifest, name: &str) -> Result<ProfileInfo, AnyError> {
    gmm_core::profiles(db, game)
        .await?
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name.trim()))
        .ok_or_else(|| {
            format!(
                "no profile {name:?} for {} (see `gmm profile list {}`)",
                game.game.name,
                game.id()
            )
            .into()
        })
}

pub async fn run(db: &Db, cmd: ProfileCommand, json: bool) -> Result<(), AnyError> {
    match cmd {
        ProfileCommand::List { game } => {
            let list = gmm_core::profiles(db, game).await?;
            output(json, &list, || {
                if list.is_empty() {
                    return format!(
                        "No profiles for {} yet (`gmm profile save {} <name>`).\n",
                        game.game.name,
                        game.id()
                    );
                }
                list.iter()
                    .map(|p| format!("{:<30} {} mods on\n", p.name, p.mods))
                    .collect()
            })
        }
        ProfileCommand::Save { game, name, setup } => {
            let target = ModsTarget::open(db, game, setup.hints()).await?;
            let saved = gmm_core::save_profile(db, &target.ws, target.mods, game, &name).await?;
            output(json, &saved, || {
                format!("saved profile {:?} ({} mods on)\n", saved.name, saved.mods)
            })
        }
        ProfileCommand::Apply { game, name, setup } => {
            let profile = by_name(db, game, &name).await?;
            let target = ModsTarget::open(db, game, setup.hints()).await?;
            let applied =
                gmm_core::apply_profile(db, &target.ws, target.mods, game, profile.id).await?;
            output(json, &applied, || {
                let mut text = format!("applied {:?}\n", profile.name);
                for n in &applied.switched_on {
                    text.push_str(&format!("  on:  {n}\n"));
                }
                for n in &applied.switched_off {
                    text.push_str(&format!("  off: {n}\n"));
                }
                for n in &applied.missing {
                    text.push_str(&format!(
                        "  missing (not in the Mods folder any more): {n}\n"
                    ));
                }
                text
            })
        }
        ProfileCommand::Rename {
            game,
            name,
            new_name,
        } => {
            let profile = by_name(db, game, &name).await?;
            gmm_core::rename_profile(db, game, profile.id, &new_name).await?;
            output(json, &new_name, || format!("renamed to {new_name:?}\n"))
        }
        ProfileCommand::Delete { game, name } => {
            let profile = by_name(db, game, &name).await?;
            gmm_core::delete_profile(db, game, profile.id).await?;
            output(json, &profile, || {
                format!("deleted profile {:?} (no mod was touched)\n", profile.name)
            })
        }
        ProfileCommand::Export { game, name } => {
            let profile = by_name(db, game, &name).await?;
            let code = gmm_core::export_profile(db, game, profile.id).await?;
            output(json, &code, || format!("{code}\n"))
        }
        ProfileCommand::Import {
            game,
            code,
            name,
            setup,
        } => {
            let code = if code == "-" {
                let mut text = String::new();
                std::io::stdin()
                    .take(gmm_core::MAX_SHARE_CODE as u64 + 1)
                    .read_to_string(&mut text)?;
                text
            } else {
                code
            };
            let hints: SetupHints = setup.hints();
            let target = ModsTarget::open(db, game, hints).await?;
            let imported =
                gmm_core::import_profile(db, &target.ws, target.mods, game, &code, name.as_deref())
                    .await?;
            output(json, &imported, || {
                let mut text = format!(
                    "imported profile {:?} ({} mods on)\n",
                    imported.profile.name, imported.profile.mods
                );
                for m in &imported.missing {
                    text.push_str(&format!(
                        "  not installed: {} (`gmm install {}`)\n",
                        m.name, m.gb_mod_id
                    ));
                }
                text
            })
        }
    }
}
