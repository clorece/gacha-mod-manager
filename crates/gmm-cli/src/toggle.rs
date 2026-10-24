// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm enable`, `gmm disable`, `gmm uninstall`: for mods GMM installed (by id, `12` or `#12`)
//! and for mods installed by hand (by name or path, as `gmm list` shows them).

use std::collections::HashMap;
use std::io::{BufRead, IsTerminal, Write};

use gmm_adapters::{GameManifest, Registry};
use gmm_core::{
    ModsTarget, RecycleBin, SetupHints, conflicts_for, conflicts_for_unmanaged, find_unmanaged,
    library, set_enabled, set_unmanaged_enabled, uninstall, uninstall_unmanaged, unmanaged_size,
};
use gmm_db::Db;

use crate::AnyError;
use crate::format::bytes;
use crate::library::describe;

/// What to do to each mod.
#[derive(Clone, Copy)]
pub enum Action {
    /// `anyway`: switch on even if it conflicts with another enabled mod.
    Enable {
        anyway: bool,
    },
    Disable,
    /// `yes`: don't ask before moving hand-installed mods to the Recycle Bin.
    Uninstall {
        yes: bool,
    },
}

/// A mod as resolved from the command line.
enum Target {
    Managed {
        game: &'static GameManifest,
        id: i64,
        enabled: bool,
    },
    Unmanaged {
        game: &'static GameManifest,
        path: String,
        enabled: bool,
    },
}

impl Target {
    fn game(&self) -> &'static GameManifest {
        match self {
            Self::Managed { game, .. } | Self::Unmanaged { game, .. } => game,
        }
    }
}

/// `12` or `#12` is a GMM id; anything else names a mod installed by hand.
fn as_id(arg: &str) -> Option<i64> {
    arg.trim().trim_start_matches('#').parse().ok()
}

struct Targets<'a> {
    db: &'a Db,
    hints: SetupHints,
    open: HashMap<String, ModsTarget>,
}

impl Targets<'_> {
    async fn get(&mut self, game: &GameManifest) -> Result<&ModsTarget, AnyError> {
        if !self.open.contains_key(game.id()) {
            let target = ModsTarget::open(self.db, game, self.hints.clone()).await?;
            self.open.insert(game.id().to_owned(), target);
        }
        self.open
            .get(game.id())
            .ok_or_else(|| "internal error: Mods folder not open".into())
    }
}

pub async fn apply(
    db: &Db,
    action: Action,
    args: &[String],
    only_game: Option<&'static GameManifest>,
    hints: SetupHints,
    json: bool,
) -> Result<(), AnyError> {
    let registry = Registry::builtin()?;
    let mut targets = Targets {
        db,
        hints,
        open: HashMap::new(),
    };

    // Resolve everything first, so a typo in the third name doesn't leave the first two done.
    let mut resolved = Vec::with_capacity(args.len());
    for arg in args {
        let target = match as_id(arg) {
            Some(id) => {
                let Some(row) = db.installed_mod(id).await? else {
                    return Err(unknown_id(db, id).await?.into());
                };
                let game = registry.get(&row.game_id).ok_or_else(|| {
                    format!("mod {id} belongs to an unknown game {:?}", row.game_id)
                })?;
                Target::Managed {
                    game,
                    id,
                    enabled: row.enabled,
                }
            }
            None => find_by_name(&mut targets, registry, only_game, arg).await?,
        };
        resolved.push(target);
    }

    // Hand-installed mods are the user's own: list exactly what goes and ask first (R8).
    if let Action::Uninstall { yes } = action {
        let mut listed = Vec::new();
        for t in &resolved {
            if let Target::Unmanaged { game, path, .. } = t {
                let target = targets.get(game).await?;
                let lib = library(db, &target.ws, target.mods, game).await?;
                let size = lib
                    .unmanaged
                    .iter()
                    .find(|u| &u.path == path)
                    .map(|u| unmanaged_size(&target.ws, target.mods, u))
                    .transpose()?
                    .unwrap_or_default();
                listed.push(format!(
                    "{}  ({} files, {}){}",
                    target
                        .mods_dir
                        .join(path.replace('/', std::path::MAIN_SEPARATOR_STR))
                        .display(),
                    size.files,
                    bytes(size.bytes),
                    if size.truncated { "+" } else { "" }
                ));
            }
        }
        if !listed.is_empty() && !confirm(&listed, yes, json)? {
            return Err(
                "nothing was removed. To move these to the Recycle Bin, run again with --yes."
                    .into(),
            );
        }
    }

    let mut results = Vec::new();
    let mut renamed_hand_installed = false;
    for t in resolved {
        let game = t.game();
        let target = targets.get(game).await?;
        let (ws, mods) = (&target.ws, target.mods);
        let value = match (action, t) {
            (Action::Enable { anyway }, Target::Managed { id, enabled, .. }) => {
                if !anyway && !enabled {
                    refuse_conflicts(
                        &format!("#{id}"),
                        conflicts_for(db, ws, mods, game, id).await?,
                    )?;
                }
                let r = set_enabled(db, ws, mods, game, id, true).await?;
                say(json, r.changed, "enabled", &r.folder_name);
                serde_json::to_value(r)?
            }
            (Action::Disable, Target::Managed { id, .. }) => {
                let r = set_enabled(db, ws, mods, game, id, false).await?;
                say(json, r.changed, "disabled", &r.folder_name);
                serde_json::to_value(r)?
            }
            (Action::Enable { anyway }, Target::Unmanaged { path, enabled, .. }) => {
                if !anyway && !enabled {
                    let report = conflicts_for_unmanaged(db, ws, mods, game, &path).await?;
                    refuse_conflicts(&path, report)?;
                }
                let r = set_unmanaged_enabled(db, ws, mods, game, &path, true).await?;
                renamed_hand_installed |= r.changed;
                say_renamed(json, &r);
                serde_json::to_value(r)?
            }
            (Action::Disable, Target::Unmanaged { path, .. }) => {
                let r = set_unmanaged_enabled(db, ws, mods, game, &path, false).await?;
                renamed_hand_installed |= r.changed;
                say_renamed(json, &r);
                serde_json::to_value(r)?
            }
            (Action::Uninstall { .. }, Target::Managed { id, .. }) => {
                let u = uninstall(db, ws, mods, game, id, &RecycleBin).await?;
                if !json {
                    if u.trashed {
                        println!("moved {:?} to the Recycle Bin", u.folder_name);
                    } else {
                        println!(
                            "{:?} was already gone from the Mods folder; forgot it",
                            u.folder_name
                        );
                    }
                }
                serde_json::to_value(u)?
            }
            (Action::Uninstall { .. }, Target::Unmanaged { path, .. }) => {
                let gone = uninstall_unmanaged(db, ws, mods, game, &path, &RecycleBin).await?;
                if !json {
                    println!("moved {gone:?} to the Recycle Bin");
                }
                serde_json::json!({ "path": gone, "trashed": true })
            }
        };
        results.push(value);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else if renamed_hand_installed {
        println!(
            "note: 3DMigoto remembers in-game toggle choices by folder name, so the renamed \
             mod's choices start fresh."
        );
    }
    Ok(())
}

/// The hand-installed mod `name` refers to, in `only_game` or in any game whose Mods folder
/// can be read.
async fn find_by_name(
    targets: &mut Targets<'_>,
    registry: &'static Registry,
    only_game: Option<&'static GameManifest>,
    name: &str,
) -> Result<Target, AnyError> {
    let games: Vec<&'static GameManifest> = match only_game {
        Some(g) => vec![g],
        None => registry.games().iter().collect(),
    };
    let db = targets.db;
    let mut found = Vec::new();
    for game in games {
        let Ok(target) = targets.get(game).await else {
            continue;
        };
        let lib = library(db, &target.ws, target.mods, game).await?;
        for u in find_unmanaged(&lib, name) {
            found.push((game, u.path.clone(), u.enabled));
        }
    }
    match found.as_slice() {
        [(game, path, enabled)] => Ok(Target::Unmanaged {
            game,
            path: path.clone(),
            enabled: *enabled,
        }),
        [] => Err(format!(
            "no mod named {name:?} (see `gmm list`; mods GMM installed are named by their id)"
        )
        .into()),
        many => {
            let options: Vec<String> = many
                .iter()
                .map(|(g, p, _)| format!("{}: {p}", g.id()))
                .collect();
            Err(format!(
                "{name:?} matches more than one mod; give the full path from `gmm list` (and \
                 --game if needed):\n  {}",
                options.join("\n  ")
            )
            .into())
        }
    }
}

