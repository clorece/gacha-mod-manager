// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mod catalogue.
//!
//! [`CatalogSource`] is what the rest of GMM talks to; [`GameBanana`] implements it over the
//! public `apiv11` (see "Catalog flow" in `docs/02-architecture.md`). The model types here are
//! GMM's own, so the UI never depends on GameBanana's field names.
//!
//! Text fields that hold HTML (`description_html`, `license_html`, update text) come straight
//! from GameBanana and must be sanitized before display (docs/04-security.md, T3).

mod cache;
mod error;
mod gamebanana;
mod model;

use std::future::Future;

pub use cache::{CacheFuture, HttpCache, MemoryCache, NoCache};
pub use error::CatalogError;
pub use gamebanana::GameBanana;
pub use model::*;

pub trait CatalogSource: Send + Sync {
    fn list_mods(
        &self,
        query: &ListQuery,
    ) -> impl Future<Output = Result<Page<ModSummary>, CatalogError>> + Send;

    fn search(
        &self,
        query: &SearchQuery,
    ) -> impl Future<Output = Result<Page<ModSummary>, CatalogError>> + Send;

    fn mod_profile(
        &self,
        mod_id: u64,
    ) -> impl Future<Output = Result<ModProfile, CatalogError>> + Send;

    /// Current and archived files. Not cached for long, since it is read right before a
    /// download.
    fn mod_files(&self, mod_id: u64)
    -> impl Future<Output = Result<ModFiles, CatalogError>> + Send;

    fn mod_updates(
        &self,
        mod_id: u64,
        page: u32,
    ) -> impl Future<Output = Result<Page<ModUpdate>, CatalogError>> + Send;

    /// Root categories of a game.
    fn categories(
        &self,
        game_id: u64,
    ) -> impl Future<Output = Result<Vec<Category>, CatalogError>> + Send;

    /// Children of a category (for skins: one per character).
    fn subcategories(
        &self,
        category_id: u64,
    ) -> impl Future<Output = Result<Vec<Category>, CatalogError>> + Send;
}
