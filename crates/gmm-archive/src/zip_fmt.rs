// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::BufReader;

use gmm_fsguard::{Guard, RelPath, ScopedPath};
use zip::ZipArchive;

use crate::error::ArchiveError;
use crate::extract::{ExtractedEntry, Writer};
use crate::limits::Limits;
use crate::plan::{EntryKind, Planner};

const S_IFMT: u32 = 0o170_000;
const S_IFREG: u32 = 0o100_000;
const S_IFDIR: u32 = 0o040_000;
const S_IFLNK: u32 = 0o120_000;
/// Windows FILE_ATTRIBUTE_REPARSE_POINT (symlinks, junctions) in the DOS attribute bits.
const DOS_REPARSE_POINT: u32 = 0x400;

pub(crate) fn extract(
    guard: &Guard,
    archive: &ScopedPath,
    dest: &ScopedPath,
    limits: &Limits,
) -> Result<Vec<ExtractedEntry>, ArchiveError> {
    let file = guard.open_read(archive)?;
    let archive_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut zip = ZipArchive::new(BufReader::new(file)).map_err(corrupt)?;

    // Pass 1: metadata only.
    let mut planner = Planner::new(limits);
    let mut planned: Vec<(usize, RelPath, EntryKind, u64)> = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip.by_index_data(index).map_err(corrupt)?;
        let name = entry.name().map_err(corrupt)?.into_owned();
        if entry.encrypted() {
            return Err(ArchiveError::Encrypted);
        }
        let kind = if entry.is_symlink() || entry.external_attributes() & DOS_REPARSE_POINT != 0 {
            EntryKind::Symlink
        } else {
            match entry.unix_mode().map(|m| m & S_IFMT) {
                Some(S_IFLNK) => EntryKind::Symlink,
                Some(S_IFDIR) => EntryKind::Dir,
                None | Some(0) | Some(S_IFREG) if entry.is_dir() || name.ends_with('\\') => {
                    EntryKind::Dir
                }
                None | Some(0) | Some(S_IFREG) => EntryKind::File,
                Some(_) => EntryKind::Special,
            }
        };
        let rel = planner.check(&name, kind, entry.size())?;
        planned.push((index, rel, kind, entry.compressed_size()));
    }

    // Pass 2: write.
    let mut writer = Writer::new(guard, dest, limits, archive_len);
    for (index, rel, kind, compressed) in planned {
        if kind == EntryKind::Dir {
            writer.dir(&rel)?;
        } else {
            let mut entry = zip.by_index(index).map_err(corrupt)?;
            writer.file(&rel, &mut entry, Some(compressed))?;
        }
    }
    Ok(writer.into_entries())
}

fn corrupt(e: zip::result::ZipError) -> ArchiveError {
    ArchiveError::Corrupt(e.to_string())
}
