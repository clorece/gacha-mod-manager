// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm`: the command-line front end. Every backend feature lands here before it gets a
//! screen (docs/05-roadmap.md).

mod config;
mod fetch;
mod format;
mod setup;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::{CatalogSource, GameBanana, ListQuery, SearchQuery, Sort};
use gmm_db::Db;
use gmm_net::{Client, NetConfig};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "gmm", version, about = "Gacha Mod Manager (command line)")]
struct Cli {
    /// Print machine-readable JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Include mods GameBanana flags as sensitive (hidden by default, like in the app).
    #[arg(long, global = true)]
    show_nsfw: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the supported games.
    Games,
    /// Find installed games (Steam, HoYoPlay, Epic) and XXMI. Read-only.
    Detect {
        /// XXMI Launcher folder, if it isn't found automatically (portable install). Overrides
        /// `gmm config set-xxmi` for this run.
        #[arg(long)]
        xxmi: Option<PathBuf>,
        /// Extra game folder to check, as GAME=FOLDER. Can be repeated. Folders saved with
        /// `gmm config add-folder` are always checked.
        #[arg(long = "folder", value_parser = setup::parse_folder)]
        folders: Vec<(String, PathBuf)>,
    },
    /// Check each game's mod setup and say what to do next. Read-only.
    Status {
        /// Only this game (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        /// XXMI Launcher folder, if it isn't found automatically (portable install). Overrides
        /// `gmm config set-xxmi` for this run.
        #[arg(long)]
        xxmi: Option<PathBuf>,
        /// Extra game folder to check, as GAME=FOLDER. Can be repeated. Folders saved with
        /// `gmm config add-folder` are always checked.
        #[arg(long = "folder", value_parser = setup::parse_folder)]
        folders: Vec<(String, PathBuf)>,
    },
    /// Browse the GameBanana catalogue.
    #[command(subcommand)]
    Catalog(CatalogCommand),
    /// What GMM remembers between runs: game folders, the XXMI folder.
    #[command(subcommand)]
    Config(config::ConfigCommand),
    /// Download a mod into GMM's cache, extract it safely and print the scan report.
    /// Nothing is installed into XXMI yet.
    Fetch {
        mod_id: u64,
        /// File id from `gmm catalog show`. Defaults to the newest clean file.
        #[arg(long)]
        file: Option<u64>,
    },
}

#[derive(Subcommand)]
enum CatalogCommand {
    /// List mods for a game.
    List {
        /// Game id (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: &'static GameManifest,
        #[arg(long, default_value_t = 1)]
        page: u32,
        #[arg(long, default_value_t = 20)]
        per_page: u32,
        #[arg(long, value_enum, default_value_t = SortArg::Default)]
        sort: SortArg,
        /// Category or character id (see `gmm catalog categories`).
        #[arg(long)]
        category: Option<u64>,
    },
    /// Search mods by name.
    Search {
        #[arg(long, value_parser = parse_game)]
        game: &'static GameManifest,
        text: String,
        #[arg(long, default_value_t = 1)]
        page: u32,
    },
    /// Show a mod page: details, files, credits.
    Show { mod_id: u64 },
    /// List a game's categories, or a category's subcategories (characters).
    Categories {
        #[arg(long, value_parser = parse_game)]
        game: &'static GameManifest,
        #[arg(long)]
        parent: Option<u64>,
    },
}

fn parse_game(id: &str) -> Result<&'static GameManifest, String> {
    let registry = Registry::builtin().map_err(|e| e.to_string())?;
    registry.get(id).ok_or_else(|| {
        let known: Vec<_> = registry.games().iter().map(GameManifest::id).collect();
        format!("unknown game {id:?} (supported: {})", known.join(", "))
    })
}

#[derive(Serialize)]
struct GameRow<'a> {
    id: &'a str,
    name: &'a str,
    gamebanana_id: u64,
    xxmi_importer: &'static str,
    steam_appid: Option<u32>,
}

#[derive(Clone, Copy, ValueEnum)]
enum SortArg {
    Default,
    Newest,
    Updated,
    Downloads,
    Likes,
    Views,
}

