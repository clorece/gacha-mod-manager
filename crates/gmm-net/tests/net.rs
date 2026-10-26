// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

use std::ops::ControlFlow;
use std::time::Duration;

use gmm_fsguard::{Access, Guard, RootId};
use gmm_net::{Allowlist, Client, DownloadRequest, NetConfig, NetError, RateLimit, Slowdown, Url};
use md5::Md5;
use sha2::{Digest, Sha256};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_client() -> Client {
    Client::new(NetConfig {
        allowlist: Allowlist::with_loopback_for_tests(),
        rate_limit: RateLimit {
            per_second: 1000.0,
            burst: 1000,
        },
        max_body_bytes: 1024,
        max_backoff: Duration::from_millis(10),
        ..NetConfig::production()
    })
    .unwrap()
}

fn url(server: &MockServer, p: &str) -> Url {
    Url::parse(&format!("{}{p}", server.uri())).unwrap()
}

// ---- get ------------------------------------------------------------------------------------

#[tokio::test]
async fn get_returns_body_and_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/apiv11/Mod/1/ProfilePage"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"_idRow":1}"#))
        .mount(&server)
        .await;
    Mock::given(path("/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_string("{}"))
        .mount(&server)
        .await;
    let client = test_client();
    let ok = client
        .get(&url(&server, "/apiv11/Mod/1/ProfilePage"))
        .await
        .unwrap();
    assert_eq!(
        (ok.status, ok.body.as_slice()),
        (200, br#"{"_idRow":1}"#.as_slice())
    );
    // 4xx is the caller's to interpret, not an error.
    let missing = client.get(&url(&server, "/missing")).await.unwrap();
    assert_eq!(missing.status, 404);
}

#[tokio::test]
async fn disallowed_hosts_are_never_contacted() {
    let client = test_client();
    for bad in [
        "https://evil.example/x",
        "http://gamebanana.com/x",
        "https://gamebanana.com.evil.example/x",
    ] {
        let err = client.get(&Url::parse(bad).unwrap()).await.unwrap_err();
        assert!(matches!(err, NetError::Disallowed(_)), "{bad}: {err:?}");
    }
}

#[tokio::test]
async fn production_config_refuses_loopback() {
    let server = MockServer::start().await;
    let client = Client::new(NetConfig::production()).unwrap();
    let err = client.get(&url(&server, "/x")).await.unwrap_err();
    assert!(matches!(err, NetError::Disallowed(_)));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn redirect_to_disallowed_host_is_blocked() {
    let server = MockServer::start().await;
    Mock::given(path("/dl/1"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", "https://evil.example/payload"),
        )
        .mount(&server)
        .await;
    let err = test_client().get(&url(&server, "/dl/1")).await.unwrap_err();
    assert!(matches!(err, NetError::Disallowed(_)), "{err:?}");
}

#[tokio::test]
async fn redirect_to_allowed_host_is_followed() {
    let server = MockServer::start().await;
    let target = format!("{}/files/x", server.uri());
    Mock::given(path("/dl/1"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", target.as_str()))
        .mount(&server)
        .await;
    Mock::given(path("/files/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;
    let resp = test_client().get(&url(&server, "/dl/1")).await.unwrap();
    assert_eq!(resp.body, b"ok");
}

#[tokio::test]
async fn retries_5xx_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(path("/busy"))
        .respond_with(ResponseTemplate::new(502))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(path("/busy"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(path("/busy"))
        .respond_with(ResponseTemplate::new(200).set_body_string("done"))
        .with_priority(3)
        .mount(&server)
        .await;
    let resp = test_client().get(&url(&server, "/busy")).await.unwrap();
    assert_eq!(resp.body, b"done");
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn a_429_pauses_every_sibling_without_more_requests() {
    let server = MockServer::start().await;
    Mock::given(path("/busy"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "120"))
        .mount(&server)
        .await;
    let api = test_client();
    let images = api.sibling(
        RateLimit {
            per_second: 1000.0,
            burst: 1000,
        },
        false,
    );
    let err = api.get(&url(&server, "/busy")).await.unwrap_err();
    assert!(
        matches!(
            err,
            NetError::SlowDown {
                why: Slowdown::RateLimited,
                ..
            }
        ),
        "{err}"
    );
    assert_eq!(err.retry_in(), Some(Duration::from_secs(120)));
    assert!(err.to_string().contains("slow down"), "{err}");
    assert!(api.cooldown_remaining().is_some());

    // Held off everywhere, and nothing more reaches the server.
    let again = images.get(&url(&server, "/anything")).await.unwrap_err();
    assert!(matches!(again, NetError::SlowDown { .. }));
    assert!(again.is_temporary());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);

    // An unrelated client (another app) isn't affected.
    assert!(test_client().cooldown_remaining().is_none());
}

#[tokio::test]
async fn a_cloudflare_challenge_is_a_slow_down_too() {
    let server = MockServer::start().await;
    Mock::given(path("/check"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("cf-mitigated", "challenge")
                .set_body_string("<html>Just a moment...</html>"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/forbidden"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let plain = test_client();
    // A plain 403 is the caller's to read, not a slow-down.
    assert_eq!(
        plain.get(&url(&server, "/forbidden")).await.unwrap().status,
        403
    );
    assert!(plain.cooldown_remaining().is_none());
    let err = plain.get(&url(&server, "/check")).await.unwrap_err();
    assert!(matches!(
        err,
        NetError::SlowDown {
            why: Slowdown::Challenged,
            ..
        }
    ));
    assert_eq!(err.retry_in(), Some(Duration::from_secs(30)));
}

#[tokio::test]
async fn gives_up_after_max_retries() {
    let server = MockServer::start().await;
    Mock::given(path("/down"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let err = test_client().get(&url(&server, "/down")).await.unwrap_err();
    assert!(matches!(err, NetError::Status { status: 503, .. }));
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        4,
        "1 try + 3 retries"
    );
}

#[tokio::test]
async fn oversized_bodies_rejected() {
    let server = MockServer::start().await;
    Mock::given(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 5000]))
        .mount(&server)
        .await;
    let err = test_client().get(&url(&server, "/big")).await.unwrap_err();
    assert!(matches!(err, NetError::BodyTooLarge { limit: 1024, .. }));
}

#[tokio::test]
async fn sends_identifying_user_agent() {
    let server = MockServer::start().await;
    Mock::given(path("/ua"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    test_client().get(&url(&server, "/ua")).await.unwrap();
    let requests = server.received_requests().await.unwrap();
    let ua = requests[0]
        .headers
        .get("user-agent")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ua.starts_with("GMM/"), "{ua}");
}

// ---- download -------------------------------------------------------------------------------

struct Dl {
    _dir: tempfile::TempDir,
    guard: Guard,
    root: RootId,
}

fn dl_env() -> Dl {
    let dir = tempfile::tempdir().unwrap();
    let mut guard = Guard::new();
    let root = guard
        .register_root("cache", dir.path(), Access::ReadWrite)
        .unwrap();
    Dl {
        _dir: dir,
        guard,
        root,
    }
}

fn payload() -> Vec<u8> {
    (0..200_000u32).map(|i| (i % 251) as u8).collect()
}

fn hex_sha256(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hex_md5(data: &[u8]) -> String {
    hex::encode(Md5::digest(data))
}

fn request(
    server: &MockServer,
    env: &Dl,
    size: Option<u64>,
    md5: Option<String>,
) -> DownloadRequest {
    DownloadRequest {
        url: url(server, "/files/mod.zip"),
        dest: env.guard.path(env.root, "downloads/mod.zip").unwrap(),
        expected_size: size,
        expected_md5: md5,
        max_bytes: 10 * 1024 * 1024,
    }
}

async fn serve_file(server: &MockServer, data: Vec<u8>) {
    Mock::given(path("/files/mod.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data))
        .mount(server)
        .await;
}

#[tokio::test]
async fn download_verifies_and_hashes() {
    let server = MockServer::start().await;
    let data = payload();
    serve_file(&server, data.clone()).await;
    let env = dl_env();
    env.guard
        .create_dir_all(&env.guard.path(env.root, "downloads").unwrap())
        .unwrap();
    let req = request(
        &server,
        &env,
        Some(data.len() as u64),
        Some(hex_md5(&data).to_uppercase()),
    );
    let mut calls = 0;
    let outcome = test_client()
        .download(&env.guard, &req, |_| {
            calls += 1;
            ControlFlow::Continue(())
        })
        .await
        .unwrap();
    assert_eq!(outcome.bytes, data.len() as u64);
    assert_eq!(outcome.sha256, hex_sha256(&data));
    assert_eq!(outcome.md5, hex_md5(&data));
    assert!(!outcome.resumed);
    assert!(calls >= 2, "progress reported");
    assert_eq!(env.guard.read(&req.dest).unwrap(), data);
    let part = env.guard.path(env.root, "downloads/.mod.zip.part").unwrap();
    assert!(!env.guard.exists(&part).unwrap(), "part file renamed away");
}

#[tokio::test]
async fn md5_mismatch_deletes_everything() {
    let server = MockServer::start().await;
    serve_file(&server, payload()).await;
    let env = dl_env();
    env.guard
        .create_dir_all(&env.guard.path(env.root, "downloads").unwrap())
        .unwrap();
    let req = request(&server, &env, None, Some("0".repeat(32)));
    let err = test_client()
        .download(&env.guard, &req, |_| ControlFlow::Continue(()))
        .await
        .unwrap_err();
    assert!(matches!(err, NetError::Md5Mismatch { .. }));
    assert!(!env.guard.exists(&req.dest).unwrap());
    let part = env.guard.path(env.root, "downloads/.mod.zip.part").unwrap();
    assert!(!env.guard.exists(&part).unwrap());
}

#[tokio::test]
async fn larger_than_catalog_says_is_rejected() {
    let server = MockServer::start().await;
    serve_file(&server, payload()).await;
    let env = dl_env();
    env.guard
        .create_dir_all(&env.guard.path(env.root, "downloads").unwrap())
        .unwrap();
    let req = request(&server, &env, Some(1000), None);
    let err = test_client()
        .download(&env.guard, &req, |_| ControlFlow::Continue(()))
        .await
        .unwrap_err();
    assert!(matches!(err, NetError::FileTooLarge { limit: 1000 }));
    assert!(!env.guard.exists(&req.dest).unwrap());
}

#[tokio::test]
async fn pause_then_resume_with_range_request() {
    let server = MockServer::start().await;
    let data = payload();
    let half = 120_000usize;
    let env = dl_env();
    env.guard
        .create_dir_all(&env.guard.path(env.root, "downloads").unwrap())
        .unwrap();
    // Simulate an earlier paused download: the first `half` bytes are on disk.
    let part = env.guard.path(env.root, "downloads/.mod.zip.part").unwrap();
    env.guard.write_atomic(&part, &data[..half]).unwrap();

    Mock::given(path("/files/mod.zip"))
        .and(header("range", format!("bytes={half}-").as_str()))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header(
                    "content-range",
                    format!("bytes {half}-{}/{}", data.len() - 1, data.len()).as_str(),
                )
                .set_body_bytes(data[half..].to_vec()),
        )
        .mount(&server)
        .await;

    let req = request(&server, &env, Some(data.len() as u64), Some(hex_md5(&data)));
    let outcome = test_client()
        .download(&env.guard, &req, |_| ControlFlow::Continue(()))
        .await
        .unwrap();
    assert!(outcome.resumed);
    assert_eq!(outcome.sha256, hex_sha256(&data), "hash covers both halves");
    assert_eq!(env.guard.read(&req.dest).unwrap(), data);
}

#[tokio::test]
async fn server_ignoring_range_restarts_cleanly() {
    let server = MockServer::start().await;
    let data = payload();
    serve_file(&server, data.clone()).await; // always 200 with the full body
    let env = dl_env();
    env.guard
        .create_dir_all(&env.guard.path(env.root, "downloads").unwrap())
        .unwrap();
    let part = env.guard.path(env.root, "downloads/.mod.zip.part").unwrap();
    env.guard
        .write_atomic(&part, b"stale partial data")
        .unwrap();
    let req = request(&server, &env, Some(data.len() as u64), None);
    let outcome = test_client()
        .download(&env.guard, &req, |_| ControlFlow::Continue(()))
        .await
        .unwrap();
    assert!(!outcome.resumed);
    assert_eq!(env.guard.read(&req.dest).unwrap(), data);
}

#[tokio::test]
async fn pausing_keeps_the_partial_file() {
    let server = MockServer::start().await;
    serve_file(&server, payload()).await;
    let env = dl_env();
    env.guard
        .create_dir_all(&env.guard.path(env.root, "downloads").unwrap())
        .unwrap();
    let req = request(&server, &env, None, None);
    let err = test_client()
        .download(&env.guard, &req, |p| {
            if p.downloaded > 0 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .await
        .unwrap_err();
    assert!(matches!(err, NetError::Paused));
    let part = env.guard.path(env.root, "downloads/.mod.zip.part").unwrap();
    assert!(env.guard.exists(&part).unwrap());
    assert!(!env.guard.exists(&req.dest).unwrap());
}

#[tokio::test]
async fn existing_destination_refused() {
    let server = MockServer::start().await;
    let env = dl_env();
    let dest = env.guard.path(env.root, "mod.zip").unwrap();
    env.guard.write_atomic(&dest, b"already here").unwrap();
    let req = DownloadRequest {
        dest,
        ..request(&server, &env, None, None)
    };
    let err = test_client()
        .download(&env.guard, &req, |_| ControlFlow::Continue(()))
        .await
        .unwrap_err();
    assert!(matches!(err, NetError::DestinationExists));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_429_on_a_download_keeps_the_partial_file() {
    let server = MockServer::start().await;
    let data = payload();
    let env = dl_env();
    env.guard
        .create_dir_all(&env.guard.path(env.root, "downloads").unwrap())
        .unwrap();
    let part = env.guard.path(env.root, "downloads/.mod.zip.part").unwrap();
    env.guard.write_atomic(&part, &data[..1000]).unwrap();
    Mock::given(path("/files/mod.zip"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    let client = test_client();
    let req = request(&server, &env, Some(data.len() as u64), None);
    let err = client
        .download(&env.guard, &req, |_| ControlFlow::Continue(()))
        .await
        .unwrap_err();
    assert!(matches!(err, NetError::SlowDown { .. }), "{err}");
    assert_eq!(
        env.guard.read(&part).unwrap(),
        &data[..1000],
        "kept for resuming"
    );
    // While paused, a second attempt doesn't even ask.
    let again = client
        .download(&env.guard, &req, |_| ControlFlow::Continue(()))
        .await
        .unwrap_err();
    assert!(matches!(again, NetError::SlowDown { .. }));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

// ---- dropped connections --------------------------------------------------------------------

/// A server that answers each connection from `script`, in turn: `None` reads the request
/// and closes the connection without a word (what an edge server does to a connection it
/// already dropped); `Some(bytes)` writes those bytes and closes.
async fn flaky_server(script: Vec<Option<Vec<u8>>>) -> (Url, tokio::task::JoinHandle<usize>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let handle = tokio::spawn(async move {
        let mut served = 0;
        for answer in script {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = sock.read(&mut buf).await;
            if let Some(bytes) = answer {
                sock.write_all(&bytes).await.unwrap();
            }
            served += 1;
            drop(sock);
        }
        served
    });
    (base, handle)
}

fn http_ok(body: &[u8], extra: &str) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

#[tokio::test]
async fn a_dropped_connection_is_retried() {
    let (base, server) = flaky_server(vec![None, Some(http_ok(b"ok", ""))]).await;
    let resp = test_client()
        .get(&base.join("/api").unwrap())
        .await
        .unwrap();
    assert_eq!((resp.status, resp.body.as_slice()), (200, b"ok".as_slice()));
    assert_eq!(server.await.unwrap(), 2);
}

#[tokio::test]
async fn a_body_cut_short_is_retried() {
    // The first answer promises 10 bytes and stops after 2.
    let cut = b"HTTP/1.1 200 OK\r\ncontent-length: 10\r\nconnection: close\r\n\r\nok".to_vec();
    let (base, server) = flaky_server(vec![Some(cut), Some(http_ok(b"0123456789", ""))]).await;
    let resp = test_client()
        .get(&base.join("/api").unwrap())
        .await
        .unwrap();
    assert_eq!(resp.body, b"0123456789");
    assert_eq!(server.await.unwrap(), 2);
}

#[tokio::test]
async fn a_download_cut_short_resumes_by_itself() {
    let data = payload();
    let half = 50_000usize;
    let mut cut = format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        data.len()
    )
    .into_bytes();
    cut.extend_from_slice(&data[..half]);
    let rest = {
        let mut out = format!(
            "HTTP/1.1 206 Partial Content\r\ncontent-length: {}\r\ncontent-range: bytes {half}-{}/{}\r\nconnection: close\r\n\r\n",
            data.len() - half,
            data.len() - 1,
            data.len()
        )
        .into_bytes();
        out.extend_from_slice(&data[half..]);
        out
    };
    let (base, server) = flaky_server(vec![Some(cut), Some(rest)]).await;
    let env = dl_env();
    env.guard
        .create_dir_all(&env.guard.path(env.root, "downloads").unwrap())
        .unwrap();
    let req = DownloadRequest {
        url: base.join("/files/mod.zip").unwrap(),
        dest: env.guard.path(env.root, "downloads/mod.zip").unwrap(),
        expected_size: Some(data.len() as u64),
        expected_md5: Some(hex_md5(&data)),
        max_bytes: 10 * 1024 * 1024,
    };
    let outcome = test_client()
        .download(&env.guard, &req, |_| ControlFlow::Continue(()))
        .await
        .unwrap();
    assert!(outcome.resumed);
    assert_eq!(outcome.sha256, hex_sha256(&data));
    assert_eq!(env.guard.read(&req.dest).unwrap(), data);
    assert_eq!(server.await.unwrap(), 2);
}
