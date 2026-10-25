// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Mods Store's featured gallery (docs/08, "Mods Store" and "What the data allows").
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
