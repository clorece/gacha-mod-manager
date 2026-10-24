// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm list`, `gmm enable`, `gmm disable`, `gmm uninstall`.

use std::collections::HashMap;
use std::fmt::Write;
use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_core::{
    DiskState, RecycleBin, UnmanagedMod, library, on_disk_name, set_enabled, uninstall,
};
use gmm_db::Db;
use serde::Serialize;

use crate::AnyError;
use crate::format::bytes;
use crate::install::ModsTarget;

#[derive(Serialize)]
struct ListRow {
    id: i64,
    name: String,
    folder: String,
    enabled: bool,
    /// `enabled`, `disabled`, `missing`, `ambiguous`, or `unknown` when XXMI wasn't found.
    disk: String,
    variant: Option<String>,
    gb_mod_id: Option<u64>,
    gb_file_id: Option<u64>,
    version: Option<String>,
    size_bytes: u64,
    installed_at: i64,
}

#[derive(Serialize)]
struct GameList {
    game: String,
    game_name: String,
    mods_dir: Option<PathBuf>,
    /// Why the Mods folder couldn't be checked, if it couldn't.
    problem: Option<String>,
    managed: Vec<ListRow>,
    unmanaged: Vec<UnmanagedMod>,
}

pub async fn list(
    db: &Db,
    game: Option<&'static GameManifest>,
    xxmi: Option<PathBuf>,
    folders: Vec<(String, PathBuf)>,
    json: bool,
) -> Result<(), AnyError> {
    let games: Vec<&GameManifest> = match game {
        Some(g) => vec![g],
        None => Registry::builtin()?.games().iter().collect(),
    };
    let mut out = Vec::new();
    for game in games {
        let mut entry = GameList {
            game: game.id().to_owned(),
            game_name: game.game.name.clone(),
            mods_dir: None,
            problem: None,
            managed: Vec::new(),
            unmanaged: Vec::new(),
        };
        match ModsTarget::open(db, game, xxmi.clone(), folders.clone()).await {
            Ok(target) => {
                let lib = library(db, &target.ws, target.mods, game).await?;
                entry.mods_dir = Some(target.mods_dir);
                entry.managed = lib
                    .managed
                    .into_iter()
                    .map(|m| row(m.row, disk_label(m.disk)))
                    .collect();
                entry.unmanaged = lib.unmanaged;
            }
            Err(e) => {
                entry.problem = Some(e.to_string());
                entry.managed = db
                    .installed_mods(Some(game.id()))
                    .await?
                    .into_iter()
                    .map(|m| row(m, "unknown"))
                    .collect();
            }
        }
        out.push(entry);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    let mut text = String::new();
    for g in &out {
        let _ = writeln!(text, "{} ({})", g.game_name, g.game);
        if let Some(dir) = &g.mods_dir {
            let _ = writeln!(text, "  {}", dir.display());
        }
        if let Some(problem) = &g.problem {
            let _ = writeln!(text, "  Mods folder not checked: {problem}");
        }
        if g.managed.is_empty() {
            let _ = writeln!(text, "  no mods installed by GMM");
        }
        for r in &g.managed {
            let state = match r.disk.as_str() {
                "enabled" => "on ",
                "disabled" => "off",
                "missing" => "MISSING",
                "ambiguous" => "BOTH",
                _ => "?  ",
            };
            let _ = writeln!(
                text,
                "  {:>4}  {state}  {:<9} {}{}",
                r.id,
                bytes(r.size_bytes),
                r.folder,
                r.version
                    .as_deref()
                    .map(|v| format!("  v{v}"))
                    .unwrap_or_default()
            );
        }
        if !g.unmanaged.is_empty() {
            let _ = writeln!(text, "  not installed by GMM (left alone):");
            for u in &g.unmanaged {
                let _ = writeln!(
                    text,
                    "        {}  {}",
                    if u.enabled { "on " } else { "off" },
                    u.path
                );
            }
        }
    }
    print!("{text}");
    Ok(())
}

fn disk_label(disk: DiskState) -> &'static str {
    match disk {
        DiskState::Enabled => "enabled",
        DiskState::Disabled => "disabled",
        DiskState::Missing => "missing",
        DiskState::Ambiguous => "ambiguous",
    }
}

fn row(m: gmm_db::InstalledMod, disk: &str) -> ListRow {
    ListRow {
        folder: on_disk_name(&m.folder_name, m.enabled),
        id: m.id,
        name: m.name,
        enabled: m.enabled,
        disk: disk.to_owned(),
        variant: m.variant,
        gb_mod_id: m.gb_mod_id,
        gb_file_id: m.gb_file_id,
        version: m.version,
        size_bytes: m.size_bytes,
        installed_at: m.installed_at,
    }
}

/// What `enable`, `disable` and `uninstall` do to each mod.
#[derive(Clone, Copy)]
pub enum Action {
    Enable,
    Disable,
    Uninstall,
}

/// Applies `action` to each mod id (from `gmm list`). Mods of different games are fine.
pub async fn apply(
    db: &Db,
    action: Action,
    ids: &[i64],
    xxmi: Option<PathBuf>,
    folders: Vec<(String, PathBuf)>,
    json: bool,
) -> Result<(), AnyError> {
    let registry = Registry::builtin()?;
    let mut targets: HashMap<String, ModsTarget> = HashMap::new();
    let mut results = Vec::new();
    for &id in ids {
        let row = db
            .installed_mod(id)
            .await?
            .ok_or_else(|| format!("no mod {id} installed by GMM (see `gmm list`)"))?;
        let game = registry
            .get(&row.game_id)
            .ok_or_else(|| format!("mod {id} belongs to an unknown game {:?}", row.game_id))?;
        if !targets.contains_key(game.id()) {
            let target = ModsTarget::open(db, game, xxmi.clone(), folders.clone()).await?;
            targets.insert(game.id().to_owned(), target);
        }
        let Some(target) = targets.get(game.id()) else {
            continue;
        };
        let value = match action {
            Action::Enable | Action::Disable => {
                let on = matches!(action, Action::Enable);
                let t = set_enabled(db, &target.ws, target.mods, game, id, on).await?;
                if !json {
                    let verb = if on { "enabled" } else { "disabled" };
                    if t.changed {
                        println!("{verb} {:?}", t.folder_name);
                    } else {
                        println!("{:?} was already {verb}", t.folder_name);
                    }
                }
                serde_json::to_value(t)?
            }
            Action::Uninstall => {
                let u = uninstall(db, &target.ws, target.mods, game, id, &RecycleBin).await?;
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
        };
        results.push(value);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    }
    Ok(())
}