impl From<SortArg> for Sort {
    fn from(s: SortArg) -> Self {
        match s {
            SortArg::Default => Sort::Default,
            SortArg::Newest => Sort::Newest,
            SortArg::Updated => Sort::RecentlyUpdated,
            SortArg::Downloads => Sort::MostDownloaded,
            SortArg::Likes => Sort::MostLiked,
            SortArg::Views => Sort::MostViewed,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: could not start: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type AnyError = Box<dyn std::error::Error>;

async fn run(cli: Cli) -> Result<(), AnyError> {
    if let Command::Games = cli.command {
        return games(cli.json);
    }
    let db = config::open_db().await?;
    let result = run_with_db(cli, &db).await;
    db.close().await;
    result
}

async fn run_with_db(cli: Cli, db: &Db) -> Result<(), AnyError> {
    let net = Client::new(NetConfig::production())?;
    let gb = GameBanana::new(net.clone(), Arc::new(db.clone()));
    match cli.command {
        Command::Games => games(cli.json),
        Command::Detect { xxmi, folders } => {
            let (xxmi, folders) = with_saved_setup(db, xxmi, folders).await?;
            setup::run_detect(cli.json, xxmi, &folders)
        }
        Command::Status {
            game,
            xxmi,
            folders,
        } => {
            let (xxmi, folders) = with_saved_setup(db, xxmi, folders).await?;
            setup::run_status(cli.json, game, xxmi, &folders)
        }
        Command::Config(cmd) => config::run(db, cmd, cli.json).await,
        Command::Catalog(cmd) => catalog(&gb, cmd, cli.json, cli.show_nsfw).await,
        Command::Fetch { mod_id, file } => fetch::run(&gb, &net, db, mod_id, file, cli.json).await,
    }
}

/// Adds the saved game folders to the ones given on the command line, and falls back to the
/// saved XXMI folder when `--xxmi` isn't given.
async fn with_saved_setup(
    db: &Db,
    xxmi: Option<PathBuf>,
    folders: Vec<(String, PathBuf)>,
) -> Result<(Option<PathBuf>, Vec<(String, PathBuf)>), AnyError> {
    let xxmi = match xxmi {
        Some(dir) => Some(dir),
        None => db.xxmi_root().await?,
    };
    let mut all: Vec<(String, PathBuf)> = db
        .manual_game_folders()
        .await?
        .into_iter()
        .map(|f| (f.game_id, f.folder))
        .collect();
    all.extend(folders);
    Ok((xxmi, all))
}

fn games(json: bool) -> Result<(), AnyError> {
    let rows: Vec<GameRow> = Registry::builtin()?
        .games()
        .iter()
        .map(|g| GameRow {
            id: g.id(),
            name: &g.game.name,
            gamebanana_id: g.gamebanana.game_id,
            xxmi_importer: g.xxmi.importer.as_str(),
            steam_appid: g.detect.steam.as_ref().map(|s| s.appid),
        })
        .collect();
    output(json, &rows, || {
        rows.iter()
            .map(|r| {
                format!(
                    "{:<6} {:<20} GameBanana {:<6} XXMI {}  Steam {}\n",
                    r.id,
                    r.name,
                    r.gamebanana_id,
                    r.xxmi_importer,
                    r.steam_appid.map_or("-".to_owned(), |a| a.to_string())
                )
            })
            .collect()
    })
}

fn hide_sensitive(page: &mut gmm_catalog::Page<gmm_catalog::ModSummary>, show: bool) -> usize {
    if show {
        return 0;
    }
    let before = page.items.len();
    page.items
        .retain(|m| !m.visibility.is_sensitive() && !m.has_content_ratings);
    before - page.items.len()
}

async fn catalog(
    gb: &GameBanana,
    cmd: CatalogCommand,
    json: bool,
    show_nsfw: bool,
) -> Result<(), AnyError> {
    match cmd {
        CatalogCommand::List {
            game,
            page,
            per_page,
            sort,
            category,
        } => {
            let query = ListQuery {
                page,
                per_page,
                sort: sort.into(),
                category_id: category,
                ..ListQuery::new(game.gamebanana.game_id)
            };
            let mut result = gb.list_mods(&query).await?;
            let hidden = hide_sensitive(&mut result, show_nsfw);
            output(json, &result, || format::mod_page(&result, hidden))
        }
        CatalogCommand::Search { game, text, page } => {
            let query = SearchQuery {
                game_id: game.gamebanana.game_id,
                text,
                page,
            };
            let mut result = gb.search(&query).await?;
            let hidden = hide_sensitive(&mut result, show_nsfw);
            output(json, &result, || format::mod_page(&result, hidden))
        }
        CatalogCommand::Show { mod_id } => {
            let profile = gb.mod_profile(mod_id).await?;
            output(json, &profile, || format::profile(&profile))
        }
        CatalogCommand::Categories { game, parent } => {
            let cats = match parent {
                Some(id) => gb.subcategories(id).await?,
                None => gb.categories(game.gamebanana.game_id).await?,
            };
            output(json, &cats, || format::categories(&cats))
        }
    }
}

fn output<T: serde::Serialize>(
    json: bool,
    value: &T,
    text: impl FnOnce() -> String,
) -> Result<(), AnyError> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        print!("{}", text());
    }
    Ok(())
}
