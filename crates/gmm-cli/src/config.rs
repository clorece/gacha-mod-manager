// SPDX-License-Identifier: MIT OR Apache-2.0

//! The database the CLI shares with the app, and `gmm config`: what GMM remembers between
//! runs (game folders detection can't find, the XXMI folder).

use std::fmt::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use gmm_adapters::Registry;
use gmm_db::{DB_FILE_NAME, Db};
use gmm_fsguard::{Access, Guard};
use serde::Serialize;

use crate::format::bytes;
use crate::{AnyError, setup};

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Show what GMM remembers.
    Show,
    /// Remember a game folder that detection doesn't find, as GAME=FOLDER.
    AddFolder {
        #[arg(value_parser = setup::parse_folder)]
        folder: (String, PathBuf),
    },
    /// Forget a game folder added with `add-folder`, as GAME=FOLDER.
    RemoveFolder {
        #[arg(value_parser = setup::parse_folder)]
        folder: (String, PathBuf),
    },
    /// Remember the XXMI Launcher folder (for portable installs detection doesn't find).
    SetXxmi { folder: PathBuf },
    /// Go back to finding XXMI automatically.
    ClearXxmi,
}

/// `%APPDATA%\GMM` on Windows, the XDG data folder elsewhere.
fn data_dir() -> Result<PathBuf, AnyError> {
    Ok(gmm_core::AppDirs::system()?.data)
}

/// Opens (creating on first run) GMM's database.
pub async fn open_db() -> Result<Db, AnyError> {
    let mut guard = Guard::new();
    let data = guard.ensure_root("data", &data_dir()?, Access::ReadWrite)?;
    let file = guard.path(data, DB_FILE_NAME)?;
    Ok(Db::open(&guard, &file).await?)
}

#[derive(Serialize)]
struct FolderRow {
    game: String,
    folder: PathBuf,
}

#[derive(Serialize)]
struct CacheRow {
    entries: u64,
    bytes: u64,
}

#[derive(Serialize)]
struct ConfigReport {
    database: PathBuf,
    schema_version: i64,
    xxmi_root: Option<PathBuf>,
    confirmed_xxmi_exe: Option<PathBuf>,
    manual_folders: Vec<FolderRow>,
    http_cache: CacheRow,
    download_cache: CacheRow,
}

pub async fn run(db: &Db, cmd: ConfigCommand, json: bool) -> Result<(), AnyError> {
    match cmd {
        ConfigCommand::Show => show(db, json).await,
        ConfigCommand::AddFolder {
            folder: (game_id, folder),
        } => {
            let game = Registry::builtin()?
                .get(&game_id)
                .ok_or_else(|| format!("unknown game {game_id:?}"))?;
            if !folder.is_absolute() {
                return Err(format!("{} is not an absolute path", folder.display()).into());
            }
            let resolved = gmm_detect::resolve_game_folder(game, &folder).ok_or_else(|| {
                format!(
                    "{} is not a {} folder (expected {})",
                    folder.display(),
                    game.game.name,
                    game.detect.exe_names.join(" or ")
                )
            })?;
            if db.add_manual_game_folder(game.id(), &resolved).await? {
                println!(
                    "remembered {} folder {}",
                    game.game.name,
                    resolved.display()
                );
            } else {
                println!("already remembered: {}", resolved.display());
            }
            Ok(())
        }
        ConfigCommand::RemoveFolder {
            folder: (game_id, folder),
        } => {
            let mut removed = db.remove_manual_game_folder(&game_id, &folder).await?;
            // The stored form is the resolved game folder; accept the folder the user typed
            // when adding it, too.
            if !removed
                && let Some(game) = Registry::builtin()?.get(&game_id)
                && let Some(resolved) = gmm_detect::resolve_game_folder(game, &folder)
            {
                removed = db.remove_manual_game_folder(&game_id, &resolved).await?;
            }
            if removed {
                println!("forgot {}", folder.display());
                Ok(())
            } else {
                Err(format!("no remembered {game_id} folder {}", folder.display()).into())
            }
        }
        ConfigCommand::SetXxmi { folder } => {
            let install = gmm_xxmi::open_xxmi(&folder).ok_or_else(|| {
                format!(
                    "{} is not an XXMI Launcher folder (expected \"XXMI Launcher Config.json\" \
                     and \"Resources\\Bin\\XXMI Launcher.exe\" inside it)",
                    folder.display()
                )
            })?;
            db.set_xxmi_root(Some(&install.root)).await?;
            println!("remembered XXMI Launcher folder {}", install.root.display());
            Ok(())
        }
        ConfigCommand::ClearXxmi => {
            db.set_xxmi_root(None).await?;
            println!("XXMI will be found automatically");
            Ok(())
        }
    }
}

async fn show(db: &Db, json: bool) -> Result<(), AnyError> {
    let (http_entries, http_bytes) = db.http_cache_stats().await?;
    let report = ConfigReport {
        database: data_dir()?.join(DB_FILE_NAME),
        schema_version: db.schema_version().await?,
        xxmi_root: db.xxmi_root().await?,
        confirmed_xxmi_exe: db.confirmed_xxmi_exe().await?,
        manual_folders: db
            .manual_game_folders()
            .await?
            .into_iter()
            .map(|f| FolderRow {
                game: f.game_id,
                folder: f.folder,
            })
            .collect(),
        http_cache: CacheRow {
            entries: http_entries,
            bytes: http_bytes,
        },
        download_cache: CacheRow {
            entries: u64::try_from(db.cached_downloads().await?.len()).unwrap_or(u64::MAX),
            bytes: db.download_cache_bytes().await?,
        },
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    let mut out = String::new();
    let path = |p: &Option<PathBuf>, unset: &str| {
        p.as_deref()
            .map_or_else(|| unset.to_owned(), |p: &Path| p.display().to_string())
    };
    let _ = writeln!(
        out,
        "database:      {} (schema v{})",
        report.database.display(),
        report.schema_version
    );
    let _ = writeln!(
        out,
        "xxmi folder:   {}",
        path(&report.xxmi_root, "(found automatically)")
    );
    let _ = writeln!(
        out,
        "xxmi exe:      {}",
        path(&report.confirmed_xxmi_exe, "(not confirmed yet)")
    );
    if report.manual_folders.is_empty() {
        let _ = writeln!(out, "game folders:  (none added)");
    } else {
        let _ = writeln!(out, "game folders:");
        for f in &report.manual_folders {
            let _ = writeln!(out, "  {:<6} {}", f.game, f.folder.display());
        }
    }
    let _ = writeln!(
        out,
        "http cache:    {} responses, {}",
        report.http_cache.entries,
        bytes(report.http_cache.bytes)
    );
    let _ = writeln!(
        out,
        "downloads:     {} archives, {}",
        report.download_cache.entries,
        bytes(report.download_cache.bytes)
    );
    print!("{out}");
    Ok(())
}
