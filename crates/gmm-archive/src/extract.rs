// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{self, Read, Write};

use gmm_fsguard::{Guard, RelPath, ScopedPath};

use crate::error::ArchiveError;
use crate::format::{Format, detect_format};
use crate::helper::Helper;
use crate::limits::Limits;
use crate::{rar_fmt, sevenz_fmt, zip_fmt};

/// One file or folder entry from the archive, as written. Folders that only exist because a
/// file sits inside them are created on disk but not listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedEntry {
    pub path: RelPath,
    pub is_dir: bool,
    /// Actual bytes written (0 for folders).
    pub size: u64,
}

/// What [`extract`] produced. Paths are relative to `dest`.
#[derive(Debug, Clone)]
pub struct ExtractReport {
    pub format: Format,
    pub dest: ScopedPath,
    /// Sorted by path.
    pub entries: Vec<ExtractedEntry>,
    pub total_bytes: u64,
}

impl ExtractReport {
    pub fn files(&self) -> impl Iterator<Item = &ExtractedEntry> {
        self.entries.iter().filter(|e| !e.is_dir)
    }
}

/// Extracts `archive` into `dest`, which must not exist yet and must be in a read-write root.
/// RAR archives go through the helper next to the running program
/// ([`Helper::beside_current_exe`]).
///
/// On error, `dest` is removed again, so a rejected archive leaves nothing behind.
pub fn extract(
    guard: &Guard,
    archive: &ScopedPath,
    dest: &ScopedPath,
    limits: &Limits,
) -> Result<ExtractReport, ArchiveError> {
    extract_with(guard, archive, dest, limits, None)
}

/// [`extract`] with an explicit RAR helper (`None`: the one next to the running program).
pub fn extract_with(
    guard: &Guard,
    archive: &ScopedPath,
    dest: &ScopedPath,
    limits: &Limits,
    helper: Option<&Helper>,
) -> Result<ExtractReport, ArchiveError> {
    let format = {
        let mut file = guard.open_read(archive)?;
        let mut header = [0u8; 8];
        let n =
            read_up_to(&mut file, &mut header).map_err(|e| ArchiveError::Corrupt(e.to_string()))?;
        detect_format(&header[..n]).ok_or(ArchiveError::UnknownFormat)?
    };
    // Found before anything is created, so a missing helper leaves nothing behind.
    let rar_helper = match (format, helper) {
        (Format::Rar, Some(h)) => Some(h.clone()),
        (Format::Rar, None) => Some(Helper::beside_current_exe()?),
        _ => None,
    };
    if guard.exists(dest)? {
        return Err(ArchiveError::DestinationExists);
    }
    guard.create_dir_all(dest)?;

    let result = match (&rar_helper, format) {
        (Some(h), _) => rar_fmt::extract(guard, archive, dest, limits, h),
        (None, Format::Zip) => zip_fmt::extract(guard, archive, dest, limits),
        (None, _) => sevenz_fmt::extract(guard, archive, dest, limits),
    };
    match result {
        Ok(mut entries) => {
            entries.sort_by_key(|e| e.path.to_string());
            let total_bytes = entries.iter().map(|e| e.size).sum();
            Ok(ExtractReport {
                format,
                dest: dest.clone(),
                entries,
                total_bytes,
            })
        }
        Err(e) => {
            let _ = guard.remove_dir_all(dest);
            Err(e)
        }
    }
}

/// Second pass: writes validated entries, counting real bytes against the limits.
pub(crate) struct Writer<'a> {
    guard: &'a Guard,
    dest: &'a ScopedPath,
    limits: &'a Limits,
    archive_len: u64,
    total: u64,
    entries: Vec<ExtractedEntry>,
}

impl<'a> Writer<'a> {
    pub(crate) fn new(
        guard: &'a Guard,
        dest: &'a ScopedPath,
        limits: &'a Limits,
        archive_len: u64,
    ) -> Self {
        Self {
            guard,
            dest,
            limits,
            archive_len,
            total: 0,
            entries: Vec::new(),
        }
    }

    pub(crate) fn dir(&mut self, rel: &RelPath) -> Result<(), ArchiveError> {
        if rel.is_root() {
            return Ok(());
        }
        self.guard.create_dir_all(&self.dest.join_rel(rel))?;
        self.entries.push(ExtractedEntry {
            path: rel.clone(),
            is_dir: true,
            size: 0,
        });
        Ok(())
    }

    /// `compressed` is the entry's compressed size when the format knows it (zip), which
    /// allows a per-entry ratio check on top of the whole-archive one.
    pub(crate) fn file(
        &mut self,
        rel: &RelPath,
        reader: &mut dyn Read,
        compressed: Option<u64>,
    ) -> Result<(), ArchiveError> {
        let target = self.dest.join_rel(rel);
        if let Some(parent) = target.parent() {
            self.guard.create_dir_all(&parent)?;
        }
        let mut out = self.guard.create_new(&target)?;
        let name = rel.to_string();
        let mut buf = vec![0u8; 64 * 1024];
        let mut written: u64 = 0;
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(ArchiveError::Corrupt(format!("{name:?}: {e}"))),
            };
            let n64 = n as u64;
            written += n64;
            self.total += n64;
            if self.total > self.limits.max_total_bytes {
                return Err(ArchiveError::TooLarge {
                    limit: self.limits.max_total_bytes,
                });
            }
            let entry_bomb = compressed.is_some_and(|c| written > self.limits.ratio_cap(c));
            if entry_bomb || self.total > self.limits.ratio_cap(self.archive_len) {
                return Err(ArchiveError::Bomb { name });
            }
            out.write_all(&buf[..n])
                .map_err(|source| ArchiveError::Write {
                    name: name.clone(),
                    source,
                })?;
        }
        self.entries.push(ExtractedEntry {
            path: rel.clone(),
            is_dir: false,
            size: written,
        });
        Ok(())
    }

    pub(crate) fn into_entries(self) -> Vec<ExtractedEntry> {
        self.entries
    }
}

fn read_up_to(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}
