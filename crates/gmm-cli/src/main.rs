// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm`: the command-line front end. Every backend feature lands here before it gets a
//! screen (docs/05-roadmap.md).

mod fetch;
mod format;

use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use gmm_catalog::{CatalogSource, GameBanana, ListQuery, MemoryCache, SearchQuery, Sort};
use gmm_net::{Client, NetConfig};

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
    /// Browse the GameBanana catalogue.
    #[command(subcommand)]
    Catalog(CatalogCommand),
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
        #[arg(long, value_enum)]
        game: Game,
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
        #[arg(long, value_enum)]
        game: Game,
        text: String,
        #[arg(long, default_value_t = 1)]
        page: u32,
    },
    /// Show a mod page: details, files, credits.
    Show { mod_id: u64 },
    /// List a game's categories, or a category's subcategories (characters).
    Categories {
        #[arg(long, value_enum)]
        game: Game,
        #[arg(long)]
        parent: Option<u64>,
    },
}

/// Supported games. Until the adapter registry exists, this is the GameBanana id table.
#[derive(Clone, Copy, ValueEnum)]
enum Game {
    Zzz,
    Wuwa,
}

impl Game {
    fn gamebanana_id(self) -> u64 {
        match self {
            Self::Zzz => 19567,
            Self::Wuwa => 20357,
        }
    }
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
    let net = Client::new(NetConfig::production())?;
    let gb = GameBanana::new(net.clone(), Arc::new(MemoryCache::default()));
    match cli.command {
        Command::Catalog(cmd) => catalog(&gb, cmd, cli.json, cli.show_nsfw).await,
        Command::Fetch { mod_id, file } => fetch::run(&gb, &net, mod_id, file, cli.json).await,
    }
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
                ..ListQuery::new(game.gamebanana_id())
            };
            let mut result = gb.list_mods(&query).await?;
            let hidden = hide_sensitive(&mut result, show_nsfw);
            output(json, &result, || format::mod_page(&result, hidden))
        }
        CatalogCommand::Search { game, text, page } => {
            let query = SearchQuery {
                game_id: game.gamebanana_id(),
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
                None => gb.categories(game.gamebanana_id()).await?,
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
