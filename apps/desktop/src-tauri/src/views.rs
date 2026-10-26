// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the commands return, where `gmm-core`'s own types don't fit the UI as they are, and
//! the image rewrite: every GameBanana image URL in catalogue data is replaced with a
//! `gmmimg` URL for an opaque key (see `gmm_core::ImageCache`), so the webview never sees,
//! or requests, an outside address.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gmm_catalog::{
    Category, Image, ImageVariant, Member, ModProfile, ModStats, ModSummary, ModUpdate, Page,
};
use gmm_core::{
    AppRelease, CacheStorage, Conflict, DiskState, FetchEvent, GameStorage, ImageCache, InstallEvent,
    InstallReport, ModUpdateInfo, UnmanagedMod, VariantChoice,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::html::{self, Links};

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
    /// When GMM's Launch button last started it (Unix seconds).
    pub last_played: Option<i64>,
    /// The Store's Category filter, from the game's manifest (GameBanana's order).
    pub categories: Vec<GameCategory>,
}

/// One of a game's root categories on GameBanana.
#[derive(Debug, Serialize, TS)]
pub struct GameCategory {
    pub id: u64,
    pub name: String,
    /// Its subcategories are characters (the Store's Characters filter).
    pub characters: bool,
}

/// A page of catalogue cards. `hidden` counts mods left out as sensitive (NSFW) because the
/// user hasn't opted in to see them.
#[derive(Debug, Serialize, TS)]
pub struct CatalogPage {
    pub page: Page<ModSummary>,
    pub hidden: u32,
}

/// A Store page. `game` is a game id or `"all"`; `categories` are one root category or
/// several subcategories (characters) of one game, merged (`gmm_core::browse`); empty means
/// the whole catalogue. `text` filters by name.
#[derive(Debug, Deserialize, TS)]
pub struct CatalogQuery {
    pub game: String,
    /// 1-based.
    pub page: u32,
    pub per_page: u32,
    pub sort: gmm_catalog::Sort,
    pub categories: Vec<u64>,
    pub text: Option<String>,
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
    pub gb_file_id: Option<u64>,
    /// Folders with the same group are versions of one mod (same GameBanana mod and file).
    pub group: Option<String>,
    pub version: Option<String>,
    pub size_bytes: u64,
    pub installed_at: i64,
    /// GameBanana's root category at install, e.g. "Character Skins".
    pub root_category: Option<String>,
    pub root_category_id: Option<u64>,
    /// The subcategory at install: for skins, the character.
    pub category: Option<String>,
    pub category_id: Option<u64>,
    /// GameBanana flagged it at install (false when unknown: earlier installs).
    pub sensitive: bool,
    /// The stored preview image as a `gmmimg` address, when there is one.
    pub preview: Option<String>,
}

/// An earlier file of an installed mod, kept in the download cache.
#[derive(Debug, Serialize, TS)]
pub struct RollbackFile {
    pub file_id: u64,
    pub name: String,
    /// Upload date (Unix seconds), when GameBanana still lists the file.
    pub date_added: Option<i64>,
    pub version: Option<String>,
}

/// See `commands::network_status`.
#[derive(Debug, Serialize, TS)]
pub struct NetworkStatus {
    /// Seconds until GMM asks GameBanana again, while it holds off after a "slow down".
    pub cooldown_secs: Option<u64>,
    /// Saved answers stood in for fresh ones in the last minute and a half.
    pub showing_saved: bool,
}

/// A card's counters, sent as each mod's numbers arrive (`catalog_stats`).
#[derive(Debug, Clone, Serialize, TS)]
pub struct CardStats {
    pub mod_id: u64,
    pub stats: ModStats,
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
    /// Install switched off ("Switch it on after installing" unticked).
    #[serde(default)]
    pub start_disabled: bool,
    /// Mods of the same game (library row ids) to switch off once this one is installed and
    /// on: the other side of a conflict resolved in the confirmation. The queue only.
    #[serde(default)]
    pub switch_off: Vec<i64>,
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

/// Lets every step event through, and download progress at most every 200 ms: the download
/// reports each chunk (thousands per file), far more than a progress bar needs over IPC.
#[derive(Default)]
pub struct ProgressThrottle {
    last: Option<Instant>,
}

impl ProgressThrottle {
    const EVERY: Duration = Duration::from_millis(200);

