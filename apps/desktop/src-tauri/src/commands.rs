// SPDX-License-Identifier: MIT OR Apache-2.0

//! The commands the webview may call: one per CLI action. Each is listed in `build.rs`,
//! granted in `capabilities/main.json` and registered in `main.rs`. Arguments are ids (game
//! ids, GameBanana ids, GMM mod ids, keys from an earlier listing), never paths or URLs
//! (security rule R5). The only paths that enter come from native folder pickers opened here,
//! and they are validated before anything is saved.

use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::{CatalogSource, Category, ListQuery, ModProfile, ModUpdate, Page, SearchQuery};
use gmm_core::{
    ClearedCache, ConflictReport, InstallEvent, InstallOptions, InstallRequest, Machine,
    ModsTarget, RecycleBin, SetupHints, VariantChoice, cache_storage, clear_downloads, confirm_exe,
    conflicts_for, conflicts_for_unmanaged, game_storage, library as core_library,
    listed_unmanaged, plan_launch, set_enabled as core_set_enabled, set_unmanaged_enabled_at,
    start, uninstall as core_uninstall, uninstall_unmanaged, unmanaged_size,
};
use gmm_xxmi::GameStatus;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::AppState;
use crate::error::{CmdError, CmdResult};
use crate::views::{
    CatalogPage, CatalogQuery, FolderRow, GameInfo, GameStorageView, InstallArgs, InstallProgress,
    InstallResult, LaunchInfo, Launched, LibraryView, ModKey, ModRow, Settings, StorageView,
    Toggled, UninstallItem, Uninstalled, VariantPick, rewrite_categories, rewrite_profile,
    rewrite_summary,
};

fn registry() -> CmdResult<&'static Registry> {
    Registry::builtin().map_err(|e| CmdError::other(e.to_string()))
}

fn game_by_id(id: &str) -> CmdResult<&'static GameManifest> {
    registry()?
        .get(id)
        .ok_or_else(|| CmdError::other(format!("unknown game {id:?}")))
}

async fn mods_target(state: &AppState, game: &GameManifest) -> CmdResult<ModsTarget> {
    Ok(ModsTarget::open(&state.db, game, SetupHints::default()).await?)
}

/// The supported games, in display order.
#[tauri::command]
pub fn games() -> CmdResult<Vec<GameInfo>> {
    Ok(registry()?
        .games()
        .iter()
        .map(|g| GameInfo {
            id: g.id().to_owned(),
            name: g.game.name.clone(),
            short_name: g.game.short_name.clone(),
            importer: g.xxmi.importer.as_str().to_owned(),
            gamebanana_id: g.gamebanana.game_id,
        })
        .collect())
}

/// Every game's setup state with the steps to fix it (what `gmm status` prints).
#[tauri::command]
pub async fn statuses(state: State<'_, AppState>) -> CmdResult<Vec<GameStatus>> {
    let machine = Machine::scan(&state.db, SetupHints::default()).await?;
    Ok(registry()?
        .games()
        .iter()
        .map(|g| machine.status(g))
        .collect())
}

fn hide_sensitive(page: &mut Page<gmm_catalog::ModSummary>, show: bool) -> u32 {
    if show {
        return 0;
    }
    let before = page.items.len();
    page.items
        .retain(|m| !m.visibility.is_sensitive() && !m.has_content_ratings);
    u32::try_from(before - page.items.len()).unwrap_or(u32::MAX)
}

fn catalog_page(
    state: &AppState,
    mut page: Page<gmm_catalog::ModSummary>,
    show_nsfw: bool,
) -> CatalogPage {
    let hidden = hide_sensitive(&mut page, show_nsfw);
    for item in &mut page.items {
        rewrite_summary(&state.images, item);
    }
    CatalogPage { page, hidden }
}

/// A page of a game's catalogue.
#[tauri::command]
pub async fn catalog_list(
    state: State<'_, AppState>,
    query: CatalogQuery,
) -> CmdResult<CatalogPage> {
    let game = game_by_id(&query.game)?;
    let list = ListQuery {
        page: query.page.max(1),
        per_page: query.per_page,
        sort: query.sort,
        category_id: query.category,
        ..ListQuery::new(game.gamebanana.game_id)
    };
    let page = state.gb.list_mods(&list).await?;
    Ok(catalog_page(&state, page, query.show_nsfw))
}

