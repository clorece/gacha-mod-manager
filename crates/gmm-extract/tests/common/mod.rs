// SPDX-License-Identifier: MIT OR Apache-2.0

//! Builds RAR test archives byte by byte (no RAR writer is available as a library). Files are
//! stored (no compression), which is enough to exercise names, types, sizes, links and
//! encryption flags; the formats follow RARLAB's published technical notes for RAR 5.0 and
//! RAR 2.9 (RAR4).

#![allow(dead_code, clippy::disallowed_methods, clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use gmm_archive::{ArchiveError, ExtractReport, Helper, Limits, extract_with};
use gmm_fsguard::{Access, Guard, RootId, ScopedPath};
use tempfile::TempDir;

pub fn helper() -> Helper {
    Helper::at(env!("CARGO_BIN_EXE_gmm-extract"))
}

// ---- CRC32 (IEEE) ----

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

// ---- RAR5 ----

/// One RAR5 file header.
#[derive(Clone)]
pub struct R5 {
    pub name: String,
    pub data: Vec<u8>,
    pub dir: bool,
    /// Unpacked size written in the header (defaults to the data length).
    pub declared: Option<u64>,
    /// Host OS: 0 Windows, 1 Unix.
    pub host: u64,
    /// File attributes (Windows attributes or a Unix mode).
    pub attrs: u64,
    /// Extra records, already encoded (size, type, data).
    pub extra: Vec<u8>,
}

impl R5 {
    pub fn file(name: &str, data: &[u8]) -> Self {
        Self {
            name: name.into(),
            data: data.to_vec(),
            dir: false,
            declared: None,
            host: 0,
            attrs: 0x20,
            extra: Vec::new(),
        }
    }

    pub fn dir(name: &str) -> Self {
        Self {
            dir: true,
            attrs: 0x10,
            ..Self::file(name, b"")
        }
    }

    /// A Unix symlink (file system redirection record, type 1).
    pub fn symlink(name: &str, target: &str) -> Self {
        Self {
            extra: redirection(1, target),
            ..Self::file(name, b"")
        }
    }

    /// A hard link (redirection type 4).
    pub fn hardlink(name: &str, target: &str) -> Self {
        Self {
            extra: redirection(4, target),
            ..Self::file(name, b"")
        }
    }

    /// A file with a Unix mode, e.g. `0o010644` for a FIFO.
    pub fn unix(name: &str, mode: u64) -> Self {
        Self {
            host: 1,
            attrs: mode,
            ..Self::file(name, b"")
        }
    }

    /// A file whose data is marked as encrypted (file encryption record).
    pub fn encrypted(name: &str, data: &[u8]) -> Self {
        let mut rec = Vec::new();
        vint(&mut rec, 0); // version: AES-256
        vint(&mut rec, 0); // flags: no password check, no tweaked checksums
        rec.push(15); // KDF count
        rec.extend_from_slice(&[7u8; 16]); // salt
        rec.extend_from_slice(&[9u8; 16]); // IV
        Self {
            extra: record(0x01, &rec),
            ..Self::file(name, data)
        }
    }
}

pub fn vint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn record(kind: u64, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    vint(&mut body, kind);
    body.extend_from_slice(data);
    let mut rec = Vec::new();
    vint(&mut rec, body.len() as u64);
    rec.extend_from_slice(&body);
    rec
}

fn redirection(kind: u64, target: &str) -> Vec<u8> {
    let mut rec = Vec::new();
    vint(&mut rec, kind);
    vint(&mut rec, 0); // flags
    vint(&mut rec, target.len() as u64);
    rec.extend_from_slice(target.as_bytes());
    record(0x05, &rec)
}

/// A RAR5 block: CRC32, header size, then `body` (type, flags, fields, extra area).
fn block5(body: &[u8]) -> Vec<u8> {
    let mut sized = Vec::new();
    vint(&mut sized, body.len() as u64);
    sized.extend_from_slice(body);
    let mut out = crc32(&sized).to_le_bytes().to_vec();
    out.extend_from_slice(&sized);
    out
}

pub const RAR5_SIGNATURE: &[u8] = b"Rar!\x1A\x07\x01\x00";