    /// Whether `event` should be sent now.
    pub fn pass(&mut self, event: &InstallEvent) -> bool {
        let progress = matches!(event, InstallEvent::Fetch(FetchEvent::Progress(_)));
        if progress && self.last.is_some_and(|t| t.elapsed() < Self::EVERY) {
            return false;
        }
        self.last = Some(Instant::now());
        true
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

/// Where a startup-check step is.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Working,
    Done,
    Failed,
}

/// One step of the startup check, sent as it starts and as it ends.
#[derive(Debug, Clone, Serialize, TS)]
pub struct StartupStep {
    /// 0 is the GMM app, then one per game, then the installed mods.
    pub step: u32,
    pub label: String,
    pub state: StepState,
    pub detail: String,
}

#[derive(Debug, Default, Serialize, TS)]
pub struct StartupReport {
    /// GameBanana mods compared with their pages.
    pub checked: usize,
    pub updates: Vec<ModUpdateInfo>,
    /// Mods or games that couldn't be checked.
    pub notes: Vec<String>,
    /// A newer GMM on GitHub, when the check found one.
    pub app_update: Option<AppRelease>,
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
    pub removed: bool,
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

/// The look of the app (`gmm_db::Theme`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    Dark,
    Light,
    System,
}

/// The user's preferences: the first-run screen, Settings, and the sensitive-mods switch in
/// the Store and Library filters all read and write these.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
pub struct Preferences {
    pub theme: ThemeChoice,
    /// Show mods GameBanana marks as sensitive (NSFW). Off by default.
    pub show_sensitive: bool,
    /// Blur sensitive previews until hovered, while they are shown. On by default.
    pub blur_sensitive: bool,
    /// The first-run preferences screen was answered.
    pub onboarded: bool,
}

impl From<gmm_db::Preferences> for Preferences {
    fn from(p: gmm_db::Preferences) -> Self {
        Self {
            theme: match p.theme {
                gmm_db::Theme::Dark => ThemeChoice::Dark,
                gmm_db::Theme::Light => ThemeChoice::Light,
                gmm_db::Theme::System => ThemeChoice::System,
            },
            show_sensitive: p.show_sensitive,
            blur_sensitive: p.blur_sensitive,
            onboarded: p.onboarded,
        }
    }
}

impl From<Preferences> for gmm_db::Preferences {
    fn from(p: Preferences) -> Self {
        Self {
            theme: match p.theme {
                ThemeChoice::Dark => gmm_db::Theme::Dark,
                ThemeChoice::Light => gmm_db::Theme::Light,
                ThemeChoice::System => gmm_db::Theme::System,
            },
            show_sensitive: p.show_sensitive,
            blur_sensitive: p.blur_sensitive,
            onboarded: p.onboarded,
        }
    }
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

/// The `gmmimg` address for a stored GameBanana image URL (`None` off GameBanana's hosts).
pub fn image_for(images: &ImageCache, url: &str) -> Option<String> {
    images.register(url).map(|k| image_src(&k))
}

pub fn rewrite_summary(images: &ImageCache, summary: &mut ModSummary) {
    summary.images = std::mem::take(&mut summary.images)
        .into_iter()
        .filter_map(|i| rewrite_image(images, i))
        .collect();
    rewrite_member(images, &mut summary.submitter);
}

/// GameBanana HTML with its addresses replaced (see `html.rs`). Still untrusted: the webview
/// sanitizes it with DOMPurify before showing it.
pub fn rewrite_html(images: &ImageCache, links: &Links, html: &str) -> String {
    html::rewrite(
        html,
        |url| images.register(url).map(|k| image_src(&k)),
        links,
    )
}

pub fn rewrite_profile(images: &ImageCache, links: &Links, profile: &mut ModProfile) {
    rewrite_summary(images, &mut profile.summary);
    profile.description_html = rewrite_html(images, links, &profile.description_html);
    profile.license_html = rewrite_html(images, links, &profile.license_html);
}

pub fn rewrite_updates(images: &ImageCache, links: &Links, updates: &mut Page<ModUpdate>) {
    for update in &mut updates.items {
        update.text_html = rewrite_html(images, links, &update.text_html);
    }
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
