// SPDX-License-Identifier: MIT OR Apache-2.0

//! The GMM desktop app. A thin layer: every command calls into `gmm-core`, the same code the
//! `gmm` CLI runs. Commands take ids (game ids, mod ids), never paths (security rule R5).

// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod error;
mod views;

#[cfg(test)]
mod bindings;

use std::sync::Arc;

use gmm_catalog::GameBanana;
use gmm_core::{AppDirs, ImageCache, open_db};
use gmm_db::Db;
use gmm_net::{Client, NetConfig, RateLimit};
use tauri::http::{Response, StatusCode, header};
use tauri::{Manager, Runtime, plugin::TauriPlugin};

/// Shared by every command.
pub struct AppState {
    pub db: Db,
    pub net: Client,
    pub gb: GameBanana,
    pub images: Arc<ImageCache>,
    pub dirs: AppDirs,
}

impl AppState {
    async fn open() -> Result<Self, Box<dyn std::error::Error>> {
        let dirs = AppDirs::system()?;
        let db = open_db(&dirs).await?;
        let net = Client::new(NetConfig::production())?;
        let gb = GameBanana::new(net.clone(), Arc::new(db.clone()));
        // Images get their own rate limit so a page of thumbnails doesn't hold up API calls.
        let image_net = Client::new(NetConfig {
            rate_limit: RateLimit {
                per_second: 8.0,
                burst: 16,
            },
            ..NetConfig::production()
        })?;
        let images = Arc::new(ImageCache::open(&dirs.cache, image_net)?);
        Ok(Self {
            db,
            net,
            gb,
            images,
            dirs,
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

fn main() {
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
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::games,
            commands::statuses,
            commands::catalog_list,
            commands::catalog_search,
            commands::catalog_mod,
            commands::catalog_updates,
            commands::catalog_categories,
            commands::install,
            commands::library,
            commands::set_enabled,
            commands::uninstall_preview,
            commands::uninstall,
            commands::conflicts,
            commands::storage,
            commands::clear_download_cache,
            commands::launch_plan,
            commands::launch,
            commands::settings,
            commands::pick_xxmi_folder,
            commands::clear_xxmi_folder,
            commands::pick_game_folder,
            commands::remove_game_folder,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("GMM could not start: {e}");
        std::process::exit(1);
    }
}