/// A complete RAR5 archive. `encrypted_headers` adds an archive encryption header first.
pub fn rar5(entries: &[R5], encrypted_headers: bool) -> Vec<u8> {
    let mut out = RAR5_SIGNATURE.to_vec();
    if encrypted_headers {
        let mut body = Vec::new();
        vint(&mut body, 4); // type: archive encryption
        vint(&mut body, 0); // header flags
        vint(&mut body, 0); // encryption version
        vint(&mut body, 0); // encryption flags
        body.push(15); // KDF count
        body.extend_from_slice(&[3u8; 16]); // salt
        out.extend_from_slice(&block5(&body));
    }
    // Main archive header: no flags.
    let mut main = Vec::new();
    vint(&mut main, 1);
    vint(&mut main, 0);
    vint(&mut main, 0);
    out.extend_from_slice(&block5(&main));

    for e in entries {
        let mut body = Vec::new();
        vint(&mut body, 2); // type: file
        let mut flags = 0x0002; // data area present
        if !e.extra.is_empty() {
            flags |= 0x0001;
        }
        vint(&mut body, flags);
        if !e.extra.is_empty() {
            vint(&mut body, e.extra.len() as u64);
        }
        vint(&mut body, e.data.len() as u64); // data (packed) size
        let file_flags = if e.dir { 0x1 } else { 0 } | 0x4; // directory, CRC32 present
        vint(&mut body, file_flags);
        vint(&mut body, e.declared.unwrap_or(e.data.len() as u64));
        vint(&mut body, e.attrs);
        body.extend_from_slice(&crc32(&e.data).to_le_bytes());
        vint(&mut body, 0); // compression: RAR 5.0, stored
        vint(&mut body, e.host);
        vint(&mut body, e.name.len() as u64);
        body.extend_from_slice(e.name.as_bytes());
        body.extend_from_slice(&e.extra);
        out.extend_from_slice(&block5(&body));
        out.extend_from_slice(&e.data);
    }

    let mut end = Vec::new();
    vint(&mut end, 5);
    vint(&mut end, 0);
    vint(&mut end, 0);
    out.extend_from_slice(&block5(&end));
    out
}

// ---- RAR4 (RAR 2.9 format) ----

pub const RAR4_SIGNATURE: &[u8] = b"Rar!\x1A\x07\x00";

/// One RAR4 header: CRC16 (low half of the CRC32 of everything after it), type, flags, size.
fn block4(kind: u8, flags: u16, fields: &[u8]) -> Vec<u8> {
    let size = (7 + fields.len()) as u16;
    let mut rest = vec![kind];
    rest.extend_from_slice(&flags.to_le_bytes());
    rest.extend_from_slice(&size.to_le_bytes());
    rest.extend_from_slice(fields);
    let mut out = ((crc32(&rest) & 0xFFFF) as u16).to_le_bytes().to_vec();
    out.extend_from_slice(&rest);
    out
}

/// A RAR4 archive of stored files (`dir` entries end with `/` in the list and become folders).
pub fn rar4(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = RAR4_SIGNATURE.to_vec();
    out.extend_from_slice(&block4(0x73, 0, &[0u8; 6]));
    for (name, data) in entries {
        let dir = name.ends_with('/');
        let name = name.trim_end_matches('/').replace('/', "\\");
        let mut fields = Vec::new();
        fields.extend_from_slice(&(data.len() as u32).to_le_bytes()); // packed size
        fields.extend_from_slice(&(data.len() as u32).to_le_bytes()); // unpacked size
        fields.push(2); // host: Windows
        fields.extend_from_slice(&crc32(data).to_le_bytes());
        fields.extend_from_slice(&0x5A21_0000u32.to_le_bytes()); // DOS time
        fields.push(29); // version needed
        fields.push(0x30); // method: store
        fields.extend_from_slice(&(name.len() as u16).to_le_bytes());
        fields.extend_from_slice(&(if dir { 0x10u32 } else { 0x20 }).to_le_bytes());
        fields.extend_from_slice(name.as_bytes());
        // 0x8000: data follows; 0x00E0: the "dictionary" bits that mark a directory.
        let flags = 0x8000 | if dir { 0x00E0 } else { 0 };
        out.extend_from_slice(&block4(0x74, flags, &fields));
        out.extend_from_slice(data);
    }
    out.extend_from_slice(&block4(0x7B, 0x4000, &[]));
    out
}

// ---- Environment ----

/// A guard with one read-write root and a sibling folder outside it, to prove nothing escapes.
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

    pub fn stage(&self, bytes: &[u8]) -> (ScopedPath, ScopedPath) {
        let n = self.counter.get();
        self.counter.set(n + 1);
        let incoming = self.guard.path(self.cache, "incoming").unwrap();
        self.guard.create_dir_all(&incoming).unwrap();
        let archive = incoming.join(&format!("a{n}.rar")).unwrap();
        self.guard.write_atomic(&archive, bytes).unwrap();
        let dest = self.guard.path(self.cache, &format!("out/{n}")).unwrap();
        (archive, dest)
    }

    pub fn extract(&self, bytes: &[u8], limits: &Limits) -> Result<ExtractReport, ArchiveError> {
        let (archive, dest) = self.stage(bytes);
        let result = extract_with(&self.guard, &archive, &dest, limits, Some(&helper()));
        if result.is_err() {
            assert!(
                !self.guard.exists(&dest).unwrap(),
                "a failed extraction must remove its destination"
            );
        }
        let leaked: Vec<_> = fs::read_dir(&self.outside).unwrap().collect();
        assert!(leaked.is_empty(), "files escaped the root: {leaked:?}");
        result
    }

    pub fn read(&self, report: &ExtractReport, rel: &str) -> Vec<u8> {
        let p = report.dest.join(rel).unwrap();
        let mut f = self.guard.open_read(&p).unwrap();
        let mut v = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut v).unwrap();
        v
    }
}
