// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Mods Store's featured gallery.
//!
//! Featured mods are the game's top submissions (GameBanana's `TopSubs`: the top few per
//! period) in GameBanana's order, minus sensitive ones unless the user opted in. Most top
//! submissions are flagged as sensitive (observed 2026-10-25: 18 of 21 for ZZZ), so the list
//! is backfilled with the most liked of the newest mods up to [`FEATURED_COUNT`]. Each pick is
//! read from its mod page, which has what a top-submission record lacks (views, downloads,
//! character, images) and the content-rating flag, checked again here.

use std::collections::HashSet;

use gmm_adapters::GameManifest;
use gmm_catalog::{CatalogSource, ListQuery, ModSummary, Sort, TopPeriod};
use serde::Serialize;

use crate::error::CoreError;

/// How many mods the featured gallery shows.
pub const FEATURED_COUNT: usize = 5;

/// How many of the newest mods the backfill picks from.
const BACKFILL_POOL: u32 = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FeaturedMod {
    /// From the mod page, so the stats include downloads.
    pub summary: ModSummary,
    /// The trending window it was picked from; `None` for a backfilled mod.
    pub period: Option<TopPeriod>,
}

/// The featured gallery for `game`. A mod page that fails to load is skipped; the call
/// fails only when neither the top submissions nor the backfill list can be read.
pub async fn featured<C: CatalogSource>(
    catalog: &C,
    game: &GameManifest,
    show_sensitive: bool,
) -> Result<Vec<FeaturedMod>, CoreError> {
    let game_id = game.gamebanana.game_id;
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(FEATURED_COUNT);

    let top = catalog.top_subs(game_id).await;
    let top_failed = top.is_err();
    let picks = top
        .unwrap_or_default()
        .into_iter()
        .filter(|t| show_sensitive || !t.visibility.is_sensitive())
        .map(|t| (t.mod_id, t.period));
    add_picks(catalog, picks, show_sensitive, &mut seen, &mut out).await;
    if out.len() >= FEATURED_COUNT {
        return Ok(out);
    }

    let query = ListQuery {
        per_page: BACKFILL_POOL,
        sort: Sort::Newest,
        ..ListQuery::new(game_id)
    };
    let mut newest = match catalog.list_mods(&query).await {
        Ok(page) => page.items,
        Err(e) if top_failed => return Err(e.into()),
        Err(_) => return Ok(out),
    };
    newest.retain(|m| show_sensitive || !m.is_sensitive());
    // Stable: equal likes keep the newest first.
    newest.sort_by_key(|m| std::cmp::Reverse(m.stats.likes));
    let picks = newest.into_iter().map(|m| (m.id, None));
    add_picks(catalog, picks, show_sensitive, &mut seen, &mut out).await;
    Ok(out)
}

async fn add_picks<C: CatalogSource>(
    catalog: &C,
    picks: impl Iterator<Item = (u64, Option<TopPeriod>)>,
    show_sensitive: bool,
    seen: &mut HashSet<u64>,
    out: &mut Vec<FeaturedMod>,
) {
    for (mod_id, period) in picks {
        if out.len() >= FEATURED_COUNT {
            return;
        }
        if !seen.insert(mod_id) {
            continue;
        }
        let Ok(profile) = catalog.mod_profile(mod_id).await else {
            continue;
        };
        if !profile.is_installable() || (!show_sensitive && profile.summary.is_sensitive()) {
            continue;
        }
        out.push(FeaturedMod {
            summary: profile.summary,
            period,
        });
    }
}

/// Most lists one Store page merges: characters ticked at once, or games under "All games".
pub const MAX_BROWSE_STREAMS: usize = 6;

/// A Store page: one list per `(GameBanana game id, category)` pair, merged. GameBanana
/// filters by one category per request (a comma list is refused, observed 2026-10-25), so
/// several ticked characters, or "All games", become several lists read side by side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Browse {
    pub streams: Vec<(u64, Option<u64>)>,
    /// 1-based; page `n` of the merge is page `n` of every list.
    pub page: u32,
    /// For the whole merged page; split between the lists (at least 10 each).
    pub per_page: u32,
    pub sort: Sort,
    pub name: Option<String>,
}

/// Reads one Store page (see [`Browse`]). With one list it is just that list. With several,
/// each list's page is read in turn and merged in the chosen order where listings carry the
/// key (newest, recently updated, likes, views); GameBanana's own order and "most downloaded"
/// (downloads aren't in listings) are interleaved instead. A list that fails is left out;
/// the call fails only when every list does. Lists past the cap are ignored.
pub async fn browse<C: CatalogSource>(
    catalog: &C,
    q: &Browse,
) -> Result<gmm_catalog::Page<ModSummary>, CoreError> {
    let streams = &q.streams[..q.streams.len().min(MAX_BROWSE_STREAMS)];
    let n = u32::try_from(streams.len()).unwrap_or(1).max(1);
    let per_list = if streams.len() > 1 {
        q.per_page.div_ceil(n).max(10)
    } else {
        q.per_page
    };
    let mut pages = Vec::with_capacity(streams.len());
    let mut last_error = None;
    for &(game_id, category_id) in streams {
        let query = ListQuery {
            page: q.page.max(1),
            per_page: per_list,
            sort: q.sort,
            category_id,
            name: q.name.clone(),
            ..ListQuery::new(game_id)
        };
        match catalog.list_mods(&query).await {
            Ok(page) => pages.push(page),
            Err(e) => last_error = Some(e),
        }
    }
    if pages.is_empty() {
        return match last_error {
            Some(e) => Err(e.into()),
            None => Ok(gmm_catalog::Page {
                items: Vec::new(),
                total: 0,
                page: q.page.max(1),
                per_page: q.per_page,
                is_last: true,
            }),
        };
    }
    if pages.len() == 1 && last_error.is_none() {
        return Ok(pages.remove(0));
    }
    let total = pages.iter().map(|p| p.total).sum();
    let is_last = pages.iter().all(|p| p.is_last);
    let per_page = pages.iter().map(|p| p.per_page).sum();
    let items = merge(pages.into_iter().map(|p| p.items).collect(), q.sort);
    Ok(gmm_catalog::Page {
        items,
        total,
        page: q.page.max(1),
        per_page,
        is_last,
    })
}

/// Merges lists into one, by `sort` where the listing has the key, else round-robin.
fn merge(lists: Vec<Vec<ModSummary>>, sort: Sort) -> Vec<ModSummary> {
    let key: Option<fn(&ModSummary) -> i64> = match sort {
        Sort::Newest => Some(|m| m.date_added),
        Sort::RecentlyUpdated => Some(|m| m.date_updated.unwrap_or(m.date_modified)),
        Sort::MostLiked => Some(|m| i64::try_from(m.stats.likes).unwrap_or(i64::MAX)),
        Sort::MostViewed => Some(|m| i64::try_from(m.stats.views).unwrap_or(i64::MAX)),
        Sort::Default | Sort::MostDownloaded => None,
    };
    let mut iters: Vec<_> = lists.into_iter().map(Vec::into_iter).collect();
    let mut out = Vec::new();
    loop {
        let mut took = false;
        for it in &mut iters {
            if let Some(m) = it.next() {
                out.push(m);
                took = true;
            }
        }
        if !took {
            break;
        }
    }
    if let Some(key) = key {
        // Stable, so equal keys keep the interleaved order.
        out.sort_by_key(|m| std::cmp::Reverse(key(m)));
    }
    out
}
