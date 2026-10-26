// SPDX-License-Identifier: MIT OR Apache-2.0

//! Preview images for the app: the `gmmimg://` protocol serves them from here
//! (docs/security.md, R2 and R5).
//!
//! The webview never touches the network and never names a URL. When the app hands catalogue
//! data to the webview, it [`register`](ImageCache::register)s each GameBanana image URL and
//! replaces it with an opaque key (the URL's SHA-256). The protocol handler then asks for the
//! key: GMM fetches the image through `gmm-net` on first use, keeps it in `cache/images/`,
//! and serves only bytes that start like a JPEG, PNG, WebP or GIF. A key GMM never handed out
//! (and that isn't cached from an earlier run) is refused.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use gmm_fsguard::{Access, Guard, ScopedPath};
use gmm_net::{Client, Url};
use sha2::{Digest, Sha256};

use crate::error::CoreError;
use crate::storage::Size;

/// GameBanana serves every preview image, avatar and category icon from this host
/// (observed in API responses: `/img/ss/`, `/img/av/`, `/img/ico/`).
pub const IMAGE_HOST: &str = "images.gamebanana.com";

/// Images in mod descriptions also come from GameBanana's file hosts (observed 2026-10-25:
/// `files.gamebanana.com` and numbered `filecacheNN.gamebanana.com` mirrors). They serve mod
/// archives too, so what comes back is still only kept when it sniffs as an image, and the
/// client's body cap (8 MiB) bounds any mistake.
const FILE_HOST: &str = "files.gamebanana.com";
const FILE_MIRROR_PREFIX: &str = "filecache";
const GAMEBANANA_DOMAIN: &str = ".gamebanana.com";

/// When the cache grows past this, the oldest images are removed.
pub const MAX_IMAGE_CACHE_BYTES: u64 = 512 * 1024 * 1024;

/// Pruning goes down to this share of the cap, so it doesn't run on every new image.
const PRUNE_TO_PERCENT: u64 = 80;

/// An image ready to serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedImage {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

/// See the module docs. Shared by every protocol request (cheap to share behind an `Arc`).
#[derive(Debug)]
pub struct ImageCache {
    guard: Guard,
    /// `<cache>/images/`.
    dir: ScopedPath,
    net: Client,
    /// Keys handed out in this run, and their URLs.
    known: Mutex<HashMap<String, Url>>,
    /// Approximate size of `images/` (measured at start, then counted up).
    total: AtomicU64,
    max_total: u64,
}

impl ImageCache {
    /// Opens `<cache>/images/`, creating it on first run. `net` should have its own rate
    /// limit, so a page of thumbnails doesn't hold up API calls.
    pub fn open(cache_dir: &Path, net: Client) -> Result<Self, CoreError> {
        Self::with_cap(cache_dir, net, MAX_IMAGE_CACHE_BYTES)
    }

    fn with_cap(cache_dir: &Path, net: Client, max_total: u64) -> Result<Self, CoreError> {
        let mut guard = Guard::new();
        let cache = guard.ensure_root("cache", cache_dir, Access::ReadWrite)?;
        let dir = guard.path(cache, "images")?;
        guard.create_dir_all(&dir)?;
        let total = guard
            .read_dir(&dir)
            .map(|entries| entries.iter().filter(|e| !e.is_dir).map(|e| e.len).sum())
            .unwrap_or(0);
        Ok(Self {
            guard,
            dir,
            net,
            known: Mutex::new(HashMap::new()),
            total: AtomicU64::new(total),
            max_total,
        })
    }