/// Mods of a game whose name matches `text`.
#[tauri::command]
pub async fn catalog_search(
    state: State<'_, AppState>,
    game: String,
    text: String,
    page: u32,
    show_nsfw: bool,
) -> CmdResult<CatalogPage> {
    let game = game_by_id(&game)?;
    let query = SearchQuery {
        game_id: game.gamebanana.game_id,
        text,
        page: page.max(1),
    };
    let result = state.gb.search(&query).await?;
    Ok(catalog_page(&state, result, show_nsfw))
}

/// A mod page: details, files, credits.
#[tauri::command]
pub async fn catalog_mod(state: State<'_, AppState>, mod_id: u64) -> CmdResult<ModProfile> {
    let mut profile = state.gb.mod_profile(mod_id).await?;
    rewrite_profile(&state.images, &mut profile);
    Ok(profile)
}

/// A page of a mod's update notes.
#[tauri::command]
pub async fn catalog_updates(
    state: State<'_, AppState>,
    mod_id: u64,
    page: u32,
) -> CmdResult<Page<ModUpdate>> {
    Ok(state.gb.mod_updates(mod_id, page.max(1)).await?)
}

/// A game's root categories, or the children of `parent` (for skins: one per character).
#[tauri::command]
pub async fn catalog_categories(
    state: State<'_, AppState>,
    game: String,
    parent: Option<u64>,
) -> CmdResult<Vec<Category>> {
    let game = game_by_id(&game)?;
    let mut categories = match parent {
        Some(id) => state.gb.subcategories(id).await?,
        None => state.gb.categories(game.gamebanana.game_id).await?,
    };
    rewrite_categories(&state.images, &mut categories);
    Ok(categories)
}

/// Downloads a mod and installs it under `Mods/GMM/` (the whole `gmm install` pipeline).
/// Progress goes to `on_progress`. When the archive holds several mods or contains blocked
/// files, the error says so (`choose_variant`, `blocked`) and the UI asks, then calls again;
/// the archive is then reused from the cache.
#[tauri::command]
pub async fn install(
    state: State<'_, AppState>,
    args: InstallArgs,
    on_progress: Channel<InstallProgress>,
) -> CmdResult<InstallResult> {
    let profile = state.gb.mod_profile(args.mod_id).await?;
    let game = profile
        .summary
        .game_id
        .and_then(|id| registry().ok()?.by_gamebanana_id(id))
        .ok_or_else(|| {
            CmdError::other(format!("mod {} isn't for a game GMM supports", args.mod_id))
        })?;
    let target = mods_target(&state, game).await?;
    let request = InstallRequest {
        mod_id: args.mod_id,
        file_id: args.file_id,
        options: InstallOptions {
            variants: match args.variants {
                VariantPick::Only => VariantChoice::Only,
                VariantPick::Named { names } => VariantChoice::Named(names),
                VariantPick::All => VariantChoice::All,
            },
            skip_blocked: args.skip_blocked,
        },
    };
    let report = gmm_core::install(
        &state.gb,
        &state.net,
        &state.db,
        &target.ws,
        target.mods,
        game,
        &request,
        |event| {
            let progress = match event {
                InstallEvent::Fetch(gmm_core::FetchEvent::UsingCache) => {
                    InstallProgress::UsingCache
                }
                InstallEvent::Fetch(gmm_core::FetchEvent::Downloading { size }) => {
                    InstallProgress::Downloading { size }
                }
                InstallEvent::Fetch(gmm_core::FetchEvent::Progress(p)) => {
                    InstallProgress::Progress {
                        downloaded: p.downloaded,
                        total: p.total,
                    }
                }
                InstallEvent::Extracting => InstallProgress::Extracting,
                InstallEvent::Installing { folder } => InstallProgress::Installing { folder },
            };
            // The window may have closed; installing carries on regardless.
            let _ = on_progress.send(progress);
        },
    )
    .await?;

    // New mods start enabled; say right away if they clash with something already on.
    let mut conflicts = Vec::new();
    for folder in &report.installed {
        let found = conflicts_for(&state.db, &target.ws, target.mods, game, folder.id).await?;
        conflicts.extend(found.conflicts);
    }
    conflicts.dedup();
    Ok(InstallResult {
        report,
        setup_warnings: target.warnings,
        conflicts,
    })
}

