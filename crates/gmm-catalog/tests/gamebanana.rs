// SPDX-License-Identifier: MIT OR Apache-2.0

//! GameBanana client against a mock server serving synthetic fixtures (same shape as the
//! live API, invented content). The `live_*` tests hit the real API and are ignored by
//! default: `cargo test -p gmm-catalog -- --ignored`.

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use gmm_catalog::{
    CatalogError, CatalogSource, GameBanana, ListQuery, MemoryCache, NoCache, SearchQuery, Sort,
    TopPeriod, Visibility,
};
use gmm_net::{Allowlist, Client, NetConfig, RateLimit, Url};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

fn catalog(server: &MockServer, cache: Arc<dyn gmm_catalog::HttpCache>) -> GameBanana {
    let base = Url::parse(&format!("{}/apiv11/", server.uri())).unwrap();
    GameBanana::with_base_url(net(), cache, base)
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

async fn serve(server: &MockServer, p: &str, file: &str) {
    Mock::given(method("GET"))
        .and(path(p))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture(file)))
        .mount(server)
        .await;
}

#[tokio::test]
async fn list_mods_sends_filters_and_maps_records() {
    let server = MockServer::start().await;
    Mock::given(path("/apiv11/Mod/Index"))
        .and(query_param("_nPage", "2"))
        .and(query_param("_nPerpage", "2"))
        .and(query_param("_aFilters[Generic_Game]", "19567"))
        .and(query_param("_aFilters[Generic_Category]", "30305"))
        .and(query_param("_sSort", "Generic_MostDownloaded"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("mod_index.json")))
        .mount(&server)
        .await;
    let query = ListQuery {
        page: 2,
        per_page: 2,
        sort: Sort::MostDownloaded,
        category_id: Some(30305),
        ..ListQuery::new(19567)
    };
    let page = catalog(&server, Arc::new(NoCache))
        .list_mods(&query)
        .await
        .unwrap();
    assert_eq!(
        (page.total, page.page, page.per_page, page.is_last),
        (1234, 2, 2, false)
    );
    assert_eq!(page.items.len(), 2);

    let a = &page.items[0];
    assert_eq!(a.id, 900001);
    assert_eq!(a.submitter.name, "test_author");
    assert_eq!(a.root_category.as_ref().unwrap().id, Some(30305));
    let character = a.category.as_ref().unwrap();
    assert_eq!(
        (character.id, character.name.as_str()),
        (Some(43004), "Test Character")
    );
    assert_eq!(a.images.len(), 2);
    let banner = a.banner().unwrap();
    assert_eq!(
        banner.url,
        "https://images.gamebanana.com/img/ss/mods/aaaa1111.jpg"
    );
    assert_eq!(
        banner.url_for_width(200),
        "https://images.gamebanana.com/img/ss/mods/220-90_aaaa1111.jpg"
    );
    assert_eq!(
        banner.url_for_width(2000),
        banner.url,
        "falls back to the original"
    );
    assert!(a.featured);
    assert_eq!(a.visibility, Visibility::Show);

    let b = &page.items[1];
    assert_eq!(b.id, 900002, "string id accepted");
    assert!(b.images.is_empty(), "[] preview accepted");
    assert!(b.category.is_none());
    assert_eq!(b.version, None, "empty version is None");
    assert_eq!(b.date_updated, Some(1790600000));
    assert!(b.visibility.is_sensitive());
    assert!(b.has_content_ratings);

    let mut visible = page.clone();
    assert_eq!(visible.hide_sensitive(), 1);
    assert_eq!(visible.items.len(), 1);
    assert_eq!(visible.items[0].id, 900001);
}

