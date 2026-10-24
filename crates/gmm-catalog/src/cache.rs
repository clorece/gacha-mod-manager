// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The future an [`HttpCache`] method returns. Boxed so the cache can sit behind
/// `Arc<dyn HttpCache>`.
pub type CacheFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Response cache keyed by URL. `gmm-db` provides the SQLite-backed implementation that the
/// CLI uses; [`MemoryCache`] is for tests and for running without a database.
///
/// A cache is best effort: failures are swallowed and look like a miss.
pub trait HttpCache: Send + Sync {
    /// A cached body no older than `max_age`.
    fn get<'a>(&'a self, key: &'a str, max_age: Duration) -> CacheFuture<'a, Option<Vec<u8>>>;
    fn put<'a>(&'a self, key: &'a str, body: Vec<u8>) -> CacheFuture<'a, ()>;
}

/// Never caches.
#[derive(Debug, Default)]
pub struct NoCache;

impl HttpCache for NoCache {
    fn get<'a>(&'a self, _: &'a str, _: Duration) -> CacheFuture<'a, Option<Vec<u8>>> {
        Box::pin(async { None })
    }

    fn put<'a>(&'a self, _: &'a str, _: Vec<u8>) -> CacheFuture<'a, ()> {
        Box::pin(async {})
    }
}

/// In-memory cache with a size bound. When full, the oldest entry is evicted.
#[derive(Debug)]
pub struct MemoryCache {
    max_entries: usize,
    entries: Mutex<HashMap<String, (Instant, Vec<u8>)>>,
}

impl MemoryCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new(512)
    }
}

impl MemoryCache {
    fn lookup(&self, key: &str, max_age: Duration) -> Option<Vec<u8>> {
        let entries = self.entries.lock().ok()?;
        let (at, body) = entries.get(key)?;
        (at.elapsed() <= max_age).then(|| body.clone())
    }

    fn store(&self, key: &str, body: Vec<u8>) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        if entries.len() >= self.max_entries && !entries.contains_key(key) {
            let oldest = entries
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(k, _)| k.clone());
            if let Some(oldest) = oldest {
                entries.remove(&oldest);
            }
        }
        entries.insert(key.to_owned(), (Instant::now(), body));
    }
}

impl HttpCache for MemoryCache {
    fn get<'a>(&'a self, key: &'a str, max_age: Duration) -> CacheFuture<'a, Option<Vec<u8>>> {
        let hit = self.lookup(key, max_age);
        Box::pin(async move { hit })
    }

    fn put<'a>(&'a self, key: &'a str, body: Vec<u8>) -> CacheFuture<'a, ()> {
        self.store(key, body);
        Box::pin(async {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expires_and_evicts() {
        let cache = MemoryCache::new(2);
        cache.store("a", b"1".to_vec());
        assert_eq!(
            cache.lookup("a", Duration::from_secs(60)),
            Some(b"1".to_vec())
        );
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(
            cache.lookup("a", Duration::from_millis(1)),
            None,
            "older than max_age"
        );
        cache.store("b", b"2".to_vec());
        cache.store("c", b"3".to_vec());
        assert_eq!(
            cache.lookup("a", Duration::from_secs(60)),
            None,
            "oldest evicted"
        );
        assert!(cache.lookup("c", Duration::from_secs(60)).is_some());
    }
}