/// A game's mods: GMM's (checked against the disk) and everything else in `Mods/`.
#[tauri::command]
pub async fn library(state: State<'_, AppState>, game: String) -> CmdResult<LibraryView> {
    let game = game_by_id(&game)?;
    let mut view = LibraryView {
        game: game.id().to_owned(),
        problem: None,
        managed: Vec::new(),
        unmanaged: Vec::new(),
    };
    let target = match mods_target(&state, game).await {
        Ok(target) => target,
        Err(e) => {
            view.problem = Some(message(&e));
            return Ok(view);
        }
    };
    let lib = core_library(&state.db, &target.ws, target.mods, game).await?;
    view.managed = lib
        .managed
        .into_iter()
        .map(|m| ModRow {
            folder: gmm_core::on_disk_name(&m.row.folder_name, m.row.enabled),
            id: m.row.id,
            name: m.row.name,
            enabled: m.row.enabled,
            disk: m.disk,
            variant: m.row.variant,
            gb_mod_id: m.row.gb_mod_id,
            version: m.row.version,
            size_bytes: m.row.size_bytes,
            installed_at: m.row.installed_at,
        })
        .collect();
    view.unmanaged = lib.unmanaged;
    Ok(view)
}

fn message(e: &CmdError) -> String {
    match e {
        CmdError::ChooseVariant { message, .. }
        | CmdError::Blocked { message, .. }
        | CmdError::Conflicts { message, .. }
        | CmdError::NotReady { message }
        | CmdError::Other { message } => message.clone(),
    }
}

/// Switches a mod on or off (the `DISABLED ` rename). Switching on refuses with `conflicts`
/// when the mod would clash with one that's already on, unless `anyway`.
#[tauri::command]
pub async fn set_enabled(
    state: State<'_, AppState>,
    game: String,
    key: ModKey,
    enabled: bool,
    anyway: bool,
) -> CmdResult<Toggled> {
    let game = game_by_id(&game)?;
    let target = mods_target(&state, game).await?;
    let (db, ws, mods) = (&state.db, &target.ws, target.mods);
    if enabled && !anyway {
        let report = match &key {
            ModKey::Managed { id } => conflicts_for(db, ws, mods, game, *id).await?,
            ModKey::Unmanaged { path } => conflicts_for_unmanaged(db, ws, mods, game, path).await?,
        };
        if !report.conflicts.is_empty() {
            return Err(CmdError::Conflicts {
                message: format!(
                    "switching this on would conflict with {} mod(s) that are already on",
                    report.conflicts.len()
                ),
                conflicts: report.conflicts,
            });
        }
    }
    let changed = match &key {
        ModKey::Managed { id } => {
            core_set_enabled(db, ws, mods, game, *id, enabled)
                .await?
                .changed
        }
        ModKey::Unmanaged { path } => {
            set_unmanaged_enabled_at(db, ws, mods, game, path, enabled)
                .await?
                .changed
        }
    };
    Ok(Toggled {
        key,
        enabled,
        changed,
    })
}

/// What [`uninstall`] would remove, with full paths and sizes, for the confirmation dialog
/// (security rule R8: hand-installed mods are never removed without an itemized yes).
#[tauri::command]
pub async fn uninstall_preview(
    state: State<'_, AppState>,
    game: String,
    keys: Vec<ModKey>,
) -> CmdResult<Vec<UninstallItem>> {
    let game = game_by_id(&game)?;
    let target = mods_target(&state, game).await?;
    let lib = core_library(&state.db, &target.ws, target.mods, game).await?;
    let mut items = Vec::with_capacity(keys.len());
    for key in keys {
        let item = match &key {
            ModKey::Managed { id } => {
                let m = lib
                    .managed
                    .iter()
                    .find(|m| m.row.id == *id)
                    .ok_or_else(|| CmdError::other(format!("no mod #{id} in {}", game.id())))?;
                let folder = gmm_core::on_disk_name(&m.row.folder_name, m.row.enabled);
                UninstallItem {
                    name: m.row.name.clone(),
                    folder: target.mods_dir.join(gmm_core::GMM_DIR).join(folder),
                    files: None,
                    bytes: m.row.size_bytes,
                    truncated: false,
                    hand_installed: false,
                    key,
                }
            }
            ModKey::Unmanaged { path } => {
                let u = listed_unmanaged(&lib, game, path)?;
                let size = unmanaged_size(&target.ws, target.mods, u)?;
                UninstallItem {
                    name: u.name.clone(),
                    folder: target
                        .mods_dir
                        .join(u.path.replace('/', std::path::MAIN_SEPARATOR_STR)),
                    files: Some(size.files),
                    bytes: size.bytes,
                    truncated: size.truncated,
                    hand_installed: true,
                    key,
                }
            }
        };
        items.push(item);
    }
    Ok(items)
}