#[tokio::test]
async fn mod_profile_maps_everything_the_mod_page_needs() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/apiv11/Mod/900001/ProfilePage",
        "mod_profile.json",
    )
    .await;
    let profile = catalog(&server, Arc::new(NoCache))
        .mod_profile(900001)
        .await
        .unwrap();
    let s = &profile.summary;
    assert_eq!(s.name, "Test Outfit A");
    assert_eq!(s.game_id, Some(19567));
    assert_eq!(s.root_category.as_ref().unwrap().name, "Character Skins");
    assert_eq!(s.category.as_ref().unwrap().id, Some(43004));
    assert_eq!(s.images.len(), 3, "banner + slideshow");
    assert_eq!(s.stats.downloads, Some(789));
    // HTML is passed through untouched; the UI sanitizes it.
    assert!(profile.description_html.contains("<script>"));
    assert_eq!(profile.license.yes, ["Download and install this Mod"]);
    assert_eq!(profile.license.no, ["Use this Mod commercially"]);
    assert_eq!(profile.credits.len(), 2);
    assert_eq!(
        profile.credits[0].authors[0].role.as_deref(),
        Some("textures")
    );
    assert_eq!(profile.credits[1].authors[0].member_id, None);
    assert_eq!(profile.updates_count, 2);
    assert!(profile.is_installable());

    let files = &profile.files;
    assert_eq!(files.files.len(), 2);
    assert_eq!(files.archived_files.len(), 1);
    assert!(files.archived_files[0].archived);
    assert_eq!(files.files[1].size, 3000, "string size accepted");
    assert!(!files.files[1].is_clean(), "not yet scanned");
    // The newest file isn't scanned yet, so the default choice is the clean one.
    let choice = files.default_choice().unwrap();
    assert_eq!(choice.id, 1000002);
    assert_eq!(
        choice.md5.as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
}

#[tokio::test]
async fn mod_files_and_updates() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/apiv11/Mod/900001/DownloadPage",
        "mod_profile.json",
    )
    .await;
    Mock::given(path("/apiv11/Mod/900001/Updates"))
        .and(query_param("_nPage", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("mod_updates.json")))
        .mount(&server)
        .await;
    let gb = catalog(&server, Arc::new(NoCache));
    let files = gb.mod_files(900001).await.unwrap();
    assert_eq!(files.files.len(), 2);
    let updates = gb.mod_updates(900001, 1).await.unwrap();
    assert!(updates.is_last);
    let u = &updates.items[0];
    assert_eq!(u.title, "Patch 3.3 fix");
    assert_eq!(u.changelog[0].category, "Bugfix");
    assert_eq!(u.file_ids, [1000002]);
}

#[tokio::test]
async fn categories_and_subcategories() {
    let server = MockServer::start().await;
    Mock::given(path("/apiv11/Mod/Categories"))
        .and(query_param("_idGameRow", "19567"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("categories.json")))
        .mount(&server)
        .await;
    Mock::given(path("/apiv11/Mod/Categories"))
        .and(query_param("_idCategoryRow", "30305"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&server)
        .await;
    let gb = catalog(&server, Arc::new(NoCache));
    let cats = gb.categories(19567).await.unwrap();
    assert_eq!(cats.len(), 2);
    assert_eq!(cats[1].item_count, 5136);
    assert_eq!(cats[1].icon_url, None);
    assert!(gb.subcategories(30305).await.unwrap().is_empty());
}

#[tokio::test]
async fn search_filters_to_mods() {
    let server = MockServer::start().await;
    let body = r#"{"_aMetadata":{"_nRecordCount":2,"_nPerpage":15,"_bIsComplete":true},
        "_aRecords":[{"_idRow":1,"_sModelName":"Mod","_sName":"Ellen outfit"},
                     {"_idRow":2,"_sModelName":"Question","_sName":"How do I install?"}]}"#;
    Mock::given(path("/apiv11/Util/Search/Results"))
        .and(query_param("_sModelName", "Mod"))
        .and(query_param("_sSearchString", "ellen outfit"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let page = catalog(&server, Arc::new(NoCache))
        .search(&SearchQuery {
            game_id: 19567,
            text: "ellen outfit".into(),
            page: 1,
        })
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name, "Ellen outfit");
}

#[tokio::test]
async fn top_subs_keep_order_and_drop_other_models() {
    let server = MockServer::start().await;
    serve(&server, "/apiv11/Game/19567/TopSubs", "top_subs.json").await;
    let top = catalog(&server, Arc::new(NoCache))
        .top_subs(19567)
        .await
        .unwrap();
    let ids: Vec<u64> = top.iter().map(|t| t.mod_id).collect();
    assert_eq!(ids, [900001, 900002, 900004]);
    assert_eq!(top[0].period, Some(TopPeriod::Today));
    assert_eq!(top[0].visibility, Visibility::Show);
    assert_eq!(top[1].period, Some(TopPeriod::ThreeMonths));
    assert_eq!(
        (top[1].visibility.clone(), top[1].likes),
        (Visibility::Hide, 1200)
    );
    assert_eq!(top[2].period, None);
}

#[tokio::test]
async fn mod_stats_asks_for_the_counters_only() {
    let server = MockServer::start().await;
    Mock::given(path("/apiv11/Mod/900001"))
        .and(query_param(
            "_csvProperties",
            "_nLikeCount,_nViewCount,_nPostCount,_nDownloadCount",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"_nDownloadCount": 504, "_nViewCount": "6242", "_nLikeCount": 63}"#,
        ))
        .mount(&server)
        .await;
    let stats = catalog(&server, Arc::new(NoCache))
        .mod_stats(900001)
        .await
        .unwrap();
    assert_eq!(
        (stats.likes, stats.views, stats.posts, stats.downloads),
        (63, 6242, 0, Some(504))
    );
}

