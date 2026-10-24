// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gmm_catalog::ModFile;
use gmm_fsguard::{Access, Guard, RootId, ScopedPath};
use sha2::{Digest, Sha256};

use crate::error::CoreError;

static UNIQUE: AtomicU64 = AtomicU64::new(0);

/// Temporary folders (extraction, staging) older than this are leftovers from a crash or an
/// interrupted run and get removed.
pub(crate) const STALE_TEMP: Duration = Duration::from_secs(60 * 60);

/// GMM's own folders (docs/02-architecture.md, "App directories").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    /// Config and database: `%APPDATA%\GMM`.
    pub data: PathBuf,
    /// Downloads and other caches: `%LOCALAPPDATA%\GMM\cache`.
    pub cache: PathBuf,
}

impl AppDirs {
    /// The standard locations for the current user (XDG equivalents outside Windows).
    pub fn system() -> Result<Self, CoreError> {
        Ok(Self {
            data: dirs::data_dir().ok_or(CoreError::NoAppDirs)?.join("GMM"),
            cache: dirs::data_local_dir()
                .ok_or(CoreError::NoAppDirs)?
                .join("GMM")
                .join("cache"),
        })
    }
}

/// The guard and roots one operation works with: GMM's cache (always) and an XXMI `Mods/`
/// folder (when installing).
#[derive(Debug)]
pub struct Workspace {
    guard: Guard,
    cache: RootId,
}

impl Workspace {
    /// Registers the cache folder (creating it on first run) with `downloads/` and
    /// `extract/` inside.
    pub fn open(cache_dir: &Path) -> Result<Self, CoreError> {
        let mut guard = Guard::new();
        let cache = guard.ensure_root("cache", cache_dir, Access::ReadWrite)?;
        guard.create_dir_all(&guard.path(cache, "downloads")?)?;
        let extract = guard.path(cache, "extract")?;
        guard.create_dir_all(&extract)?;
        remove_stale(&guard, &extract, STALE_TEMP);
        Ok(Self { guard, cache })
    }

    pub fn guard(&self) -> &Guard {
        &self.guard
    }

    pub fn cache_root(&self) -> RootId {
        self.cache
    }

    /// Registers an XXMI `Mods/` folder as a read-write root, creating it if the importer
    /// folder exists but `Mods/` doesn't yet.
    pub fn add_mods_root(&mut self, mods_dir: &Path) -> Result<RootId, CoreError> {
        Ok(self
            .guard
            .ensure_root("mods", mods_dir, Access::ReadWrite)?)
    }

    /// Where a GameBanana file is cached: `downloads/<file id>-<file name>`, or
    /// `downloads/<file id>.bin` when GameBanana's name isn't a safe file name.
    pub(crate) fn download_path(&self, file: &ModFile) -> Result<ScopedPath, CoreError> {
        Ok(self
            .guard
            .path(self.cache, &format!("downloads/{}-{}", file.id, file.name))
            .or_else(|_| {
                self.guard
                    .path(self.cache, &format!("downloads/{}.bin", file.id))
            })?)
    }

    /// A fresh, not yet existing folder under `extract/`.
    pub(crate) fn new_extract_dir(&self) -> Result<ScopedPath, CoreError> {
        Ok(self
            .guard
            .path(self.cache, &format!("extract/{}", unique_id()))?)
    }
}

/// Unique enough for temporary folder names: process id, time, counter.
pub(crate) fn unique_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!(
        "{}-{nanos:x}-{}",
        std::process::id(),
        UNIQUE.fetch_add(1, Ordering::Relaxed)
    )
}

/// SHA-256 (lowercase hex) and length of a file.
pub(crate) fn hash_file(guard: &Guard, path: &ScopedPath) -> Result<(String, u64), CoreError> {
    let mut file = guard.open_read(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut total = 0u64;
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(CoreError::Io(format!("reading the cached archive: {e}"))),
        };
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok((hex::encode(hasher.finalize()), total))
}

/// Removes entries of `dir` not modified for `age`. Only for GMM's own temporary folders.
/// Best effort.
pub(crate) fn remove_stale(guard: &Guard, dir: &ScopedPath, age: Duration) {
    let Ok(entries) = guard.read_dir(dir) else {
        return;
    };
    for entry in entries {
        let stale = guard
            .metadata(&entry.path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|elapsed| elapsed > age);
        if stale {
            let _ = if entry.is_dir {
                guard.remove_dir_all(&entry.path)
            } else {
                guard.remove_file(&entry.path)
            };
        }
    }
}
