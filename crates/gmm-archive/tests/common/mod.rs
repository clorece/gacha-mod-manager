// SPDX-License-Identifier: MIT OR Apache-2.0

//! Builders for test archives. Malicious names are patched into the raw bytes after writing,
//! so the tests don't depend on the writer libraries accepting them.

#![allow(dead_code, clippy::disallowed_methods, clippy::unwrap_used)]

use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;

use gmm_archive::{ArchiveError, ExtractReport, Limits, extract};
use gmm_fsguard::{Access, Guard, RootId, ScopedPath};
use sevenz_rust2::{ArchiveEntry, ArchiveWriter};
use tempfile::TempDir;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

pub enum E<'a> {
    File(&'a str, &'a [u8]),
    Dir(&'a str),
    Link(&'a str, &'a str),
}

pub fn zip_bytes(entries: &[E], method: CompressionMethod) -> Vec<u8> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(method);
    for e in entries {
        match e {
            E::File(name, data) => {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
            E::Dir(name) => w.add_directory(*name, opts).unwrap(),
            E::Link(name, target) => w.add_symlink(*name, *target, opts).unwrap(),
        }
    }
    w.finish().unwrap().into_inner()
}

pub fn zip_stored(entries: &[E]) -> Vec<u8> {
    zip_bytes(entries, CompressionMethod::Stored)
}

pub fn sevenz_bytes(entries: &[E]) -> Vec<u8> {
    let mut w = ArchiveWriter::new(Cursor::new(Vec::new())).unwrap();
    for e in entries {
        match e {
            E::File(name, data) => {
                w.push_archive_entry(ArchiveEntry::new_file(name), Some(*data))
                    .unwrap();
            }
            E::Dir(name) => {
                w.push_archive_entry::<&[u8]>(ArchiveEntry::new_directory(name), None)
                    .unwrap();
            }
            E::Link(name, target) => {
                // 7-Zip stores Unix symlinks as files whose content is the target, with the
                // Unix mode in the high 16 bits of the attributes.
                let mut entry = ArchiveEntry::new_file(name);
                entry.has_windows_attributes = true;
                entry.windows_attributes = 0x8000 | (0o120_777 << 16);
                w.push_archive_entry(entry, Some(target.as_bytes()))
                    .unwrap();
            }
        }
    }
    w.finish().unwrap().into_inner()
}

/// Replaces every occurrence of `from` with `to` (same length) in raw archive bytes.
pub fn patch(mut bytes: Vec<u8>, from: &str, to: &str) -> Vec<u8> {
    assert_eq!(from.len(), to.len(), "patch must keep the length");
    let (from, to) = (from.as_bytes(), to.as_bytes());
    let mut found = false;
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + from.len()].copy_from_slice(to);
            found = true;
            i += from.len();
        } else {
            i += 1;
        }
    }
    assert!(found, "patch target not found");
    bytes
}

/// Sets the "encrypted" flag on every local and central zip header.
pub fn mark_zip_encrypted(mut bytes: Vec<u8>) -> Vec<u8> {
    for (sig, flag_offset) in [(b"PK\x03\x04", 6), (b"PK\x01\x02", 8)] {
        let mut i = 0;
        while i + 4 <= bytes.len() {
            if &bytes[i..i + 4] == sig {
                bytes[i + flag_offset] |= 1;
            }
            i += 1;
        }
    }
    bytes
}

/// A guard with one read-write root (the app cache) and a sibling folder outside every root,
/// used to prove nothing escapes.
pub struct Env {
    _dir: TempDir,
    pub guard: Guard,
    pub cache: RootId,
    pub outside: PathBuf,
    counter: std::cell::Cell<u32>,
}

impl Env {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let outside = dir.path().join("outside");
        fs::create_dir(&cache).unwrap();
        fs::create_dir(&outside).unwrap();
        let mut guard = Guard::new();
        let cache = guard
            .register_root("cache", &cache, Access::ReadWrite)
            .unwrap();
        Self {
            _dir: dir,
            guard,
            cache,
            outside,
            counter: std::cell::Cell::new(0),
        }
    }

    /// Writes archive bytes and returns (archive path, fresh destination path).
    pub fn stage(&self, bytes: &[u8]) -> (ScopedPath, ScopedPath) {
        let n = self.counter.get();
        self.counter.set(n + 1);
        let incoming = self.guard.path(self.cache, "incoming").unwrap();
        self.guard.create_dir_all(&incoming).unwrap();
        let archive = incoming.join(&format!("a{n}.bin")).unwrap();
        self.guard.write_atomic(&archive, bytes).unwrap();
        let dest = self.guard.path(self.cache, &format!("out/{n}")).unwrap();
        (archive, dest)
    }

    pub fn extract(&self, bytes: &[u8], limits: &Limits) -> Result<ExtractReport, ArchiveError> {
        let (archive, dest) = self.stage(bytes);
        let result = extract(&self.guard, &archive, &dest, limits);
        if result.is_err() {
            assert!(
                !self.guard.exists(&dest).unwrap(),
                "a failed extraction must remove its destination"
            );
        }
        self.assert_outside_untouched();
        result
    }

    pub fn assert_outside_untouched(&self) {
        let leaked: Vec<_> = fs::read_dir(&self.outside).unwrap().collect();
        assert!(leaked.is_empty(), "files escaped the root: {leaked:?}");
    }
}