    /// The key for a GameBanana image URL, remembered so [`get`](Self::get) may fetch it.
    /// `None` for anything that isn't an HTTPS URL on [`IMAGE_HOST`] or GameBanana's file
    /// hosts (the caller then shows no image).
    pub fn register(&self, url: &str) -> Option<String> {
        let parsed = Url::parse(url).ok()?;
        if !image_url_allowed(&parsed) || self.net.config().allowlist.check(&parsed).is_err() {
            return None;
        }
        let key = hex::encode(Sha256::digest(parsed.as_str().as_bytes()));
        self.known
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key.clone(), parsed);
        Some(key)
    }

    /// The image for `key`: from the cache, or fetched once and cached.
    pub async fn get(&self, key: &str) -> Result<CachedImage, CoreError> {
        if !is_key(key) {
            return Err(CoreError::UnknownImage);
        }
        let path = self.dir.join(key)?;
        if let Ok(bytes) = self.guard.read(&path)
            && let Some(mime) = sniff(&bytes)
        {
            return Ok(CachedImage { bytes, mime });
        }
        let url = self
            .known
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
            .cloned()
            .ok_or(CoreError::UnknownImage)?;
        let response = self.net.get(&url).await?;
        if !response.is_success() {
            return Err(CoreError::NotAnImage(format!("HTTP {}", response.status)));
        }
        let mime = sniff(&response.body)
            .ok_or_else(|| CoreError::NotAnImage("not a JPEG, PNG, WebP or GIF".to_owned()))?;
        self.guard.write_atomic(&path, &response.body)?;
        let len = response.body.len() as u64;
        if self.total.fetch_add(len, Ordering::Relaxed) + len > self.max_total {
            self.prune(&path);
        }
        Ok(CachedImage {
            bytes: response.body,
            mime,
        })
    }

    /// Deletes every cached image for good (Settings › Storage › Clear). Keys handed out in
    /// this run stay valid: their images are fetched again when next shown.
    pub fn clear(&self) -> Size {
        let cleared = delete_images(&self.guard, &self.dir);
        self.total.store(0, Ordering::Relaxed);
        cleared
    }

    /// Removes the oldest images until the cache is well under its cap. Keeps `keep` (the
    /// image just written). Best effort.
    fn prune(&self, keep: &ScopedPath) {
        let Ok(entries) = self.guard.read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(SystemTime, u64, ScopedPath)> = entries
            .into_iter()
            .filter(|e| !e.is_dir && !e.is_symlink && &e.path != keep)
            .map(|e| {
                let modified = self
                    .guard
                    .metadata(&e.path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                (modified, e.len, e.path)
            })
            .collect();
        files.sort_by_key(|(modified, _, _)| *modified);
        let keep_len = self.guard.metadata(keep).map_or(0, |m| m.len());
        let mut total: u64 = files.iter().map(|(_, len, _)| len).sum::<u64>() + keep_len;
        let target = self.max_total / 100 * PRUNE_TO_PERCENT;
        for (_, len, path) in files {
            if total <= target {
                break;
            }
            if self.guard.remove_file(&path).is_ok() {
                total -= len;
            }
        }
        self.total.store(total, Ordering::Relaxed);
    }
}

/// Deletes every cached image in `<cache>/images/` for good, for the CLI (`gmm storage
/// --clear-images`); the app clears through its running [`ImageCache::clear`].
pub fn clear_image_cache(cache_dir: &Path) -> Result<Size, CoreError> {
    let mut guard = Guard::new();
    let cache = guard.ensure_root("cache", cache_dir, Access::ReadWrite)?;
    let dir = guard.path(cache, "images")?;
    Ok(delete_images(&guard, &dir))
}

/// Best effort: a file that can't be removed (in use) stays and isn't counted.
fn delete_images(guard: &Guard, dir: &ScopedPath) -> Size {
    let mut cleared = Size::default();
    let Ok(entries) = guard.read_dir(dir) else {
        return cleared;
    };
    for e in entries {
        if e.is_dir || e.is_symlink || !is_key(&e.name) {
            continue;
        }
        if guard.remove_file(&e.path).is_ok() {
            cleared.files += 1;
            cleared.bytes += e.len;
        }
    }
    cleared
}

fn image_url_allowed(url: &Url) -> bool {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let mirror = host
        .strip_suffix(GAMEBANANA_DOMAIN)
        .and_then(|label| label.strip_prefix(FILE_MIRROR_PREFIX))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    if url.scheme() == "https" && (host == IMAGE_HOST || host == FILE_HOST || mirror) {
        return true;
    }
    // Unit tests serve images from a local mock server.
    cfg!(test) && url.scheme() == "http" && url.host_str() == Some("127.0.0.1")
}

/// Keys are SHA-256 hashes in lowercase hex, which also makes them safe file names.
fn is_key(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The image type, from the first bytes. Anything else (SVG, HTML, ...) is never served.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some("image/png"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Some("image/gif"),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods, clippy::unwrap_used)]
mod tests {
    use gmm_net::{Allowlist, NetConfig};
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-png";

    fn client() -> Client {
        Client::new(NetConfig {
            allowlist: Allowlist::with_loopback_for_tests(),
            ..NetConfig::production()
        })
        .unwrap()
    }