#[tokio::test]
async fn errors_are_mapped() {
    let server = MockServer::start().await;
    Mock::given(path("/apiv11/Mod/1/ProfilePage"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_string(r#"{"_sErrorCode":"NO_SUCH_RECORD","_sErrorMessage":"x"}"#),
        )
        .mount(&server)
        .await;
    Mock::given(path("/apiv11/Mod/Index"))
        .respond_with(ResponseTemplate::new(400).set_body_string(
            r#"{"_sErrorCode":"INPUT_ERRORS","_aErrorData":{"_sSort":{"_sErrorMessage":"Sort alias not recognized"}}}"#,
        ))
        .mount(&server)
        .await;
    Mock::given(path("/apiv11/Mod/2/ProfilePage"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>maintenance</html>"))
        .mount(&server)
        .await;
    let gb = catalog(&server, Arc::new(NoCache));
    assert!(matches!(
        gb.mod_profile(1).await,
        Err(CatalogError::NotFound)
    ));
    match gb.list_mods(&ListQuery::new(19567)).await {
        Err(CatalogError::BadRequest(m)) => assert!(m.contains("Sort alias"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        gb.mod_profile(2).await,
        Err(CatalogError::Decode { .. })
    ));
}

#[tokio::test]
async fn responses_are_cached() {
    let server = MockServer::start().await;
    serve(
        &server,
        "/apiv11/Mod/900001/ProfilePage",
        "mod_profile.json",
    )
    .await;
    let gb = catalog(&server, Arc::new(MemoryCache::default()));
    gb.mod_profile(900001).await.unwrap();
    gb.mod_profile(900001).await.unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn failed_responses_are_not_cached() {
    let server = MockServer::start().await;
    Mock::given(path("/apiv11/Mod/900001/ProfilePage"))
        .respond_with(ResponseTemplate::new(200).set_body_string("garbage"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    serve(
        &server,
        "/apiv11/Mod/900001/ProfilePage",
        "mod_profile.json",
    )
    .await;
    let gb = catalog(&server, Arc::new(MemoryCache::default()));
    assert!(gb.mod_profile(900001).await.is_err());
    assert!(gb.mod_profile(900001).await.is_ok());
}

// ---- live API (ignored by default) -----------------------------------------------------------

fn live() -> GameBanana {
    GameBanana::new(
        Client::new(NetConfig::production()).unwrap(),
        Arc::new(NoCache),
    )
}

#[tokio::test]
#[ignore = "hits the live GameBanana API"]
async fn live_list_and_profile_for_both_games() {
    let gb = live();
    for game_id in [19567, 20357] {
        let page = gb.list_mods(&ListQuery::new(game_id)).await.unwrap();
        assert!(page.total > 100, "game {game_id}: {}", page.total);
        assert!(!page.items.is_empty());
        let first = &page.items[0];
        assert!(first.id > 0 && !first.name.is_empty());
        let profile = gb.mod_profile(first.id).await.unwrap();
        assert_eq!(profile.summary.id, first.id);
        let cats = gb.categories(game_id).await.unwrap();
        assert!(!cats.is_empty());
    }
}

#[tokio::test]
#[ignore = "hits the live GameBanana API"]
async fn live_top_subs_and_stats() {
    let gb = live();
    for game_id in [19567, 20357] {
        let top = gb.top_subs(game_id).await.unwrap();
        assert!(!top.is_empty(), "game {game_id}");
        assert!(top.iter().all(|t| t.period.is_some()), "{top:?}");
        let stats = gb.mod_stats(top[0].mod_id).await.unwrap();
        assert!(stats.downloads.is_some());
    }
}
