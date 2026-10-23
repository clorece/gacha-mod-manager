// SPDX-License-Identifier: MIT OR Apache-2.0

//! GameBanana `apiv11` client. Endpoints and fields were confirmed against the live API on
//! 2026-10-22 (docs/02-architecture.md, "Catalog flow").
//!
//! GameBanana's JSON is loosely typed: numbers sometimes arrive as strings, and an empty
//! object is often serialized as `[]`. The raw types below therefore deserialize every field
//! leniently and fall back to defaults instead of failing the whole response.

use std::sync::Arc;
use std::time::Duration;

use gmm_net::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use url::Url;

use crate::CatalogSource;
use crate::cache::HttpCache;
use crate::error::CatalogError;
use crate::model::*;

const BASE_URL: &str = "https://gamebanana.com/apiv11/";

const TTL_LIST: Duration = Duration::from_secs(5 * 60);
const TTL_DETAIL: Duration = Duration::from_secs(60 * 60);
const TTL_FILES: Duration = Duration::from_secs(2 * 60);
const TTL_CATEGORIES: Duration = Duration::from_secs(24 * 60 * 60);

pub struct GameBanana {
    net: Client,
    base: Url,
    cache: Arc<dyn HttpCache>,
}

impl GameBanana {
    pub fn new(net: Client, cache: Arc<dyn HttpCache>) -> Self {
        Self::with_base_url(
            net,
            cache,
            Url::parse(BASE_URL).unwrap_or_else(|_| unreachable!()),
        )
    }

    /// For tests against a mock server. The network client's allowlist still applies, so
    /// this cannot point GMM anywhere a release build couldn't already reach.
    pub fn with_base_url(net: Client, cache: Arc<dyn HttpCache>, base: Url) -> Self {
        Self { net, base, cache }
    }

    fn endpoint(&self, path: &str, query: &[(&str, String)]) -> Result<Url, CatalogError> {
        let mut url = self.base.join(path).map_err(|e| CatalogError::Decode {
            what: path.to_owned(),
            message: e.to_string(),
        })?;
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        Ok(url)
    }

    async fn fetch<T: DeserializeOwned>(
        &self,
        url: Url,
        ttl: Duration,
        what: &str,
    ) -> Result<T, CatalogError> {
        let decode = |body: &[u8]| {
            serde_json::from_slice::<T>(body).map_err(|e| CatalogError::Decode {
                what: what.to_owned(),
                message: e.to_string(),
            })
        };
        if let Some(body) = self.cache.get(url.as_str(), ttl)
            && let Ok(value) = decode(&body)
        {
            return Ok(value);
        }
        let resp = self.net.get(&url).await?;
        match resp.status {
            200..=299 => {
                let value = decode(&resp.body)?;
                self.cache.put(url.as_str(), resp.body);
                Ok(value)
            }
            404 => Err(CatalogError::NotFound),
            400 => Err(CatalogError::BadRequest(error_message(&resp.body))),
            status => Err(CatalogError::Http(status)),
        }
    }
}

impl CatalogSource for GameBanana {
    async fn list_mods(&self, q: &ListQuery) -> Result<Page<ModSummary>, CatalogError> {
        let mut query = vec![
            ("_nPage", q.page.max(1).to_string()),
            ("_nPerpage", q.per_page.clamp(1, 50).to_string()),
            ("_aFilters[Generic_Game]", q.game_id.to_string()),
        ];
        if let Some(category) = q.category_id {
            query.push(("_aFilters[Generic_Category]", category.to_string()));
        }
        if let Some(sort) = sort_alias(q.sort) {
            query.push(("_sSort", sort.to_owned()));
        }
        let url = self.endpoint("Mod/Index", &query)?;
        let raw: RawList<RawRecord> = self.fetch(url, TTL_LIST, "mod list").await?;
        Ok(raw.into_page(q.page.max(1), RawRecord::into_summary))
    }

    async fn search(&self, q: &SearchQuery) -> Result<Page<ModSummary>, CatalogError> {
        let url = self.endpoint(
            "Util/Search/Results",
            &[
                ("_sModelName", "Mod".to_owned()),
                ("_sOrder", "best_match".to_owned()),
                ("_idGameRow", q.game_id.to_string()),
                ("_sSearchString", q.text.clone()),
                ("_nPage", q.page.max(1).to_string()),
            ],
        )?;
        let mut raw: RawList<RawRecord> = self.fetch(url, TTL_LIST, "search").await?;
        raw.records
            .retain(|r| r.model_name.is_empty() || r.model_name == "Mod");
        Ok(raw.into_page(q.page.max(1), RawRecord::into_summary))
    }

