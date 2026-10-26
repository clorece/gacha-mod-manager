// SPDX-License-Identifier: MIT OR Apache-2.0

//! The GMM desktop app. A thin layer: every command calls into `gmm-core`, the same code the
//! `gmm` CLI runs. Commands take ids (game ids, mod ids), never paths (security rule R5).

// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_update;
mod commands;
mod error;
mod html;
mod local;
mod open;
mod profiles;
mod queue;
mod report;
mod startup;
mod views;
mod window;

#[cfg(test)]
mod bindings;
#[cfg(test)]
mod ipc_fuzz;

use std::sync::Arc;

use gmm_catalog::GameBanana;
use gmm_core::{AppDirs, ImageCache, InstallQueue, open_db};
use gmm_db::Db;
use gmm_net::{Client, NetConfig, RateLimit};
use tauri::http::{Response, StatusCode, header};
use tauri::{Manager, Runtime, plugin::TauriPlugin};

/// Shared by every command.
pub struct AppState {
    pub db: Db,
    pub net: Client,
    pub gb: GameBanana,
    /// For background reads (download counts on cards): its own slow rate limit, the same
    /// response cache.
    pub gb_background: GameBanana,
    pub images: Arc<ImageCache>,
    /// Link keys handed out in rewritten descriptions (`open_link`).
    pub links: html::Links,
    /// The archive dropped or picked for a local install (`local.rs`).
    pub local: local::Local,
    pub dirs: AppDirs,
    /// The Downloads page's installs, run one at a time by `queue::spawn_worker`.
    pub queue: InstallQueue,
    /// Wakes the worker when an install is queued or resumed.
    pub wake: tokio::sync::Notify,
    pub watchers: queue::Watchers,
    /// Errors kept for bug reports (`report.rs`).
    pub errors: Arc<gmm_core::ErrorLog>,
    /// The app update check's GitHub client (`app_update.rs`).
    pub releases: Client,
    /// The newer GMM the last check found: the only version `open_app_release` opens.
    pub newest_release: std::sync::Mutex<Option<String>>,
}

impl AppState {
    async fn open() -> Result<Self, Box<dyn std::error::Error>> {
        Self::open_with(AppDirs::system()?, NetConfig::production()).await
    }

    /// The IPC fuzz harness opens it on temporary folders with a network that refuses
    /// everything (`ipc_fuzz.rs`).
    async fn open_with(dirs: AppDirs, net: NetConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let db = open_db(&dirs).await?;
        // Every client shares one connection pool and one "slow down" state: they all reach
        // GameBanana from the same IP address (docs/10-gamebanana-api.md).
        // The app update check reads GMM's releases list on GitHub through a client of its own:
        // its own allowlist (that list only), connections and slow-down state, so GitHub can
        // never hold up GameBanana traffic (R2).
        let releases = Client::new(NetConfig {
            allowlist: net.allowlist.for_github_releases(),
            rate_limit: RateLimit {
                per_second: 0.2,
                burst: 2,
            },
            ..net.clone()
        })?;
        let net = Client::new(net)?;
        let gb = GameBanana::new(net.clone(), Arc::new(db.clone()));
        // Background reads (card counters) spend the API budget too, at most 1/s of it.
        let background_net = net.sibling(
            RateLimit {
                per_second: 1.0,
                burst: 2,
            },
            true,
        );
        let gb_background = GameBanana::new(background_net, Arc::new(db.clone()));
        // Images come from GameBanana's image host, not the API, so a page of thumbnails
        // gets its own rate and doesn't hold up API calls.
        let image_net = net.sibling(
            RateLimit {
                per_second: 8.0,
                burst: 16,
            },
            false,
        );
        let images = Arc::new(ImageCache::open(&dirs.cache, image_net)?);
        let errors = Arc::new(dirs.error_log()?);
        Ok(Self {
            db,
            net,
            gb,
            gb_background,
            images,
            links: html::Links::default(),
            local: local::Local::default(),
            dirs,
            queue: InstallQueue::new(),
            wake: tokio::sync::Notify::new(),
            watchers: queue::Watchers::default(),
            errors,
            releases,
            newest_release: std::sync::Mutex::new(None),
        })
    }
}

