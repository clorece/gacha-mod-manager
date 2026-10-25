// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use gmm_catalog::{CatalogSource, ModFile, ModFiles};
use gmm_db::{Db, NewCachedDownload};
use gmm_fsguard::ScopedPath;
use gmm_net::{Client, DownloadRequest, Progress, Url};

use crate::error::CoreError;
use crate::stop::{StopReason, StopSignal};
use crate::workspace::{Workspace, hash_file};

/// Downloads above this are refused outright, whatever the catalogue says.
pub const MAX_DOWNLOAD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// What happens during [`fetch_archive`], for progress display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchEvent {
    /// The file is already in the download cache.
    UsingCache,
    /// A download starts; `size` is what GameBanana says.
    Downloading {
        size: u64,
    },
    Progress(Progress),
}

/// An archive in the download cache, ready to extract.
#[derive(Debug, Clone)]
pub struct FetchedArchive {
    pub mod_id: u64,
    pub file: ModFile,
    /// In the workspace's cache root.
    pub archive: ScopedPath,
    pub sha256: String,
    pub bytes: u64,
    pub from_cache: bool,
    pub resumed: bool,
}

/// The file to install: `file_id` if given (current or archived files), else the newest
/// current file. Either way it must have passed GameBanana's virus scan (T4).
pub fn choose_file(
    files: &ModFiles,
    mod_id: u64,
    file_id: Option<u64>,
) -> Result<ModFile, CoreError> {
    let file = match file_id {
        Some(id) => files
            .files
            .iter()
            .chain(&files.archived_files)
            .find(|f| f.id == id)
            .ok_or(CoreError::NoSuchFile {
                mod_id,
                file_id: id,
            })?,
        None => files
            .default_choice()
            .ok_or(CoreError::NoCleanFile(mod_id))?,
    };
    if !file.is_clean() {
        return Err(CoreError::NotClean {
            file_id: file.id,
            status: file
                .av_result
                .clone()
                .unwrap_or_else(|| "pending".to_owned()),
        });
    }
    Ok(file.clone())
}

/// One download per GameBanana file at a time, across the process: a second fetch of the
/// same file (the install confirmation while the queue downloads it, say) waits for the
/// first, then finds the file in the cache or resumes its partial download.
fn file_lock(file_id: u64) -> Arc<tokio::sync::Mutex<()>> {
    type Locks = Mutex<HashMap<u64, Weak<tokio::sync::Mutex<()>>>>;
    static LOCKS: OnceLock<Locks> = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(Locks::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    locks.retain(|_, held| held.strong_count() > 0);
    if let Some(lock) = locks.get(&file_id).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    locks.insert(file_id, Arc::downgrade(&lock));
    lock
}

/// Puts a GameBanana file into the download cache (or finds it there) and records it in the
/// database. The file list is fetched fresh, since it's read right before a download. A pause
/// or cancel on `stop` ends the download early ([`CoreError::Stopped`]); the partial file is
/// kept, so fetching again resumes it.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_archive<C: CatalogSource>(
    catalog: &C,
    net: &Client,
    db: &Db,
    ws: &Workspace,
    mod_id: u64,
    file_id: Option<u64>,
    stop: &StopSignal,
    mut on_event: impl FnMut(FetchEvent),
) -> Result<FetchedArchive, CoreError> {
    stop.check()?;
    let files = catalog.mod_files(mod_id).await?;
    let file = choose_file(&files, mod_id, file_id)?;
    let lock = file_lock(file.id);
    let _one_at_a_time = lock.lock().await;
    stop.check()?;
    let guard = ws.guard();
    let dest = ws.download_path(&file)?;
    let path_rel = dest.rel().to_string();
    let recorded = db
        .cached_download_for_file(file.id)
        .await?
        .filter(|d| d.path_rel == path_rel);

    let (sha256, bytes, from_cache, resumed) = if guard.exists(&dest)? {
        on_event(FetchEvent::UsingCache);
        let len = guard.metadata(&dest)?.len();
        match recorded {
            Some(d) if d.size_bytes == len => (d.sha256, len, true, false),
            // Unrecorded (fetched before the database existed) or changed: hash it again.
            _ => {
                let (sha256, len) = hash_file(guard, &dest)?;
                (sha256, len, true, false)
            }
        }
    } else {
        if let Some(stale) = recorded {
            // The file was deleted behind GMM's back; forget it and download again.
            db.remove_download(&stale.sha256).await?;
        }
        on_event(FetchEvent::Downloading { size: file.size });
        let request = DownloadRequest {
            url: Url::parse(&file.download_url)
                .map_err(|_| gmm_net::NetError::Disallowed("invalid download URL".to_owned()))?,
            dest: dest.clone(),
            expected_size: Some(file.size),
            expected_md5: file.md5.clone(),
            max_bytes: MAX_DOWNLOAD_BYTES,
        };
        let outcome = net
            .download(guard, &request, |p| {
                on_event(FetchEvent::Progress(p));
                match stop.requested() {
                    Some(_) => ControlFlow::Break(()),
                    None => ControlFlow::Continue(()),
                }
            })
            .await
            .map_err(|e| match e {
                gmm_net::NetError::Paused => {
                    CoreError::Stopped(stop.requested().unwrap_or(StopReason::Paused))
                }
                other => other.into(),
            })?;
        (outcome.sha256, outcome.bytes, false, outcome.resumed)
    };

    db.record_download(&NewCachedDownload {
        sha256: sha256.clone(),
        gb_mod_id: Some(mod_id),
        gb_file_id: Some(file.id),
        path_rel,
        size_bytes: bytes,
    })
    .await?;
    Ok(FetchedArchive {
        mod_id,
        file,
        archive: dest,
        sha256,
        bytes,
        from_cache,
        resumed,
    })
}