    async fn mod_profile(&self, mod_id: u64) -> Result<ModProfile, CatalogError> {
        let url = self.endpoint(&format!("Mod/{mod_id}/ProfilePage"), &[])?;
        let raw: RawProfile = self.fetch(url, TTL_DETAIL, "mod page").await?;
        Ok(raw.into_profile())
    }

    async fn mod_files(&self, mod_id: u64) -> Result<ModFiles, CatalogError> {
        let url = self.endpoint(&format!("Mod/{mod_id}/DownloadPage"), &[])?;
        let raw: RawDownloadPage = self.fetch(url, TTL_FILES, "mod files").await?;
        Ok(map_files(raw.files, raw.archived_files))
    }

    async fn mod_updates(&self, mod_id: u64, page: u32) -> Result<Page<ModUpdate>, CatalogError> {
        let url = self.endpoint(
            &format!("Mod/{mod_id}/Updates"),
            &[
                ("_nPage", page.max(1).to_string()),
                ("_nPerpage", "10".to_owned()),
            ],
        )?;
        let raw: RawList<RawUpdate> = self.fetch(url, TTL_DETAIL, "mod updates").await?;
        Ok(raw.into_page(page.max(1), RawUpdate::into_update))
    }

    async fn categories(&self, game_id: u64) -> Result<Vec<Category>, CatalogError> {
        let url = self.endpoint(
            "Mod/Categories",
            &[
                ("_idGameRow", game_id.to_string()),
                ("_sSort", "a_to_z".to_owned()),
                ("_bShowEmpty", "true".to_owned()),
            ],
        )?;
        let raw: Vec<RawCategory> = self.fetch(url, TTL_CATEGORIES, "categories").await?;
        Ok(raw.into_iter().map(RawCategory::into_category).collect())
    }

    async fn subcategories(&self, category_id: u64) -> Result<Vec<Category>, CatalogError> {
        let url = self.endpoint(
            "Mod/Categories",
            &[
                ("_idCategoryRow", category_id.to_string()),
                ("_sSort", "a_to_z".to_owned()),
                ("_bShowEmpty", "true".to_owned()),
            ],
        )?;
        let raw: Vec<RawCategory> = self.fetch(url, TTL_CATEGORIES, "subcategories").await?;
        Ok(raw.into_iter().map(RawCategory::into_category).collect())
    }
}

fn sort_alias(sort: Sort) -> Option<&'static str> {
    match sort {
        Sort::Default => None,
        Sort::Newest => Some("Generic_Newest"),
        Sort::RecentlyUpdated => Some("Generic_LatestUpdated"),
        Sort::MostDownloaded => Some("Generic_MostDownloaded"),
        Sort::MostLiked => Some("Generic_MostLiked"),
        Sort::MostViewed => Some("Generic_MostViewed"),
    }
}

/// Best human-readable message from a GameBanana error body, e.g.
/// `{"_sErrorCode":"INPUT_ERRORS","_aErrorData":{"_sSort":{"_sErrorMessage":"..."}}}`.
fn error_message(body: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return "invalid request".to_owned();
    };
    let mut messages = Vec::new();
    if let Some(m) = value.get("_sErrorMessage").and_then(Value::as_str) {
        messages.push(m.to_owned());
    }
    if let Some(Value::Object(fields)) = value.get("_aErrorData") {
        for (field, detail) in fields {
            if let Some(m) = detail.get("_sErrorMessage").and_then(Value::as_str) {
                messages.push(format!("{field}: {m}"));
            }
        }
    }
    if messages.is_empty() {
        value
            .get("_sErrorCode")
            .and_then(Value::as_str)
            .unwrap_or("invalid request")
            .to_owned()
    } else {
        messages.join("; ")
    }
}

// ---- lenient field decoding ------------------------------------------------------------

fn de_u64<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    Ok(as_u64(&Value::deserialize(d)?).unwrap_or(0))
}

fn de_opt_u64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    Ok(as_u64(&Value::deserialize(d)?))
}

fn de_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(as_i64(&Value::deserialize(d)?).unwrap_or(0))
}

