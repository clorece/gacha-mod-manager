// SPDX-License-Identifier: MIT OR Apache-2.0

//! The featured gallery against a mock GameBanana: order, the sensitive filter, the backfill.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use gmm_adapters::{GameManifest, Registry};
use gmm_catalog::{GameBanana, NoCache, TopPeriod};
use gmm_core::{FEATURED_COUNT, featured};
use gmm_net::{Allowlist, Client, NetConfig, RateLimit, Url};
use serde_json::{Value, json};
use wiremock::matchers::{path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn zzz() -> &'static GameManifest {
    Registry::builtin().unwrap().get("zzz").unwrap()
}

fn net() -> Client {
    Client::new(NetConfig {
        allowlist: Allowlist::with_loopback_for_tests(),
        rate_limit: RateLimit {
            per_second: 1000.0,
            burst: 1000,
        },
        max_backoff: Duration::from_millis(10),
        ..NetConfig::production()
    })
    .unwrap()
}

async fn respond(server: &MockServer, p: &str, body: Value) {
    Mock::given(path(p))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// A mod page. `flagged` sets the content-rating flag that top-submission records lack.
async fn profile(server: &MockServer, id: u64, likes: u64, flagged: bool) {
    let body = json!({
        "_idRow": id,
        "_sName": format!("Mod {id}"),
        "_aGame": { "_idRow": 19567 },
        "_nLikeCount": likes,
        "_nDownloadCount": 10,
        "_bHasContentRatings": flagged,
    });
    respond(server, &format!("/apiv11/Mod/{id}/ProfilePage"), body).await;
}

fn record(id: u64, likes: u64, visibility: &str) -> Value {
    json!({ "_idRow": id, "_sModelName": "Mod", "_sName": format!("Mod {id}"),
            "_nLikeCount": likes, "_sInitialVisibility": visibility })
}

async fn newest(server: &MockServer, records: Vec<Value>) {
    Mock::given(path("/apiv11/Mod/Index"))
        .and(query_param("_sSort", "Generic_Newest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "_aMetadata": { "_nRecordCount": records.len(), "_nPerpage": 50, "_bIsComplete": true },
            "_aRecords": records,
        })))
        .mount(server)
        .await;
}

fn catalog(server: &MockServer) -> GameBanana {
    let base = Url::parse(&format!("{}/apiv11/", server.uri())).unwrap();
    GameBanana::with_base_url(net(), Arc::new(NoCache), base)
}

#[tokio::test]
async fn safe_top_subs_first_then_the_most_liked_newest() {
    let server = MockServer::start().await;
    respond(
        &server,
        "/apiv11/Game/19567/TopSubs",
        json!([
            { "_idRow": 1, "_sModelName": "Mod", "_sPeriod": "today", "_sInitialVisibility": "show" },
            { "_idRow": 2, "_sModelName": "Mod", "_sPeriod": "week", "_sInitialVisibility": "hide" },
            { "_idRow": 3, "_sModelName": "Mod", "_sPeriod": "month", "_sInitialVisibility": "show" },
            { "_idRow": 4, "_sModelName": "Mod", "_sPeriod": "year", "_sInitialVisibility": "show" },
        ]),
    )
    .await;
    profile(&server, 1, 50, false).await;
    profile(&server, 2, 900, false).await;
    // Shown in the record, but its page carries a content rating.
    profile(&server, 3, 40, true).await;
    // No page for 4: skipped.
    newest(
        &server,
        vec![
            record(1, 50, "show"),
            record(10, 5, "show"),
            record(11, 70, "show"),
            record(12, 99, "warn"),
            record(13, 20, "show"),
            record(14, 20, "show"),
            record(15, 1, "show"),
        ],
    )
    .await;
    for (id, likes) in [(10, 5), (11, 70), (13, 20), (14, 20), (15, 1)] {
        profile(&server, id, likes, false).await;
    }

    let gb = catalog(&server);
    let picks = featured(&gb, zzz(), false).await.unwrap();
    let ids: Vec<u64> = picks.iter().map(|f| f.summary.id).collect();
    assert_eq!(ids, [1, 11, 13, 14, 10]);
    assert_eq!(ids.len(), FEATURED_COUNT);
    assert_eq!(picks[0].period, Some(TopPeriod::Today));
    assert!(picks[1..].iter().all(|f| f.period.is_none()));
    assert_eq!(picks[0].summary.stats.downloads, Some(10));

    // Opted in: the flagged ones count too, and the top submissions fill the gallery.
    let all = featured(&gb, zzz(), true).await.unwrap();
    let ids: Vec<u64> = all.iter().map(|f| f.summary.id).collect();
    // 4 and 12 have no page.
    assert_eq!(ids, [1, 2, 3, 11, 13]);
}

#[tokio::test]
async fn backfill_alone_when_top_subs_fail() {
    let server = MockServer::start().await;
    Mock::given(path("/apiv11/Game/19567/TopSubs"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    newest(&server, vec![record(20, 3, "show"), record(21, 8, "show")]).await;
    profile(&server, 20, 3, false).await;
    profile(&server, 21, 8, false).await;
    let picks = featured(&catalog(&server), zzz(), false).await.unwrap();
    let ids: Vec<u64> = picks.iter().map(|f| f.summary.id).collect();
    assert_eq!(ids, [21, 20]);
}

#[tokio::test]
async fn fails_when_nothing_can_be_read() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    assert!(featured(&catalog(&server), zzz(), false).await.is_err());
}
