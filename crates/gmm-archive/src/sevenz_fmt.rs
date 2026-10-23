// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::io::BufReader;

use gmm_fsguard::{Guard, RelPath, ScopedPath};
use sevenz_rust2::{ArchiveEntry, ArchiveReader, Password};

use crate::error::ArchiveError;
use crate::extract::{ExtractedEntry, Writer};
use crate::limits::Limits;
use crate::plan::{EntryKind, Planner};

/// Windows FILE_ATTRIBUTE_REPARSE_POINT (symlinks, junctions).
const REPARSE_POINT: u32 = 0x400;
/// 7-Zip sets this bit when the high 16 bits hold a Unix mode.
const UNIX_EXTENSION: u32 = 0x8000;
const S_IFMT: u32 = 0o170_000;
const S_IFLNK: u32 = 0o120_000;
const S_IFREG: u32 = 0o100_000;
const S_IFDIR: u32 = 0o040_000;

pub(crate) fn extract(
    guard: &Guard,
    archive: &ScopedPath,
    dest: &ScopedPath,
    limits: &Limits,
) -> Result<Vec<ExtractedEntry>, ArchiveError> {
    let file = guard.open_read(archive)?;
    let archive_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut reader =
        ArchiveReader::new(BufReader::new(file), Password::empty()).map_err(map_err)?;

    // Pass 1: metadata only. 7z entries are matched by name in pass 2; identical names are
    // already rejected as collisions here, so the map is unambiguous.
    let mut planner = Planner::new(limits);
    let mut planned: HashMap<String, (RelPath, EntryKind)> = HashMap::new();
    for entry in &reader.archive().files {
        if entry.is_anti_item() {
            continue;
        }
        let kind = kind_of(entry);
        let rel = planner.check(entry.name(), kind, entry.size())?;
        planned.insert(entry.name().to_owned(), (rel, kind));
    }

    // Pass 2: write. Solid archives must be decoded in order, hence the callback.
    let mut writer = Writer::new(guard, dest, limits, archive_len);
    let mut failure = None;
    let result = reader.for_each_entries(|entry, data| {
        if entry.is_anti_item() {
            return Ok(true);
        }
        let outcome = match planned.get(entry.name()) {
            None => Err(ArchiveError::Corrupt(format!(
                "entry {:?} missing from the archive index",
                entry.name()
            ))),
            Some((rel, EntryKind::Dir)) => writer.dir(rel),
            Some((rel, _)) => writer.file(rel, data, None),
        };
        match outcome {
            Ok(()) => Ok(true),
            Err(e) => {
                failure = Some(e);
                Ok(false)
            }
        }
    });
    if let Some(e) = failure {
        return Err(e);
    }
    result.map_err(map_err)?;
    Ok(writer.into_entries())
}

fn kind_of(entry: &ArchiveEntry) -> EntryKind {
    let attrs = if entry.has_windows_attributes {
        entry.windows_attributes()
    } else {
        0
    };
    if attrs & REPARSE_POINT != 0 {
        return EntryKind::Symlink;
    }
    if attrs & UNIX_EXTENSION != 0 {
        match (attrs >> 16) & S_IFMT {
            S_IFLNK => return EntryKind::Symlink,
            0 | S_IFREG | S_IFDIR => {}
            _ => return EntryKind::Special,
        }
    }
    if entry.is_directory() {
        EntryKind::Dir
    } else {
        EntryKind::File
    }
}

fn map_err(e: sevenz_rust2::Error) -> ArchiveError {
    match e {
        sevenz_rust2::Error::PasswordRequired | sevenz_rust2::Error::MaybeBadPassword(_) => {
            ArchiveError::Encrypted
        }
        other => ArchiveError::Corrupt(other.to_string()),
    }
}
