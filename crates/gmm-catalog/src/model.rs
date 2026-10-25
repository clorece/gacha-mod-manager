// SPDX-License-Identifier: MIT OR Apache-2.0

//! GMM's catalogue model. Timestamps are Unix seconds. Field meanings are mapped from
//! GameBanana in `gamebanana.rs`; see also the field table in docs/07-frontend-design.md.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    /// The catalogue's default order.
    #[default]
    Default,
    Newest,
    /// Only mods that have received an update.
    RecentlyUpdated,
    MostDownloaded,
    MostLiked,
    MostViewed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    pub game_id: u64,
    /// 1-based.
    pub page: u32,
    /// Clamped to 1..=50.
    pub per_page: u32,
    pub sort: Sort,
    /// Root category or subcategory (e.g. a character).
    pub category_id: Option<u64>,
}

impl ListQuery {
    pub fn new(game_id: u64) -> Self {
        Self {
            game_id,
            page: 1,
            per_page: 20,
            sort: Sort::Default,
            category_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub game_id: u64,
    pub text: String,
    pub page: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Total matching records across all pages.
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub is_last: bool,
}

/// How GameBanana wants a mod shown before the user opts in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Show,
    /// Blur / warn first (often NSFW).
    Warn,
    Hide,
    Other(String),
}

impl Visibility {
    pub(crate) fn from_gb(s: &str) -> Self {
        match s {
            "" | "show" => Self::Show,
            "warn" => Self::Warn,
            "hide" => Self::Hide,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Whether the UI should hide or blur it until the user opts in.
    pub fn is_sensitive(&self) -> bool {
        !matches!(self, Self::Show)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Member {
    pub id: u64,
    pub name: String,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CategoryRef {
    /// Absent when GameBanana only gives a name.
    pub id: Option<u64>,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ImageVariant {
    pub url: String,
    pub size: ImageSize,
}

/// A preview image. The first image of a mod is its banner (catalogue card and mod page
/// hero); the rest form the slideshow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Image {
    /// Full-size original.
    pub url: String,
    /// Scaled copies, smallest first.
    pub variants: Vec<ImageVariant>,
}

impl Image {
    /// Smallest variant at least `min_width` wide, else the original.
    pub fn url_for_width(&self, min_width: u32) -> &str {
        self.variants
            .iter()
            .find(|v| v.size.width >= min_width)
            .map_or(self.url.as_str(), |v| v.url.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModStats {
    pub likes: u64,
    pub views: u64,
    pub posts: u64,
    /// Only on the mod page, not in listings.
    pub downloads: Option<u64>,
}

/// One card in the catalogue grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModSummary {
    pub id: u64,
    /// GameBanana game id (`_aGame`), when the response includes it (mod pages do;
    /// observed 2026-10-23).
    pub game_id: Option<u64>,
    pub name: String,
    pub submitter: Member,
    pub root_category: Option<CategoryRef>,
    /// For skins this is the character.
    pub category: Option<CategoryRef>,
    pub version: Option<String>,
    pub date_added: i64,
    pub date_modified: i64,
    pub date_updated: Option<i64>,
    pub images: Vec<Image>,
    pub stats: ModStats,
    pub featured: bool,
    pub obsolete: bool,
    pub visibility: Visibility,
    pub has_content_ratings: bool,
    pub profile_url: String,
}

impl ModSummary {
    pub fn banner(&self) -> Option<&Image> {
        self.images.first()
    }

    /// Flagged by GameBanana (visibility or content ratings): hidden until the user opts in.
    pub fn is_sensitive(&self) -> bool {
        self.visibility.is_sensitive() || self.has_content_ratings
    }
}

impl Page<ModSummary> {
    /// Drops sensitive mods (see [`ModSummary::is_sensitive`]) and returns how many.
    pub fn hide_sensitive(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|m| !m.is_sensitive());
        before - self.items.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModFile {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub date_added: i64,
    pub download_count: u64,
    pub download_url: String,
    /// Often empty on GameBanana.
    pub md5: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    /// GameBanana's preliminary analysis, e.g. "ok".
    pub analysis_result: Option<String>,
    /// GameBanana's antivirus result, e.g. "clean".
    pub av_result: Option<String>,
    pub archived: bool,
}

impl ModFile {
    /// Only files GameBanana scanned as clean may be installed (docs/04-security.md, T4).
    pub fn is_clean(&self) -> bool {
        self.av_result.as_deref() == Some("clean")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModFiles {
    pub files: Vec<ModFile>,
    pub archived_files: Vec<ModFile>,
}

impl ModFiles {
    /// Newest non-archived clean file: the default "Install" choice.
    pub fn default_choice(&self) -> Option<&ModFile> {
        self.files
            .iter()
            .filter(|f| f.is_clean())
            .max_by_key(|f| f.date_added)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LicenseChecklist {
    pub yes: Vec<String>,
    pub ask: Vec<String>,
    pub no: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Credit {
    pub name: String,
    pub role: Option<String>,
    pub member_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreditGroup {
    pub name: String,
    pub authors: Vec<Credit>,
}

/// Everything on a mod page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModProfile {
    pub summary: ModSummary,
    /// Untrusted HTML; sanitize before display.
    pub description_html: String,
    /// Untrusted HTML; sanitize before display.
    pub license_html: String,
    pub license: LicenseChecklist,
    pub credits: Vec<CreditGroup>,
    pub files: ModFiles,
    pub updates_count: u64,
    pub is_trashed: bool,
    pub is_withheld: bool,
}

impl ModProfile {
    /// Trashed, withheld or obsolete mods are shown but cannot be installed.
    pub fn is_installable(&self) -> bool {
        !self.is_trashed && !self.is_withheld && !self.summary.obsolete
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChangeEntry {
    pub text: String,
    /// e.g. "Addition", "Bugfix".
    pub category: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModUpdate {
    pub id: u64,
    pub title: String,
    pub version: Option<String>,
    pub date_added: i64,
    pub changelog: Vec<ChangeEntry>,
    /// Untrusted HTML; sanitize before display.
    pub text_html: String,
    pub file_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Category {
    pub id: u64,
    pub name: String,
    pub item_count: u64,
    pub subcategory_count: u64,
    pub icon_url: Option<String>,
    pub obsolete: bool,
}