fn refuse_conflicts(what: &str, report: gmm_core::ConflictReport) -> Result<(), AnyError> {
    if report.conflicts.is_empty() {
        return Ok(());
    }
    let lines: Vec<String> = report.conflicts.iter().map(describe).collect();
    Err(format!(
        "enabling {what} would conflict with mods that are already on:\n  {}\nDisable the \
         other one first, or add --anyway.",
        lines.join("\n  ")
    )
    .into())
}

fn say(json: bool, changed: bool, verb: &str, folder: &str) {
    if json {
        return;
    }
    if changed {
        println!("{verb} {folder:?}");
    } else {
        println!("{folder:?} was already {verb}");
    }
}

fn say_renamed(json: bool, r: &gmm_core::UnmanagedToggle) {
    if json {
        return;
    }
    let verb = if r.enabled { "enabled" } else { "disabled" };
    if r.changed {
        println!("{verb} {:?} (renamed to {:?})", r.from, r.to);
    } else {
        println!("{:?} was already {verb}", r.from);
    }
}

/// Lists what would be moved to the Recycle Bin and asks. Without a terminal (or with
/// `--json`), only `--yes` confirms.
fn confirm(items: &[String], yes: bool, json: bool) -> Result<bool, AnyError> {
    if yes {
        return Ok(true);
    }
    if json || !std::io::stdin().is_terminal() {
        eprintln!("these mods weren't installed by GMM:");
        for item in items {
            eprintln!("  {item}");
        }
        return Ok(false);
    }
    println!("These mods weren't installed by GMM. They'll be moved to the Recycle Bin:");
    for item in items {
        println!("  {item}");
    }
    print!("Move them? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

/// The error for an id GMM doesn't know. GameBanana ids are much larger than GMM's and easy
/// to mix up with them, so if one matches an installed mod, say which GMM id it means.
async fn unknown_id(db: &Db, id: i64) -> Result<String, AnyError> {
    let matches = match u64::try_from(id) {
        Ok(gb) => db.installed_mods_for_gb(gb).await?,
        Err(_) => Vec::new(),
    };
    if matches.is_empty() {
        return Ok(format!("no mod {id} installed by GMM (see `gmm list`)"));
    }
    let ids: Vec<String> = matches
        .iter()
        .map(|m| format!("#{} {}", m.id, m.folder_name))
        .collect();
    Ok(format!(
        "{id} is a GameBanana mod id; use GMM's id instead: {}",
        ids.join(", ")
    ))
}
