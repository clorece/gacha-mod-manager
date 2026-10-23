// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Response cache keyed by URL. The SQLite-backed implementation arrives with the database
/// crate; until then [`MemoryCache`] serves the app and the CLI.
pub trait HttpCache: Send + Sync {
    /// A cached body no older than `max_age`.
    fn get(&self, key: &str, max_age: Duration) -> Option<Vec<u8>>;
    fn put(&self, key: &str, body: Vec<u8>);
}

/// Never caches.
#[derive(Debug, Default)]
pub struct NoCache;

impl HttpCache for NoCache {
    fn get(&self, _: &str, _: Duration) -> Option<Vec<u8>> {
        None
    }

    fn put(&self, _: &str, _: Vec<u8>) {}
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

impl HttpCache for MemoryCache {
    fn get(&self, key: &str, max_age: Duration) -> Option<Vec<u8>> {
        let entries = self.entries.lock().ok()?;
        let (at, body) = entries.get(key)?;
        (at.elapsed() <= max_age).then(|| body.clone())
    }

    fn put(&self, key: &str, body: Vec<u8>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expires_and_evicts() {
        let cache = MemoryCache::new(2);
        cache.put("a", b"1".to_vec());
        assert_eq!(cache.get("a", Duration::from_secs(60)), Some(b"1".to_vec()));
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(
            cache.get("a", Duration::from_millis(1)),
            None,
            "older than max_age"
        );
        cache.put("b", b"2".to_vec());
        cache.put("c", b"3".to_vec());
        assert_eq!(
            cache.get("a", Duration::from_secs(60)),
            None,
            "oldest evicted"
        );
        assert!(cache.get("c", Duration::from_secs(60)).is_some());
    }
}
