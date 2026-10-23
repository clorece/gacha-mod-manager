// SPDX-License-Identifier: MIT OR Apache-2.0

//! First pass: validate every entry's metadata before a single byte is written.

use std::collections::HashMap;

use gmm_fsguard::RelPath;

use crate::error::ArchiveError;
use crate::limits::Limits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    File,
    Dir,
    Symlink,
    Special,
}

/// Accumulates checks across all entries of one archive.
pub(crate) struct Planner<'a> {
    limits: &'a Limits,
    count: usize,
    declared_total: u64,
    /// Lower-cased path -> is_dir. Windows and macOS file systems are case-insensitive, so
    /// `Mod/a.ini` and `MOD/A.INI` would silently overwrite each other.
    seen: HashMap<String, bool>,
}

impl<'a> Planner<'a> {
    pub(crate) fn new(limits: &'a Limits) -> Self {
        Self {
            limits,
            count: 0,
            declared_total: 0,
            seen: HashMap::new(),
        }
    }

    /// Checks one entry and returns its validated relative path. `declared_size` comes from
    /// archive metadata and may be a lie; actual bytes are counted again while writing.
    pub(crate) fn check(
        &mut self,
        raw_name: &str,
        kind: EntryKind,
        declared_size: u64,
    ) -> Result<RelPath, ArchiveError> {
        self.count += 1;
        if self.count > self.limits.max_entries {
            return Err(ArchiveError::TooManyEntries {
                limit: self.limits.max_entries,
            });
        }
        match kind {
            EntryKind::Symlink => return Err(ArchiveError::Symlink(raw_name.to_owned())),
            EntryKind::Special => return Err(ArchiveError::SpecialFile(raw_name.to_owned())),
            EntryKind::File | EntryKind::Dir => {}
        }
        let rel = RelPath::parse(raw_name).map_err(|reason| ArchiveError::UnsafePath {
            name: raw_name.to_owned(),
            reason,
        })?;
        if rel.is_root() {
            // A bare "/" or "./" directory entry is harmless; a file with no name is not.
            return match kind {
                EntryKind::Dir => Ok(rel),
                _ => Err(ArchiveError::EmptyName),
            };
        }
        if rel.depth() > self.limits.max_depth {
            return Err(ArchiveError::TooDeep {
                name: raw_name.to_owned(),
                limit: self.limits.max_depth,
            });
        }
        if rel.to_string().chars().count() > self.limits.max_path_len {
            return Err(ArchiveError::PathTooLong {
                name: raw_name.to_owned(),
                limit: self.limits.max_path_len,
            });
        }
        self.declared_total = self.declared_total.saturating_add(declared_size);
        if self.declared_total > self.limits.max_total_bytes {
            return Err(ArchiveError::TooLarge {
                limit: self.limits.max_total_bytes,
            });
        }
        self.register(&rel, kind == EntryKind::Dir, raw_name)?;
        Ok(rel)
    }

    fn register(
        &mut self,
        rel: &RelPath,
        is_dir: bool,
        raw_name: &str,
    ) -> Result<(), ArchiveError> {
        let collision = || ArchiveError::Collision(raw_name.to_owned());
        let lower: Vec<String> = rel.components().iter().map(|c| c.to_lowercase()).collect();
        // Every ancestor must be (or become) a directory.
        for i in 1..lower.len() {
            let key = lower[..i].join("/");
            match self.seen.get(&key) {
                Some(false) => return Err(collision()),
                Some(true) => {}
                None => {
                    self.seen.insert(key, true);
                }
            }
        }
        let key = lower.join("/");
        match self.seen.get(&key) {
            // The same directory listed twice is common and harmless.
            Some(true) if is_dir => Ok(()),
            Some(_) => Err(collision()),
            None => {
                self.seen.insert(key, is_dir);
                Ok(())
            }
        }
    }
}