/// Moves mods to the Recycle Bin. GMM's mods by id (the archive stays in the cache); others
/// only by the exact path [`uninstall_preview`] listed, after the user confirmed.
#[tauri::command]
pub async fn uninstall(
    state: State<'_, AppState>,
    game: String,
    keys: Vec<ModKey>,
) -> CmdResult<Vec<Uninstalled>> {
    let game = game_by_id(&game)?;
    let target = mods_target(&state, game).await?;
    let (db, ws, mods) = (&state.db, &target.ws, target.mods);
    let mut done = Vec::with_capacity(keys.len());
    for key in keys {
        let trashed = match &key {
            ModKey::Managed { id } => {
                core_uninstall(db, ws, mods, game, *id, &RecycleBin)
                    .await?
                    .trashed
            }
            ModKey::Unmanaged { path } => {
                uninstall_unmanaged(db, ws, mods, game, path, &RecycleBin).await?;
                true
            }
        };
        done.push(Uninstalled { key, trashed });
    }
    Ok(done)
}

/// Enabled mods of a game that replace the same things.
#[tauri::command]
pub async fn conflicts(state: State<'_, AppState>, game: String) -> CmdResult<ConflictReport> {
    let game = game_by_id(&game)?;
    let target = mods_target(&state, game).await?;
    Ok(gmm_core::conflicts(&state.db, &target.ws, target.mods, game).await?)
}

/// Space used per mod and per game, by GMM's cache, and free space.
#[tauri::command]
pub async fn storage(state: State<'_, AppState>) -> CmdResult<StorageView> {
    let machine = Machine::scan(&state.db, SetupHints::default()).await?;
    let mut games = Vec::new();
    for game in registry()?.games() {
        let mut view = GameStorageView {
            game: game.id().to_owned(),
            problem: None,
            storage: None,
        };
        match ModsTarget::from_machine(&machine, game) {
            Ok(t) => view.storage = Some(game_storage(&state.db, &t.ws, t.mods, game).await?),
            Err(e) => view.problem = Some(e.to_string()),
        }
        games.push(view);
    }
    let ws = gmm_core::Workspace::open(&state.dirs.cache)?;
    let cache = cache_storage(&state.db, &ws).await?;
    Ok(StorageView { games, cache })
}

/// Moves every cached download to the Recycle Bin. Installed mods stay.
#[tauri::command]
pub async fn clear_download_cache(state: State<'_, AppState>) -> CmdResult<ClearedCache> {
    let ws = gmm_core::Workspace::open(&state.dirs.cache)?;
    Ok(clear_downloads(&state.db, &ws, &RecycleBin).await?)
}

async fn plan(state: &AppState, game: &GameManifest) -> CmdResult<gmm_core::LaunchPlan> {
    let machine = Machine::scan(&state.db, SetupHints::default()).await?;
    let xxmi = machine.xxmi.as_ref().ok_or_else(|| CmdError::NotReady {
        message: "XXMI Launcher not found".to_owned(),
    })?;
    let status = machine.status(game);
    Ok(plan_launch(&state.db, xxmi, &status, game).await?)
}

/// What "Launch with XXMI" would run, and whether the user already confirmed that exe.
#[tauri::command]
pub async fn launch_plan(state: State<'_, AppState>, game: String) -> CmdResult<LaunchInfo> {
    let plan = plan(&state, game_by_id(&game)?).await?;
    Ok(LaunchInfo {
        exe: plan.command.exe,
        args: plan.command.args.to_vec(),
        confirmed: plan.confirmed,
    })
}

