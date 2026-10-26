// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm`: the command-line front end. Every backend feature lands here before it gets a
//! screen (docs/05-roadmap.md).

mod config;
mod fetch;
mod format;
mod install;
mod launch;
mod library;
mod profile;
mod report;
mod setup;
mod toggle;
mod update;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::{CatalogSource, GameBanana, ListQuery, SearchQuery, Sort};
use gmm_core::SetupHints;
use gmm_db::Db;
use gmm_net::{Client, NetConfig};
use serde::Serialize;
use toggle::Action;

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
    /// GMM's local error log (errors only; your home folder shows as `~`).
    Log {
        /// Delete the log for good.
        #[arg(long)]
        clear: bool,
        /// How many of the newest entries to show.
        #[arg(long, default_value_t = 50)]
        last: usize,
    },
    /// Print a link to a pre-filled GitHub issue (what you write, versions and setup states;
    /// never a path). Nothing is sent: open the link, check it and submit it on GitHub.
    ReportBug {
        /// The issue's title.
        #[arg(long)]
        title: String,
        /// What happened, what you expected, and the steps.
        #[arg(long, default_value = "")]
        text: String,
        #[command(flatten)]
        setup: SetupArgs,
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
    /// Download a mod into GMM's cache, extract it safely and print what's inside (scan
    /// report, variants). Nothing is installed.
    Fetch {
        mod_id: u64,
        /// File id from `gmm catalog show`. Defaults to the newest clean file.
        #[arg(long)]
        file: Option<u64>,
    },
    /// Download a mod and install it into XXMI's Mods folder (under `Mods/GMM/`).
    Install {
        mod_id: u64,
        /// File id from `gmm catalog show`. Defaults to the newest clean file.
        #[arg(long)]
        file: Option<u64>,
        /// The game, if GameBanana doesn't say (normally detected from the mod page).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        /// Which variant to install when the archive holds several, by number or name
        /// (see `gmm fetch`). Can be repeated; each becomes its own mod folder.
        #[arg(long = "variant")]
        variants: Vec<String>,
        /// Install every variant in the archive, each as its own mod folder.
        #[arg(long, conflicts_with = "variants")]
        all_variants: bool,
        /// Install even though the mod contains executables or scripts; those files are left
        /// out (they stay only in the cached archive and are never run).
        #[arg(long)]
        skip_blocked: bool,
        /// XXMI Launcher folder, if it isn't found automatically (portable install). Overrides
        /// `gmm config set-xxmi` for this run.
        #[arg(long)]
        xxmi: Option<PathBuf>,
        /// Extra game folder to check, as GAME=FOLDER. Can be repeated.
        #[arg(long = "folder", value_parser = setup::parse_folder)]
        folders: Vec<(String, PathBuf)>,
    },
    /// Move an installed GameBanana mod to its newest clean file, keeping each folder's name
    /// and on/off state (the previous file stays cached for `gmm rollback`). Without a mod
    /// id, list the installed mods that have a newer file.
    Update {
        /// GameBanana mod id (see `gmm list`).
        gb_mod_id: Option<u64>,
        /// A specific file id (see `gmm catalog show`) instead of the newest.
        #[arg(long, requires = "gb_mod_id")]
        file: Option<u64>,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Move an installed GameBanana mod back to an earlier file from the download cache
    /// (the newest other one, or --file). Nothing is downloaded.
    Rollback {
        /// GameBanana mod id (see `gmm list`).
        gb_mod_id: u64,
        /// A specific cached file id.
        #[arg(long)]
        file: Option<u64>,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Install a mod from a .zip, .7z or .rar archive on this computer, with the same checks
    /// as a download. The archive is copied into GMM's cache; your file is left alone.
    InstallFile {
        file: PathBuf,
        /// The game it's for (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: &'static GameManifest,
        /// Which variant to install when the archive holds several (repeatable).
        #[arg(long = "variant")]
        variants: Vec<String>,
        #[arg(long, conflicts_with = "variants")]
        all_variants: bool,
        /// Install without executables or scripts found inside.
        #[arg(long)]
        skip_blocked: bool,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// List each game's mods: the ones GMM installed (checked against the disk) and the
    /// ones it didn't.
    List {
        /// Only this game (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Switch mods on: GMM's by id (`12` or `#12`), hand-installed ones by name or path, as
    /// `gmm list` shows them. Refuses when a mod would conflict with one that's already on,
    /// unless --anyway.
    Enable {
        #[arg(required = true)]
        mods: Vec<String>,
        /// Only look in this game for hand-installed mods (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        /// Enable even if it conflicts with another enabled mod.
        #[arg(long)]
        anyway: bool,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Switch mods off: XXMI skips folders named `DISABLED ...`. GMM's mods by id, hand-installed
    /// ones by name or path (see `gmm list`).
    Disable {
        #[arg(required = true)]
        mods: Vec<String>,
        /// Only look in this game for hand-installed mods (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Show a mod's key bindings (its [Key...] sections): which keys switch its parts in
    /// game. GMM's mods by id, hand-installed ones by name or path (see `gmm list`).
    Keys {
        r#mod: String,
        /// Only look in this game for hand-installed mods (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Take over a hand-installed mod as a GameBanana mod (update checks, updates, its page).
    /// Without --gb, list the GameBanana mods it could be.
    Adopt {
        /// The hand-installed mod's name or path (see `gmm list`).
        r#mod: String,
        /// The GameBanana mod id it is.
        #[arg(long)]
        gb: Option<u64>,
        /// Which of its files it is (default: the newest clean one).
        #[arg(long, requires = "gb")]
        file: Option<u64>,
        /// Only look in this game (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Profiles: named sets of switched-on mods per game, shareable as a code.
    #[command(subcommand)]
    Profile(profile::ProfileCommand),
    /// Show enabled mods that replace the same things (managed and hand-installed).
    Conflicts {
        /// Only this game (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Space used by each mod, each game's Mods folder and GMM's cache, and free space.
    Storage {
        /// Only this game (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        /// Delete every cached download for good (installed mods stay).
        #[arg(long)]
        clear_downloads: bool,
        /// Delete every cached preview image for good (they download again when shown).
        #[arg(long)]
        clear_images: bool,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Start a game with mods: runs XXMI Launcher with `--nogui --xxmi <importer>`, like its
    /// Quick Start shortcut. The first time, you confirm the launcher's path.
    Launch {
        /// The game (see `gmm games`).
        #[arg(value_parser = parse_game)]
        game: &'static GameManifest,
        /// Confirm the XXMI Launcher path without asking (first launch, or after it moved).
        #[arg(long)]
        yes: bool,
        #[command(flatten)]
        setup: SetupArgs,
    },
    /// Delete mods for good. GMM's mods by id (their downloaded archives go too once no
    /// version is left); hand-installed ones by name or path, after listing them and asking.
    Uninstall {
        #[arg(required = true)]
        mods: Vec<String>,
        /// Only look in this game for hand-installed mods (see `gmm games`).
        #[arg(long, value_parser = parse_game)]
        game: Option<&'static GameManifest>,
        /// Don't ask before deleting hand-installed mods.
        #[arg(long)]
        yes: bool,
        #[command(flatten)]
        setup: SetupArgs,
    },
}

/// Where to find XXMI and the games, when detection needs help.
#[derive(clap::Args, Clone, Default)]
struct SetupArgs {
    /// XXMI Launcher folder, if it isn't found automatically (portable install). Overrides
    /// `gmm config set-xxmi` for this run.
    #[arg(long)]
    xxmi: Option<PathBuf>,
    /// Extra game folder to check, as GAME=FOLDER. Can be repeated.
    #[arg(long = "folder", value_parser = setup::parse_folder)]
    folders: Vec<(String, PathBuf)>,
}

impl SetupArgs {
    fn hints(self) -> SetupHints {
        SetupHints {
            xxmi: self.xxmi,
            folders: self.folders,
        }
    }
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
            // Kept for bug reports (`gmm log`); best effort.
            if let Ok(log) = gmm_core::AppDirs::system().and_then(|d| d.error_log()) {
                log.record("cli", &e.to_string());
            }
            ExitCode::FAILURE
        }
    }
}

type AnyError = Box<dyn std::error::Error>;

async fn run(cli: Cli) -> Result<(), AnyError> {
    if let Command::Games = cli.command {
        return games(cli.json);
    }
    if let Command::Log { clear, last } = cli.command {
        return report::log(clear, last, cli.json);
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
            let (xxmi, folders) = gmm_core::saved_setup(db, SetupHints { xxmi, folders }).await?;
            setup::run_detect(cli.json, xxmi, &folders)
        }
        Command::Status {
            game,
            xxmi,
            folders,
        } => {
            let (xxmi, folders) = gmm_core::saved_setup(db, SetupHints { xxmi, folders }).await?;
            setup::run_status(cli.json, game, xxmi, &folders)
        }
        Command::Config(cmd) => config::run(db, cmd, cli.json).await,
        Command::ReportBug { title, text, setup } => {
            report::report_bug(db, &title, &text, setup.hints(), cli.json).await
        }
        Command::Log { .. } => unreachable!("handled before the database opens"),
        Command::Catalog(cmd) => catalog(&gb, cmd, cli.json, cli.show_nsfw).await,
        Command::Fetch { mod_id, file } => fetch::run(&gb, &net, db, mod_id, file, cli.json).await,
        Command::Install {
            mod_id,
            file,
            game,
            variants,
            all_variants,
            skip_blocked,
            xxmi,
            folders,
        } => {
            let args = install::InstallArgs {
                mod_id,
                file,
                game,
                variants,
                all_variants,
                skip_blocked,
                xxmi,
                folders,
            };
            install::run(&gb, &net, db, args, cli.json).await
        }
        Command::List { game, setup } => {
            library::list(db, game, setup.xxmi, setup.folders, cli.json).await
        }
        Command::InstallFile {
            file,
            game,
            variants,
            all_variants,
            skip_blocked,
            setup,
        } => {
            let args = install::LocalArgs {
                file,
                game,
                variants,
                all_variants,
                skip_blocked,
                xxmi: setup.xxmi,
                folders: setup.folders,
            };
            install::run_local(db, args, cli.json).await
        }
        Command::Update {
            gb_mod_id,
            file,
            setup,
        } => {
            let args = update::ChangeArgs {
                gb_mod_id,
                file,
                rollback: false,
                xxmi: setup.xxmi,
                folders: setup.folders,
            };
            update::run(&gb, &net, db, args, cli.json).await
        }
        Command::Rollback {
            gb_mod_id,
            file,
            setup,
        } => {
            let args = update::ChangeArgs {
                gb_mod_id: Some(gb_mod_id),
                file,
                rollback: true,
                xxmi: setup.xxmi,
                folders: setup.folders,
            };
            update::run(&gb, &net, db, args, cli.json).await
        }
        Command::Enable {
            mods,
            game,
            anyway,
            setup,
        } => {
            let action = Action::Enable { anyway };
            toggle::apply(db, action, &mods, game, setup.hints(), cli.json).await
        }
        Command::Disable { mods, game, setup } => {
            toggle::apply(db, Action::Disable, &mods, game, setup.hints(), cli.json).await
        }
        Command::Storage {
            game,
            clear_downloads,
            clear_images,
            setup,
        } => {
            library::storage(
                db,
                game,
                clear_downloads,
                clear_images,
                setup.xxmi,
                setup.folders,
                cli.json,
            )
            .await
        }
        Command::Launch { game, yes, setup } => {
            let hints = SetupHints {
                xxmi: setup.xxmi,
                folders: setup.folders,
            };
            launch::run(db, game, yes, hints, cli.json).await
        }
        Command::Keys { r#mod, game, setup } => {
            toggle::keys(db, &r#mod, game, setup.hints(), cli.json).await
        }
        Command::Profile(cmd) => profile::run(db, cmd, cli.json).await,
        Command::Adopt {
            r#mod,
            gb: gb_mod,
            file,
            game,
            setup,
        } => toggle::adopt(&gb, db, &r#mod, game, gb_mod, file, setup.hints(), cli.json).await,
        Command::Conflicts { game, setup } => {
            library::show_conflicts(db, game, setup.xxmi, setup.folders, cli.json).await
        }
        Command::Uninstall {
            mods,
            game,
            yes,
            setup,
        } => {
            let action = Action::Uninstall { yes };
            toggle::apply(db, action, &mods, game, setup.hints(), cli.json).await
        }
    }
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
    if show { 0 } else { page.hide_sensitive() }
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
