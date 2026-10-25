// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the commands return, where `gmm-core`'s own types don't fit the UI as they are, and
//! the image rewrite: every GameBanana image URL in catalogue data is replaced with a
//! `gmmimg` URL for an opaque key (see `gmm_core::ImageCache`), so the webview never sees,
//! or requests, an outside address.

use std::path::PathBuf;

use gmm_catalog::{Category, Image, ImageVariant, Member, ModProfile, ModSummary, Page};
use gmm_core::{
    CacheStorage, Conflict, DiskState, FetchEvent, GameStorage, ImageCache, InstallEvent,
    InstallReport, UnmanagedMod, VariantChoice,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The custom protocol that serves cached images (registered in `main.rs`).
pub const IMAGE_SCHEME: &str = "gmmimg";

/// How the webview addresses a custom protocol differs by platform: WebView2 on Windows
/// only allows `http(s)://<scheme>.localhost/`.
fn image_src(key: &str) -> String {
    if cfg!(windows) {
        format!("http://{IMAGE_SCHEME}.localhost/{key}")
    } else {
        format!("{IMAGE_SCHEME}://localhost/{key}")
    }
}

#[derive(Debug, Serialize, TS)]
pub struct GameInfo {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub importer: String,
    pub gamebanana_id: u64,
}

/// A page of catalogue cards. `hidden` counts mods left out as sensitive (NSFW) because the
/// user hasn't opted in to see them.
#[derive(Debug, Serialize, TS)]
pub struct CatalogPage {
    pub page: Page<ModSummary>,
    pub hidden: u32,
}

#[derive(Debug, Deserialize, TS)]
pub struct CatalogQuery {
    pub game: String,
    /// 1-based.
    pub page: u32,
    pub per_page: u32,
    pub sort: gmm_catalog::Sort,
    /// Root category or subcategory (character).
    pub category: Option<u64>,
    pub show_nsfw: bool,
}

/// A mod GMM installed, as the library lists it.
#[derive(Debug, Serialize, TS)]
pub struct ModRow {
    pub id: i64,
    pub name: String,
    /// The folder's name on disk right now (with `DISABLED ` when off).
    pub folder: String,
    pub enabled: bool,
    pub disk: DiskState,
    pub variant: Option<String>,
    pub gb_mod_id: Option<u64>,
    pub version: Option<String>,
    pub size_bytes: u64,
    pub installed_at: i64,
}

#[derive(Debug, Serialize, TS)]
pub struct LibraryView {
    pub game: String,
    /// Why the Mods folder couldn't be read (XXMI or the importer missing), if it couldn't.
    pub problem: Option<String>,
    pub managed: Vec<ModRow>,
    pub unmanaged: Vec<UnmanagedMod>,
}

/// Names one mod in a game's library: GMM's by id, any other by the exact `path` the
/// library listed (relative to `Mods/`; checked against a fresh listing, never used as a
/// path as given).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModKey {
    Managed { id: i64 },
    Unmanaged { path: String },
}

/// Which variants of an archive to install.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VariantPick {
    /// The archive's only mod (fails with `choose_variant` if it holds several).
    #[default]
    Only,
    /// By name, as `choose_variant` listed them. Each becomes its own mod folder.
    Named {
        names: Vec<String>,
    },
    All,
}

impl From<VariantPick> for VariantChoice {
    fn from(pick: VariantPick) -> Self {
        match pick {
            VariantPick::Only => Self::Only,
            VariantPick::Named { names } => Self::Named(names),
            VariantPick::All => Self::All,
        }
    }
}

#[derive(Debug, Deserialize, TS)]
pub struct InstallArgs {
    pub mod_id: u64,
    /// Defaults to the newest file GameBanana scanned as clean.
    pub file_id: Option<u64>,
    #[serde(default)]
    pub variants: VariantPick,
    /// Install without the blocked files (after the user saw them listed).
    #[serde(default)]
    pub skip_blocked: bool,
}

/// Install progress, sent on the command's channel.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallProgress {
    UsingCache,
    Downloading { size: u64 },
    Progress { downloaded: u64, total: Option<u64> },
    Extracting,
    Installing { folder: String },
}