/// Starts the game through XXMI. `confirm` records the user's yes to the exe that
/// [`launch_plan`] showed (needed on first launch, and again if the exe moved).
#[tauri::command]
pub async fn launch(
    state: State<'_, AppState>,
    game: String,
    confirm: bool,
) -> CmdResult<Launched> {
    let mut plan = plan(&state, game_by_id(&game)?).await?;
    if !plan.confirmed && confirm {
        confirm_exe(&state.db, &mut plan).await?;
    }
    Ok(Launched { pid: start(&plan)? })
}

/// What GMM remembers between runs.
#[tauri::command]
pub async fn settings(state: State<'_, AppState>) -> CmdResult<Settings> {
    let db = &state.db;
    let (http_cache_entries, http_cache_bytes) = db.http_cache_stats().await?;
    Ok(Settings {
        database: state.dirs.data.join(gmm_db::DB_FILE_NAME),
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
        http_cache_entries,
        http_cache_bytes,
        download_cache_entries: u64::try_from(db.cached_downloads().await?.len())
            .unwrap_or(u64::MAX),
        download_cache_bytes: db.download_cache_bytes().await?,
    })
}

/// A folder the user picks in the system dialog, or `None` if they cancel.
async fn pick_folder(app: &AppHandle, title: &str) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .pick_folder(move |folder| {
            let _ = tx.send(folder.and_then(|f| f.into_path().ok()));
        });
    rx.await.ok().flatten()
}

/// Asks for the XXMI Launcher folder (portable installs detection doesn't find) and
/// remembers it if it really is one. `None` when the user cancels.
#[tauri::command]
pub async fn pick_xxmi_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<PathBuf>> {
    let Some(folder) = pick_folder(&app, "Choose the XXMI Launcher folder").await else {
        return Ok(None);
    };
    let install = gmm_xxmi::open_xxmi(&folder).ok_or_else(|| {
        CmdError::other(format!(
            "{} is not an XXMI Launcher folder (expected \"XXMI Launcher Config.json\" and \
             \"Resources\\Bin\\XXMI Launcher.exe\" inside it)",
            folder.display()
        ))
    })?;
    state.db.set_xxmi_root(Some(&install.root)).await?;
    Ok(Some(install.root))
}

/// Goes back to finding XXMI automatically.
#[tauri::command]
pub async fn clear_xxmi_folder(state: State<'_, AppState>) -> CmdResult<()> {
    Ok(state.db.set_xxmi_root(None).await?)
}

/// Asks for a game folder detection didn't find and remembers it if it holds the game.
/// `None` when the user cancels.
#[tauri::command]
pub async fn pick_game_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    game: String,
) -> CmdResult<Option<PathBuf>> {
    let game = game_by_id(&game)?;
    let title = format!("Choose the {} folder", game.game.name);
    let Some(folder) = pick_folder(&app, &title).await else {
        return Ok(None);
    };
    let resolved = gmm_detect::resolve_game_folder(game, &folder).ok_or_else(|| {
        CmdError::other(format!(
            "{} is not a {} folder (expected {})",
            folder.display(),
            game.game.name,
            game.detect.exe_names.join(" or ")
        ))
    })?;
    state
        .db
        .add_manual_game_folder(game.id(), &resolved)
        .await?;
    Ok(Some(resolved))
}

/// Forgets a game folder [`settings`] listed. The folder is only matched against what's
/// stored; nothing on disk is touched.
#[tauri::command]
pub async fn remove_game_folder(
    state: State<'_, AppState>,
    game: String,
    folder: PathBuf,
) -> CmdResult<()> {
    let game = game_by_id(&game)?;
    let stored = state
        .db
        .manual_game_folders()
        .await?
        .into_iter()
        .find(|f| f.game_id == game.id() && f.folder == folder)
        .ok_or_else(|| CmdError::other("that folder isn't remembered"))?;
    state
        .db
        .remove_manual_game_folder(&stored.game_id, &stored.folder)
        .await?;
    Ok(())
}