/// The webview only ever shows GMM's own pages: any navigation elsewhere (a link in a mod
/// description, say) is refused. Links to GameBanana open through a command instead.
fn navigation_guard<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("gmm-navigation-guard")
        .on_navigation(|_webview, url| {
            let host = url.host_str().unwrap_or_default();
            match url.scheme() {
                // Production assets (`tauri://localhost` elsewhere, `http://tauri.localhost`
                // on Windows) and the Vite server in development.
                "tauri" => host == "localhost",
                "http" | "https" => {
                    host == "tauri.localhost"
                        || (cfg!(debug_assertions)
                            && host == "localhost"
                            && url.port() == Some(1420))
                }
                _ => false,
            }
        })
        .build()
}

fn image_response(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "private, max-age=604800, immutable")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// Every command the webview may call (each also listed in build.rs and allowed in
/// capabilities/main.json). Generic over the runtime, so the IPC fuzz harness runs the same
/// list on Tauri's mock runtime.
fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::games,
        commands::statuses,
        commands::catalog_list,
        commands::catalog_search,
        commands::catalog_mod,
        commands::catalog_updates,
        commands::catalog_categories,
        commands::catalog_featured,
        commands::catalog_stats,
        commands::install,
        queue::queue_install,
        queue::queue_proceed,
        queue::queue_change_file,
        queue::queue_jobs,
        queue::queue_watch,
        queue::queue_pause,
        queue::queue_resume,
        queue::queue_cancel,
        queue::queue_dismiss,
        queue::queue_clear_finished,
        startup::startup_check,
        app_update::app_update_check,
        app_update::open_app_release,
        commands::library,
        commands::switch_version,
        commands::rollback_files,
        commands::set_enabled,
        commands::uninstall_preview,
        commands::uninstall,
        commands::mod_keybindings,
        commands::adopt_candidates,
        commands::adopt,
        local::drop_watch,
        local::pick_archive,
        local::prepare_local,
        local::install_local,
        local::discard_local,
        commands::conflicts,
        commands::storage,
        commands::clear_download_cache,
        commands::clear_http_cache,
        commands::clear_image_cache,
        report::error_log,
        report::clear_error_log,
        report::open_log_folder,
        report::log_ui_error,
        report::bug_report_draft,
        report::open_bug_report,
        commands::network_status,
        commands::launch_plan,
        commands::launch,
        commands::settings,
        commands::preferences,
        commands::set_preferences,
        commands::pick_xxmi_folder,
        commands::clear_xxmi_folder,
        commands::pick_game_folder,
        commands::remove_game_folder,
        open::open_mod_page,
        open::open_xxmi_releases,
        open::open_link,
        open::open_mod_folder,
        profiles::profiles,
        profiles::profile_save,
        profiles::profile_apply,
        profiles::profile_rename,
        profiles::profile_delete,
        profiles::profile_export,
        profiles::profile_import,
        window::window_minimize,
        window::window_toggle_maximize,
        window::window_is_maximized,
        window::window_close,
        window::window_start_drag,
    ]
}

fn main() {
    // A panic goes to the error log too (where and what, nothing else), then on as usual.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a panic".to_owned());
        let at = info
            .location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();
        error::record("panic", &format!("{what}{at}"));
        default_hook(info);
    }));
    let result = tauri::Builder::default()
        .plugin(navigation_guard())
        // Only called from Rust (folder pickers); the webview has no dialog permissions.
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol(
            views::IMAGE_SCHEME,
            |ctx, request, responder| {
                let images = ctx.app_handle().state::<AppState>().images.clone();
                // `/<key>`: the opaque key a catalogue command handed out.
                let key = request.uri().path().trim_start_matches('/').to_owned();
                tauri::async_runtime::spawn(async move {
                    let response = match images.get(&key).await {
                        Ok(image) => image_response(StatusCode::OK, image.mime, image.bytes),
                        Err(gmm_core::CoreError::UnknownImage) => {
                            image_response(StatusCode::NOT_FOUND, "text/plain", Vec::new())
                        }
                        Err(_) => image_response(StatusCode::BAD_GATEWAY, "text/plain", Vec::new()),
                    };
                    responder.respond(response);
                });
            },
        )
        .setup(|app| {
            let state = tauri::async_runtime::block_on(AppState::open())?;
            error::install_log(state.errors.clone());
            app.manage(state);
            queue::spawn_worker(app.handle().clone());
            // The window starts hidden, so it appears once, at its size for this screen.
            if let Some(main) = app.get_webview_window("main") {
                window::fit_to_screen(&main);
                main.show()?;
            }
            Ok(())
        })
        // Files dropped on the window reach Rust, not the webview (R5): a local install.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                local::dropped(window.app_handle(), paths);
            }
        })
        .invoke_handler(handler())
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("GMM could not start: {e}");
        std::process::exit(1);
    }
}