fn de_opt_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(as_i64(&Value::deserialize(d)?))
}

fn de_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    })
}

fn de_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_u64().is_some_and(|n| n != 0),
        Value::String(s) => s == "true" || s == "1",
        _ => false,
    })
}

/// Objects that GameBanana sends as `[]` when empty, or that have an unexpected shape.
fn de_lenient<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    d: D,
) -> Result<T, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).unwrap_or_default())
}

fn as_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn non_empty(s: String) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// `https://gamebanana.com/mods/cats/43004` -> 43004
fn id_from_url(url: &str) -> Option<u64> {
    url.trim_end_matches('/').rsplit('/').next()?.parse().ok()
}

// ---- raw GameBanana shapes ----------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawMeta {
    #[serde(rename = "_nRecordCount", deserialize_with = "de_u64")]
    record_count: u64,
    #[serde(rename = "_nPerpage", deserialize_with = "de_u64")]
    per_page: u64,
    #[serde(rename = "_bIsComplete", deserialize_with = "de_bool")]
    is_complete: bool,
}

#[derive(Debug, Deserialize)]
struct RawList<T> {
    #[serde(rename = "_aMetadata", default, deserialize_with = "de_lenient")]
    meta: RawMeta,
    #[serde(rename = "_aRecords", default = "Vec::new")]
    records: Vec<T>,
}

