// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm list`, `gmm conflicts`, `gmm storage`.

use std::fmt::Write;
use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_core::{
    Conflict, DiskState, ModsTarget, RecycleBin, SetupHints, UnmanagedMod, conflicts, library,
    on_disk_name,
};
use gmm_db::Db;
use serde::Serialize;

use crate::AnyError;
use crate::format::bytes;

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
        match ModsTarget::open(
            db,
            game,
            SetupHints {
                xxmi: xxmi.clone(),
                folders: folders.clone(),
            },
        )
        .await
        {
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

/// `A  <->  B  (3 shared: 0fc8027f, 9040e2c4, ...)`
pub fn describe(c: &Conflict) -> String {
    let shown: Vec<&str> = c.hashes.iter().take(4).map(String::as_str).collect();
    let more = if c.hashes.len() > shown.len() {
        ", ..."
    } else {
        ""
    };
    format!(
        "{}  <->  {}  ({} shared: {}{more})",
        c.a.label(),
        c.b.label(),
        c.hashes.len(),
        shown.join(", ")
    )
}

/// `gmm conflicts`: every conflict between enabled mods, per game.
pub async fn show_conflicts(
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
    let mut reports = Vec::new();
    let mut text = String::new();
    for game in games {
        let target = match ModsTarget::open(
            db,
            game,
            SetupHints {
                xxmi: xxmi.clone(),
                folders: folders.clone(),
            },
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                let _ = writeln!(text, "{}: not checked ({e})", game.game.name);
                continue;
            }
        };
        let report = conflicts(db, &target.ws, target.mods, game).await?;
        let _ = writeln!(
            text,
            "{}: {} enabled mod(s) checked, {} conflict(s)",
            game.game.name,
            report.mods_checked,
            report.conflicts.len()
        );
        for c in &report.conflicts {
            let _ = writeln!(text, "  {}", describe(c));
        }
        for lib in &report.libraries {
            let _ = writeln!(text, "  library (ignored): {}", lib.label());
        }
        for note in &report.notes {
            let _ = writeln!(text, "  note: {note}");
        }
        reports.push(report);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        print!("{text}");
    }
    Ok(())
}

/// `gmm storage`: space used per mod and per game, the cache, and free space.
pub async fn storage(
    db: &Db,
    game: Option<&'static GameManifest>,
    clear_downloads: bool,
    xxmi: Option<PathBuf>,
    folders: Vec<(String, PathBuf)>,
    json: bool,
) -> Result<(), AnyError> {
    let ws = gmm_core::Workspace::open(&gmm_core::AppDirs::system()?.cache)?;
    if clear_downloads {
        let cleared = gmm_core::clear_downloads(db, &ws, &RecycleBin).await?;
        if json {
            println!("{}", serde_json::to_string_pretty(&cleared)?);
        } else {
            println!(
                "moved {} cached download(s) ({}) to the Recycle Bin",
                cleared.archives,
                bytes(cleared.bytes)
            );
        }
        return Ok(());
    }

    let games: Vec<&GameManifest> = match game {
        Some(g) => vec![g],
        None => Registry::builtin()?.games().iter().collect(),
    };
    #[derive(Serialize)]
    struct Out {
        games: Vec<gmm_core::GameStorage>,
        cache: gmm_core::CacheStorage,
        not_checked: Vec<String>,
    }
    let mut out = Out {
        games: Vec::new(),
        cache: gmm_core::cache_storage(db, &ws).await?,
        not_checked: Vec::new(),
    };
    for game in games {
        match ModsTarget::open(
            db,
            game,
            SetupHints {
                xxmi: xxmi.clone(),
                folders: folders.clone(),
            },
        )
        .await
        {
            Ok(target) => {
                out.games
                    .push(gmm_core::game_storage(db, &target.ws, target.mods, game).await?);
            }
            Err(e) => out.not_checked.push(format!("{}: {e}", game.game.name)),
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    let size =
        |s: &gmm_core::Size| format!("{}{}", bytes(s.bytes), if s.truncated { "+" } else { "" });
    let free = |f: Option<u64>| f.map_or("?".to_owned(), bytes);
    let mut text = String::new();
    for g in &out.games {
        let name = Registry::builtin()?
            .get(&g.game)
            .map_or(g.game.clone(), |m| m.game.name.clone());
        let _ = writeln!(
            text,
            "{name}: GMM mods {} ({} mods), other mods {} ({}), drive free {}",
            size(&g.managed_total),
            g.managed.len(),
            size(&g.unmanaged_total),
            g.unmanaged.len(),
            free(g.free_bytes)
        );
        for m in g.managed.iter().chain(&g.unmanaged) {
            let _ = writeln!(
                text,
                "  {:>10}  {}  {}",
                size(&m.size),
                if m.enabled { "on " } else { "off" },
                m.mod_ref.label()
            );
        }
        if g.other.bytes > 0 {
            let _ = writeln!(text, "  {:>10}  other files", size(&g.other));
        }
    }
    for line in &out.not_checked {
        let _ = writeln!(text, "{line}");
    }
    let c = &out.cache;
    let _ = writeln!(
        text,
        "cache: downloads {} ({} files), API responses {} ({}), drive free {}",
        size(&c.downloads),
        c.downloads.files,
        bytes(c.http_bytes),
        c.http_entries,
        free(c.free_bytes)
    );
    if c.extract.bytes > 0 {
        let _ = writeln!(text, "  leftover extraction folders: {}", size(&c.extract));
    }
    print!("{text}");
    Ok(())
}