    async fn server() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(path("/img/ss/a.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG))
            .mount(&server)
            .await;
        Mock::given(path("/img/ss/page.svg"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<svg onload=x()>"))
            .mount(&server)
            .await;
        server
    }

    #[test]
    fn only_gamebanana_image_urls_get_a_key() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::open(dir.path(), client()).unwrap();
        let key = cache
            .register("https://images.gamebanana.com/img/ss/mods/x.jpg")
            .unwrap();
        assert!(is_key(&key));
        for description_host in [
            "https://files.gamebanana.com/bitpit/x.png",
            "https://filecache41.gamebanana.com/img/x.jpg",
        ] {
            assert!(
                cache.register(description_host).is_some(),
                "{description_host}"
            );
        }
        for bad in [
            "https://evil.example/img.png",
            "https://gamebanana.com/mods/1",
            "https://filecache.gamebanana.com/x.png",
            "https://filecache4x.gamebanana.com/x.png",
            "https://filecache41.gamebanana.com.evil.example/x.png",
            "https://i.imgur.com/x.png",
            "http://images.gamebanana.com/img/ss/x.jpg",
            "https://user:pw@images.gamebanana.com/x.jpg",
            "https://images.gamebanana.com:8443/x.jpg",
            "file:///C:/Windows/win.ini",
            "not a url",
        ] {
            assert_eq!(cache.register(bad), None, "{bad}");
        }
    }

    #[tokio::test]
    async fn fetches_once_then_serves_from_disk() {
        let server = server().await;
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::open(dir.path(), client()).unwrap();
        let key = cache
            .register(&format!("{}/img/ss/a.png", server.uri()))
            .unwrap();
        let first = cache.get(&key).await.unwrap();
        assert_eq!(first.mime, "image/png");
        assert_eq!(first.bytes, PNG);

        // A new run knows no URLs, but the cached file is still served.
        drop(server);
        let again = ImageCache::open(dir.path(), client()).unwrap();
        assert_eq!(again.get(&key).await.unwrap(), first);
    }

    #[tokio::test]
    async fn clearing_deletes_the_files_and_keeps_the_keys() {
        let server = server().await;
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::open(dir.path(), client()).unwrap();
        let key = cache
            .register(&format!("{}/img/ss/a.png", server.uri()))
            .unwrap();
        cache.get(&key).await.unwrap();
        // Something that isn't an image key stays.
        std::fs::write(dir.path().join("images").join("notes.txt"), b"x").unwrap();

        let cleared = cache.clear();
        assert_eq!((cleared.files, cleared.bytes), (1, PNG.len() as u64));
        assert!(!dir.path().join("images").join(&key).exists());
        assert!(dir.path().join("images").join("notes.txt").exists());
        // Shown again, it's fetched again.
        assert_eq!(cache.get(&key).await.unwrap().bytes, PNG);
        assert_eq!(clear_image_cache(dir.path()).unwrap().files, 1);
    }

    #[tokio::test]
    async fn unknown_keys_and_non_images_are_refused() {
        let server = server().await;
        let dir = tempfile::tempdir().unwrap();
        let cache = ImageCache::open(dir.path(), client()).unwrap();
        for key in ["../gmm.db", "", "ABC", &"a".repeat(64), &"a".repeat(65)] {
            assert!(
                matches!(cache.get(key).await, Err(CoreError::UnknownImage)),
                "{key}"
            );
        }
        let svg = cache
            .register(&format!("{}/img/ss/page.svg", server.uri()))
            .unwrap();
        assert!(matches!(
            cache.get(&svg).await,
            Err(CoreError::NotAnImage(_))
        ));
        let missing = cache
            .register(&format!("{}/img/ss/gone.png", server.uri()))
            .unwrap();
        assert!(matches!(
            cache.get(&missing).await,
            Err(CoreError::NotAnImage(_))
        ));
        // Nothing that failed was written.
        let names: Vec<_> = std::fs::read_dir(dir.path().join("images"))
            .unwrap()
            .collect();
        assert!(names.is_empty());
    }

    #[tokio::test]
    async fn oldest_images_go_when_the_cache_is_full() {
        let server = MockServer::start().await;
        for name in ["1", "2", "3"] {
            Mock::given(path(format!("/img/ss/{name}.png")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG.repeat(10)))
                .mount(&server)
                .await;
        }
        let dir = tempfile::tempdir().unwrap();
        let size = PNG.len() as u64 * 10;
        let cache = ImageCache::with_cap(dir.path(), client(), size * 2).unwrap();
        let mut keys = Vec::new();
        for name in ["1", "2", "3"] {
            let key = cache
                .register(&format!("{}/img/ss/{name}.png", server.uri()))
                .unwrap();
            cache.get(&key).await.unwrap();
            keys.push(key);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let on_disk = |k: &str| dir.path().join("images").join(k).is_file();
        assert!(!on_disk(&keys[0]), "oldest image should be pruned");
        assert!(on_disk(&keys[2]), "newest image must stay");
    }

    #[test]
    fn sniffs_only_raster_images() {
        assert_eq!(sniff(b"\xFF\xD8\xFF\xE0"), Some("image/jpeg"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"GIF89a"), Some("image/gif"));
        assert_eq!(sniff(b"<svg"), None);
        assert_eq!(sniff(b"<!doctype html>"), None);
        assert_eq!(sniff(b""), None);
    }
}