impl<T> RawList<T> {
    fn into_page<U>(self, page: u32, map: impl Fn(T) -> U) -> Page<U> {
        Page {
            items: self.records.into_iter().map(map).collect(),
            total: self.meta.record_count,
            page,
            per_page: u32::try_from(self.meta.per_page).unwrap_or(u32::MAX),
            is_last: self.meta.is_complete,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawImage {
    #[serde(rename = "_sBaseUrl", deserialize_with = "de_string")]
    base_url: String,
    #[serde(rename = "_sFile", deserialize_with = "de_string")]
    file: String,
    #[serde(rename = "_sFile100", deserialize_with = "de_string")]
    file_100: String,
    #[serde(rename = "_wFile100", deserialize_with = "de_u64")]
    w_100: u64,
    #[serde(rename = "_hFile100", deserialize_with = "de_u64")]
    h_100: u64,
    #[serde(rename = "_sFile220", deserialize_with = "de_string")]
    file_220: String,
    #[serde(rename = "_wFile220", deserialize_with = "de_u64")]
    w_220: u64,
    #[serde(rename = "_hFile220", deserialize_with = "de_u64")]
    h_220: u64,
    #[serde(rename = "_sFile530", deserialize_with = "de_string")]
    file_530: String,
    #[serde(rename = "_wFile530", deserialize_with = "de_u64")]
    w_530: u64,
    #[serde(rename = "_hFile530", deserialize_with = "de_u64")]
    h_530: u64,
    #[serde(rename = "_sFile800", deserialize_with = "de_string")]
    file_800: String,
    #[serde(rename = "_wFile800", deserialize_with = "de_u64")]
    w_800: u64,
    #[serde(rename = "_hFile800", deserialize_with = "de_u64")]
    h_800: u64,
}

impl RawImage {
    fn into_image(self) -> Option<Image> {
        let base = self.base_url.trim_end_matches('/');
        if base.is_empty() || self.file.is_empty() {
            return None;
        }
        let to_u32 = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        let variants = [
            (self.file_100, self.w_100, self.h_100),
            (self.file_220, self.w_220, self.h_220),
            (self.file_530, self.w_530, self.h_530),
            (self.file_800, self.w_800, self.h_800),
        ]
        .into_iter()
        .filter(|(file, w, _)| !file.is_empty() && *w > 0)
        .map(|(file, w, h)| ImageVariant {
            url: format!("{base}/{file}"),
            size: ImageSize {
                width: to_u32(w),
                height: to_u32(h),
            },
        })
        .collect();
        Some(Image {
            url: format!("{base}/{}", self.file),
            variants,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawPreview {
    #[serde(rename = "_aImages")]
    images: Vec<RawImage>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawMember {
    #[serde(rename = "_idRow", deserialize_with = "de_u64")]
    id: u64,
    #[serde(rename = "_sName", deserialize_with = "de_string")]
    name: String,
    #[serde(rename = "_sAvatarUrl", deserialize_with = "de_string")]
    avatar_url: String,
    #[serde(rename = "_sProfileUrl", deserialize_with = "de_string")]
    profile_url: String,
}

impl RawMember {
    fn into_member(self) -> Member {
        Member {
            id: self.id,
            name: self.name,
            avatar_url: non_empty(self.avatar_url),
            profile_url: non_empty(self.profile_url),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawCategoryRef {
    #[serde(rename = "_idRow", deserialize_with = "de_opt_u64")]
    id: Option<u64>,
    #[serde(rename = "_sName", deserialize_with = "de_string")]
    name: String,
    #[serde(rename = "_sProfileUrl", deserialize_with = "de_string")]
    profile_url: String,
}

impl RawCategoryRef {
    fn into_ref(self) -> Option<CategoryRef> {
        let name = non_empty(self.name)?;
        Some(CategoryRef {
            id: self.id.or_else(|| id_from_url(&self.profile_url)),
            name,
        })
    }
}

/// A mod as it appears in listings and search results.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawRecord {
    #[serde(rename = "_idRow", deserialize_with = "de_u64")]
    id: u64,
    #[serde(rename = "_sModelName", deserialize_with = "de_string")]
    model_name: String,
    #[serde(rename = "_sName", deserialize_with = "de_string")]
    name: String,
    #[serde(rename = "_sProfileUrl", deserialize_with = "de_string")]
    profile_url: String,
    #[serde(rename = "_tsDateAdded", deserialize_with = "de_i64")]
    date_added: i64,
    #[serde(rename = "_tsDateModified", deserialize_with = "de_i64")]
    date_modified: i64,
    #[serde(rename = "_tsDateUpdated", deserialize_with = "de_opt_i64")]
    date_updated: Option<i64>,
    #[serde(rename = "_aPreviewMedia", deserialize_with = "de_lenient")]
    preview: RawPreview,
    #[serde(rename = "_aSubmitter", deserialize_with = "de_lenient")]
    submitter: RawMember,
    #[serde(rename = "_aRootCategory", deserialize_with = "de_lenient")]
    root_category: RawCategoryRef,
    #[serde(rename = "_aSubCategory", deserialize_with = "de_lenient")]
    sub_category: RawCategoryRef,
    #[serde(rename = "_sVersion", deserialize_with = "de_string")]
    version: String,
    #[serde(rename = "_bIsObsolete", deserialize_with = "de_bool")]
    obsolete: bool,
    #[serde(rename = "_sInitialVisibility", deserialize_with = "de_string")]
    visibility: String,
    #[serde(rename = "_bHasContentRatings", deserialize_with = "de_bool")]
    has_content_ratings: bool,
    #[serde(rename = "_nLikeCount", deserialize_with = "de_u64")]
    likes: u64,
    #[serde(rename = "_nPostCount", deserialize_with = "de_u64")]
    posts: u64,
    #[serde(rename = "_nViewCount", deserialize_with = "de_u64")]
    views: u64,
    #[serde(rename = "_bWasFeatured", deserialize_with = "de_bool")]
    featured: bool,
}

impl RawRecord {
    fn into_summary(self) -> ModSummary {
        ModSummary {
            id: self.id,
            name: self.name,
            submitter: self.submitter.into_member(),
            root_category: self.root_category.into_ref(),
            category: self.sub_category.into_ref(),
            version: non_empty(self.version),
            date_added: self.date_added,
            date_modified: self.date_modified,
            date_updated: self.date_updated.filter(|t| *t > 0),
            images: self
                .preview
                .images
                .into_iter()
                .filter_map(RawImage::into_image)
                .collect(),
            stats: ModStats {
                likes: self.likes,
                views: self.views,
                posts: self.posts,
                downloads: None,
            },
            featured: self.featured,
            obsolete: self.obsolete,
            visibility: Visibility::from_gb(&self.visibility),
            has_content_ratings: self.has_content_ratings,
            profile_url: self.profile_url,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawLicenseChecklist {
    #[serde(deserialize_with = "de_lenient")]
    yes: Vec<String>,
    #[serde(deserialize_with = "de_lenient")]
    ask: Vec<String>,
    #[serde(deserialize_with = "de_lenient")]
    no: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawAuthor {
    #[serde(rename = "_sName", deserialize_with = "de_string")]
    name: String,
    #[serde(rename = "_sRole", deserialize_with = "de_string")]
    role: String,
    #[serde(rename = "_idRow", deserialize_with = "de_opt_u64")]
    id: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawCreditGroup {
    #[serde(rename = "_sGroupName", deserialize_with = "de_string")]
    name: String,
    #[serde(rename = "_aAuthors", deserialize_with = "de_lenient")]
    authors: Vec<RawAuthor>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawFile {
    #[serde(rename = "_idRow", deserialize_with = "de_u64")]
    id: u64,
    #[serde(rename = "_sFile", deserialize_with = "de_string")]
    name: String,
    #[serde(rename = "_nFilesize", deserialize_with = "de_u64")]
    size: u64,
    #[serde(rename = "_tsDateAdded", deserialize_with = "de_i64")]
    date_added: i64,
    #[serde(rename = "_nDownloadCount", deserialize_with = "de_u64")]
    download_count: u64,
    #[serde(rename = "_sDownloadUrl", deserialize_with = "de_string")]
    download_url: String,
    #[serde(rename = "_sMd5Checksum", deserialize_with = "de_string")]
    md5: String,
    #[serde(rename = "_sVersion", deserialize_with = "de_string")]
    version: String,
    #[serde(rename = "_sDescription", deserialize_with = "de_string")]
    description: String,
    #[serde(rename = "_sAnalysisResult", deserialize_with = "de_string")]
    analysis_result: String,
    #[serde(rename = "_sAvResult", deserialize_with = "de_string")]
    av_result: String,
}

impl RawFile {
    fn into_file(self, archived: bool) -> ModFile {
        ModFile {
            id: self.id,
            name: self.name,
            size: self.size,
            date_added: self.date_added,
            download_count: self.download_count,
            download_url: self.download_url,
            md5: non_empty(self.md5),
            version: non_empty(self.version),
            description: non_empty(self.description),
            analysis_result: non_empty(self.analysis_result),
            av_result: non_empty(self.av_result),
            archived,
        }
    }
}

fn map_files(files: Vec<RawFile>, archived: Vec<RawFile>) -> ModFiles {
    ModFiles {
        files: files.into_iter().map(|f| f.into_file(false)).collect(),
        archived_files: archived.into_iter().map(|f| f.into_file(true)).collect(),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawDownloadPage {
    #[serde(rename = "_aFiles", deserialize_with = "de_lenient")]
    files: Vec<RawFile>,
    #[serde(rename = "_aArchivedFiles", deserialize_with = "de_lenient")]
    archived_files: Vec<RawFile>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawProfile {
    #[serde(flatten)]
    record: RawRecord,
    #[serde(rename = "_aCategory", deserialize_with = "de_lenient")]
    category: RawCategoryRef,
    #[serde(rename = "_aSuperCategory", deserialize_with = "de_lenient")]
    super_category: RawCategoryRef,
    #[serde(rename = "_sText", deserialize_with = "de_string")]
    text: String,
    #[serde(rename = "_sLicense", deserialize_with = "de_string")]
    license: String,
    #[serde(rename = "_aLicenseChecklist", deserialize_with = "de_lenient")]
    license_checklist: RawLicenseChecklist,
    #[serde(rename = "_aCredits", deserialize_with = "de_lenient")]
    credits: Vec<RawCreditGroup>,
    #[serde(rename = "_aFiles", deserialize_with = "de_lenient")]
    files: Vec<RawFile>,
    #[serde(rename = "_aArchivedFiles", deserialize_with = "de_lenient")]
    archived_files: Vec<RawFile>,
    #[serde(rename = "_nDownloadCount", deserialize_with = "de_u64")]
    downloads: u64,
    #[serde(rename = "_nUpdatesCount", deserialize_with = "de_u64")]
    updates_count: u64,
    #[serde(rename = "_bIsTrashed", deserialize_with = "de_bool")]
    is_trashed: bool,
    #[serde(rename = "_bIsWithheld", deserialize_with = "de_bool")]
    is_withheld: bool,
}

impl RawProfile {
    fn into_profile(self) -> ModProfile {
        let mut summary = self.record.into_summary();
        // The profile page names categories differently from listings.
        if let Some(category) = self.category.into_ref() {
            summary.category = Some(category);
        }
        if let Some(root) = self.super_category.into_ref() {
            summary.root_category = Some(root);
        }
        summary.stats.downloads = Some(self.downloads);
        ModProfile {
            summary,
            description_html: self.text,
            license_html: self.license,
            license: LicenseChecklist {
                yes: self.license_checklist.yes,
                ask: self.license_checklist.ask,
                no: self.license_checklist.no,
            },
            credits: self
                .credits
                .into_iter()
                .map(|g| CreditGroup {
                    name: g.name,
                    authors: g
                        .authors
                        .into_iter()
                        .map(|a| Credit {
                            name: a.name,
                            role: non_empty(a.role),
                            member_id: a.id,
                        })
                        .collect(),
                })
                .collect(),
            files: map_files(self.files, self.archived_files),
            updates_count: self.updates_count,
            is_trashed: self.is_trashed,
            is_withheld: self.is_withheld,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawChange {
    #[serde(deserialize_with = "de_string")]
    text: String,
    #[serde(deserialize_with = "de_string")]
    cat: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawUpdate {
    #[serde(rename = "_idRow", deserialize_with = "de_u64")]
    id: u64,
    #[serde(rename = "_sName", deserialize_with = "de_string")]
    name: String,
    #[serde(rename = "_sVersion", deserialize_with = "de_string")]
    version: String,
    #[serde(rename = "_tsDateAdded", deserialize_with = "de_i64")]
    date_added: i64,
    #[serde(rename = "_aChangeLog", deserialize_with = "de_lenient")]
    changelog: Vec<RawChange>,
    #[serde(rename = "_sText", deserialize_with = "de_string")]
    text: String,
    #[serde(rename = "_aFileRowIds", deserialize_with = "de_lenient")]
    file_ids: Vec<u64>,
}

impl RawUpdate {
    fn into_update(self) -> ModUpdate {
        ModUpdate {
            id: self.id,
            title: self.name,
            version: non_empty(self.version),
            date_added: self.date_added,
            changelog: self
                .changelog
                .into_iter()
                .map(|c| ChangeEntry {
                    text: c.text,
                    category: c.cat,
                })
                .collect(),
            text_html: self.text,
            file_ids: self.file_ids,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawCategory {
    #[serde(rename = "_idRow", deserialize_with = "de_u64")]
    id: u64,
    #[serde(rename = "_sName", deserialize_with = "de_string")]
    name: String,
    #[serde(rename = "_nItemCount", deserialize_with = "de_u64")]
    item_count: u64,
    #[serde(rename = "_nCategoryCount", deserialize_with = "de_u64")]
    category_count: u64,
    #[serde(rename = "_sIconUrl", deserialize_with = "de_string")]
    icon_url: String,
    #[serde(rename = "_bIsObsolete", deserialize_with = "de_bool")]
    obsolete: bool,
}

impl RawCategory {
    fn into_category(self) -> Category {
        Category {
            id: self.id,
            name: self.name,
            item_count: self.item_count,
            subcategory_count: self.category_count,
            icon_url: non_empty(self.icon_url),
            obsolete: self.obsolete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenient_decoding_survives_gamebanana_quirks() {
        // Numbers as strings, `[]` instead of `{}` for empty objects, missing fields.
        let raw: RawRecord = serde_json::from_str(
            r#"{"_idRow":"42","_sName":"X","_aPreviewMedia":[],"_aSubmitter":[],
                "_nLikeCount":"7","_bIsObsolete":0,"_sVersion":1.5}"#,
        )
        .unwrap();
        let summary = raw.into_summary();
        assert_eq!(summary.id, 42);
        assert_eq!(summary.stats.likes, 7);
        assert!(summary.images.is_empty());
        assert_eq!(summary.version.as_deref(), Some("1.5"));
        assert_eq!(summary.visibility, Visibility::Show);
    }

    #[test]
    fn error_messages_are_extracted() {
        let body = br#"{"_sErrorCode":"INPUT_ERRORS","_aErrorData":{"_sSort":{"_sErrorCode":"UNKNOWN_SORT","_sErrorMessage":"Sort alias not recognized"}}}"#;
        assert_eq!(error_message(body), "_sSort: Sort alias not recognized");
        assert_eq!(error_message(b"not json"), "invalid request");
    }

    #[test]
    fn category_ids_from_urls() {
        assert_eq!(
            id_from_url("https://gamebanana.com/mods/cats/43004"),
            Some(43004)
        );
        assert_eq!(
            id_from_url("https://gamebanana.com/mods/cats/43004/"),
            Some(43004)
        );
        assert_eq!(id_from_url("https://gamebanana.com/mods/cats/"), None);
    }
}
