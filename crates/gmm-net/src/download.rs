// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{Read, Write};
use std::ops::ControlFlow;

use gmm_fsguard::{Guard, ScopedPath};
use md5::Md5;
use reqwest::header::{CONTENT_RANGE, RANGE};
use sha2::{Digest, Sha256};
use url::Url;

use crate::allowlist::redact;
use crate::client::{Client, request_error};
use crate::error::NetError;

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: Url,
    /// Final file. Must not exist. Data goes to a `.<name>.part` sibling until verified.
    pub dest: ScopedPath,
    /// From the catalog (GameBanana `_nFilesize`). Enforced exactly when given.
    pub expected_size: Option<u64>,
    /// From the catalog (GameBanana `_sMd5Checksum`; often empty).
    pub expected_md5: Option<String>,
    /// Hard ceiling regardless of what the server or catalog claims.
    pub max_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOutcome {
    pub bytes: u64,
    /// Lowercase hex. GMM's own integrity anchor from here on.
    pub sha256: String,
    pub md5: String,
    /// True when an earlier partial download was continued.
    pub resumed: bool,
}

struct Hashes {
    sha256: Sha256,
    md5: Md5,
}

impl Hashes {
    fn new() -> Self {
        Self {
            sha256: Sha256::new(),
            md5: Md5::new(),
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.sha256.update(data);
        self.md5.update(data);
    }
}

impl Client {
    /// Downloads to `req.dest` through the guard.
    ///
    /// `on_progress` is called before the first byte and after every chunk; returning
    /// `ControlFlow::Break(())` pauses: the partial file is kept, and calling `download` again
    /// with the same request resumes it with an HTTP range request. Network errors keep the
    /// partial file too. Size or checksum failures delete it.
    pub async fn download(
        &self,
        guard: &Guard,
        req: &DownloadRequest,
        mut on_progress: impl FnMut(Progress) -> ControlFlow<()>,
    ) -> Result<DownloadOutcome, NetError> {
        self.config.allowlist.check(&req.url)?;
        if guard.exists(&req.dest)? {
            return Err(NetError::DestinationExists);
        }
        let part = part_path(&req.dest)?;
        let limit = req
            .expected_size
            .map_or(req.max_bytes, |size| size.min(req.max_bytes));

        let mut hashes = Hashes::new();
        let mut have = 0u64;
        if guard.exists(&part)? {
            have = hash_existing(guard, &part, &mut hashes)?;
            if have > limit {
                guard.remove_file(&part)?;
                hashes = Hashes::new();
                have = 0;
            }
        }

        self.respect_cooldown().await?;
        self.acquire().await;
        let mut request = self.http.get(req.url.clone());
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        }
        let mut resp = request
            .send()
            .await
            .map_err(|e| request_error(&req.url, e))?;
        let status = resp.status().as_u16();
        if let Some(slow_down) = self.slowdown(status, resp.headers()) {
            // The partial file stays; the next attempt resumes it.
            return Err(slow_down);
        }
        let (mut file, resumed) = match status {
            206 if have > 0 && content_range_start(&resp) == Some(have) => {
                (guard.open_append(&part)?, true)
            }
            200 => {
                // Fresh download, or the server ignored our range: start over.
                if have > 0 {
                    guard.remove_file(&part)?;
                    hashes = Hashes::new();
                    have = 0;
                }
                (guard.create_new(&part)?, false)
            }
            _ => {
                if status == 416 && have > 0 {
                    // Our partial file doesn't match the server's; next attempt starts fresh.
                    guard.remove_file(&part)?;
                }
                return Err(NetError::Status {
                    url: redact(&req.url),
                    status,
                });
            }
        };
        if resp
            .content_length()
            .is_some_and(|len| have.saturating_add(len) > limit)
        {
            drop(file);
            guard.remove_file(&part)?;
            return Err(NetError::FileTooLarge { limit });
        }
        let total = req
            .expected_size
            .or_else(|| resp.content_length().map(|len| len + have));

        if on_progress(Progress {
            downloaded: have,
            total,
        })
        .is_break()
        {
            return Err(NetError::Paused);
        }
        while let Some(chunk) = resp.chunk().await.map_err(|e| request_error(&req.url, e))? {
            have += chunk.len() as u64;
            if have > limit {
                drop(file);
                guard.remove_file(&part)?;
                return Err(NetError::FileTooLarge { limit });
            }
            file.write_all(&chunk).map_err(NetError::Write)?;
            hashes.update(&chunk);
            if on_progress(Progress {
                downloaded: have,
                total,
            })
            .is_break()
            {
                file.flush().map_err(NetError::Write)?;
                return Err(NetError::Paused);
            }
        }
        file.sync_all().map_err(NetError::Write)?;
        drop(file);

        if let Some(expected) = req.expected_size
            && have != expected
        {
            guard.remove_file(&part)?;
            return Err(NetError::SizeMismatch {
                expected,
                actual: have,
            });
        }
        let md5 = hex::encode(hashes.md5.finalize());
        if let Some(expected) = req.expected_md5.as_deref().filter(|s| !s.is_empty())
            && !expected.eq_ignore_ascii_case(&md5)
        {
            guard.remove_file(&part)?;
            return Err(NetError::Md5Mismatch {
                expected: expected.to_owned(),
                actual: md5,
            });
        }
        let sha256 = hex::encode(hashes.sha256.finalize());
        guard.rename(&part, &req.dest)?;
        self.succeeded();
        Ok(DownloadOutcome {
            bytes: have,
            sha256,
            md5,
            resumed,
        })
    }
}

/// `dir/name.zip` -> `dir/.name.zip.part`
pub(crate) fn part_path(dest: &ScopedPath) -> Result<ScopedPath, NetError> {
    let name = dest.file_name().unwrap_or("download");
    let parent = dest.parent().ok_or(NetError::DestinationExists)?;
    Ok(parent
        .join(&format!(".{name}.part"))
        .map_err(gmm_fsguard::FsError::from)?)
}

fn hash_existing(guard: &Guard, part: &ScopedPath, hashes: &mut Hashes) -> Result<u64, NetError> {
    let mut file = guard.open_read(part)?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(NetError::Write)?;
        if n == 0 {
            return Ok(total);
        }
        hashes.update(&buf[..n]);
        total += n as u64;
    }
}

/// Start offset from `Content-Range: bytes <start>-<end>/<total>`.
fn content_range_start(resp: &reqwest::Response) -> Option<u64> {
    let value = resp.headers().get(CONTENT_RANGE)?.to_str().ok()?;
    let range = value.trim().strip_prefix("bytes ")?;
    range.split('-').next()?.trim().parse().ok()
}