impl From<InstallEvent> for InstallProgress {
    fn from(event: InstallEvent) -> Self {
        match event {
            InstallEvent::Fetch(FetchEvent::UsingCache) => Self::UsingCache,
            InstallEvent::Fetch(FetchEvent::Downloading { size }) => Self::Downloading { size },
            InstallEvent::Fetch(FetchEvent::Progress(p)) => Self::Progress {
                downloaded: p.downloaded,
                total: p.total,
            },
            InstallEvent::Extracting => Self::Extracting,
            InstallEvent::Installing { folder } => Self::Installing { folder },
        }
    }
}

#[derive(Debug, Serialize, TS)]
pub struct InstallResult {
    pub report: InstallReport,
    /// Setup problems that don't stop installing but stop mods from loading.
    pub setup_warnings: Vec<String>,
    /// Mods already on that the new ones clash with (both stay on).
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Serialize, TS)]
pub struct Toggled {
    pub key: ModKey,
    pub enabled: bool,
    /// False when the mod already was in that state.
    pub changed: bool,
}

/// One mod as the uninstall confirmation lists it (security rule R8).
#[derive(Debug, Serialize, TS)]
pub struct UninstallItem {
    pub key: ModKey,
    pub name: String,
    /// Full path, for the user to check.
    pub folder: PathBuf,
    /// Counted for hand-installed mods only.
    pub files: Option<u64>,
    pub bytes: u64,
    /// The size is at least this (the walk stopped early).
    pub truncated: bool,
    /// Not installed by GMM: the user's own files.
    pub hand_installed: bool,
}

#[derive(Debug, Serialize, TS)]
pub struct Uninstalled {
    pub key: ModKey,
    /// False when the folder was already gone and only GMM's record was removed.
    pub trashed: bool,
}

#[derive(Debug, Serialize, TS)]
pub struct GameStorageView {
    pub game: String,
    pub problem: Option<String>,
    pub storage: Option<GameStorage>,
}

#[derive(Debug, Serialize, TS)]
pub struct StorageView {
    pub games: Vec<GameStorageView>,
    pub cache: CacheStorage,
}

/// What "Launch with XXMI" would run. Shown to the user before the first launch.
#[derive(Debug, Serialize, TS)]
pub struct LaunchInfo {
    pub exe: PathBuf,
    pub args: Vec<String>,
    /// Whether the user already confirmed this exact exe.
    pub confirmed: bool,
}

#[derive(Debug, Serialize, TS)]
pub struct Launched {
    pub pid: Option<u32>,
}

#[derive(Debug, Serialize, TS)]
pub struct FolderRow {
    pub game: String,
    pub folder: PathBuf,
}

#[derive(Debug, Serialize, TS)]
pub struct Settings {
    pub database: PathBuf,
    /// The XXMI folder the user picked, or `None` when it's found automatically.
    pub xxmi_root: Option<PathBuf>,
    pub confirmed_xxmi_exe: Option<PathBuf>,
    pub manual_folders: Vec<FolderRow>,
    pub http_cache_entries: u64,
    pub http_cache_bytes: u64,
    pub download_cache_entries: u64,
    pub download_cache_bytes: u64,
}

fn rewrite_image(images: &ImageCache, image: Image) -> Option<Image> {
    let url = image_src(&images.register(&image.url)?);
    let variants = image
        .variants
        .into_iter()
        .filter_map(|v| {
            Some(ImageVariant {
                url: image_src(&images.register(&v.url)?),
                size: v.size,
            })
        })
        .collect();
    Some(Image { url, variants })
}

fn rewrite_member(images: &ImageCache, member: &mut Member) {
    member.avatar_url = member
        .avatar_url
        .take()
        .and_then(|u| images.register(&u))
        .map(|k| image_src(&k));
}

pub fn rewrite_summary(images: &ImageCache, summary: &mut ModSummary) {
    summary.images = std::mem::take(&mut summary.images)
        .into_iter()
        .filter_map(|i| rewrite_image(images, i))
        .collect();
    rewrite_member(images, &mut summary.submitter);
}

/// Images in the description HTML are left as they are: the CSP keeps the webview from
/// loading them, and the mod page sanitizes and rewrites that HTML when it's built.
pub fn rewrite_profile(images: &ImageCache, profile: &mut ModProfile) {
    rewrite_summary(images, &mut profile.summary);
}

pub fn rewrite_categories(images: &ImageCache, categories: &mut [Category]) {
    for c in categories {
        c.icon_url = c
            .icon_url
            .take()
            .and_then(|u| images.register(&u))
            .map(|k| image_src(&k));
    }
}
