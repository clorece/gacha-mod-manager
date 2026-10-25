// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frames the `gmm-extract` helper writes to its stdout, and GMM reads back. Shared by
//! both sides so they can't drift. Little-endian, length-prefixed:
//!
//! - `E` entry: kind (u8), flags (u8, bit 0 = encrypted), size (u64, `u64::MAX` = unknown),
//!   name length (u32), name (UTF-8).
//! - `D` data for the current file entry: length (u32, at most [`MAX_CHUNK`]), bytes.
//! - `X` end of the archive.
//! - `F` failure: kind (u8), message length (u32, at most [`MAX_MESSAGE`]), message (UTF-8).
//!
//! The helper is treated as compromised-capable: every length is bounded before anything is
//! allocated, and GMM checks names and sizes itself (`plan.rs`, `extract.rs`).

use std::io::{self, Read, Write};

/// Longest entry name accepted (bytes). Names are checked against the much lower path
/// limits afterwards; this only bounds the allocation.
pub const MAX_NAME: usize = 4096;
/// Largest data frame (bytes).
pub const MAX_CHUNK: usize = 1 << 20;
/// Longest failure message (bytes).
pub const MAX_MESSAGE: usize = 1024;

const TAG_ENTRY: u8 = b'E';
const TAG_DATA: u8 = b'D';
const TAG_END: u8 = b'X';
const TAG_FAIL: u8 = b'F';
const UNKNOWN_SIZE: u64 = u64::MAX;

/// What the helper is asked to do; its only argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Report every entry, no data (pass 1).
    List,
    /// Report every entry with its data (pass 2).
    Extract,
}

impl Mode {
    pub fn arg(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Extract => "extract",
        }
    }

    pub fn from_arg(arg: &str) -> Option<Self> {
        match arg {
            "list" => Some(Self::List),
            "extract" => Some(Self::Extract),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    File,
    Dir,
    /// Symbolic or hard link (rejected by GMM).
    Link,
    /// Device, FIFO, socket (rejected by GMM).
    Special,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryHeader {
    pub kind: EntryType,
    pub encrypted: bool,
    /// From the archive's metadata; may be absent or a lie.
    pub size: Option<u64>,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailKind {
    Encrypted,
    Unsupported,
    Corrupt,
    /// An entry name that isn't valid in any encoding libarchive knows.
    BadName,
    /// The helper couldn't lower its own rights, so it refused to read the archive.
    Lockdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Entry(EntryHeader),
    /// Length of the data now waiting in the reader's buffer ([`FrameReader::data`]).
    Data(usize),
    End,
    Fail {
        kind: FailKind,
        message: String,
    },
}

// ---- Writing (the helper) ----

pub fn write_entry<W: Write + ?Sized>(w: &mut W, e: &EntryHeader) -> io::Result<()> {
    let name = e.name.as_bytes();
    if name.len() > MAX_NAME {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "name too long"));
    }
    let kind = match e.kind {
        EntryType::File => 0u8,
        EntryType::Dir => 1,
        EntryType::Link => 2,
        EntryType::Special => 3,
    };
    w.write_all(&[TAG_ENTRY, kind, u8::from(e.encrypted)])?;
    w.write_all(&e.size.unwrap_or(UNKNOWN_SIZE).to_le_bytes())?;
    w.write_all(&len32(name.len())?.to_le_bytes())?;
    w.write_all(name)
}

/// Writes `data` as one or more data frames.
pub fn write_data<W: Write + ?Sized>(w: &mut W, data: &[u8]) -> io::Result<()> {
    for chunk in data.chunks(MAX_CHUNK) {
        w.write_all(&[TAG_DATA])?;
        w.write_all(&len32(chunk.len())?.to_le_bytes())?;
        w.write_all(chunk)?;
    }
    Ok(())
}

pub fn write_end<W: Write + ?Sized>(w: &mut W) -> io::Result<()> {
    w.write_all(&[TAG_END])
}

pub fn write_fail<W: Write + ?Sized>(w: &mut W, kind: FailKind, message: &str) -> io::Result<()> {
    let mut end = message.len().min(MAX_MESSAGE);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    let msg = &message.as_bytes()[..end];
    let kind = match kind {
        FailKind::Encrypted => 1u8,
        FailKind::Unsupported => 2,
        FailKind::Corrupt => 3,
        FailKind::BadName => 4,
        FailKind::Lockdown => 5,
    };
    w.write_all(&[TAG_FAIL, kind])?;
    w.write_all(&len32(msg.len())?.to_le_bytes())?;
    w.write_all(msg)
}

fn len32(n: usize) -> io::Result<u32> {
    u32::try_from(n).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too long"))
}

// ---- Reading (GMM) ----

/// Reads frames, never allocating more than the protocol's bounds.
pub struct FrameReader<R> {
    inner: R,
    buf: Vec<u8>,
}

impl<R: Read> FrameReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Vec::new(),
        }
    }

    /// The bytes of the last [`Frame::Data`].
    pub fn data(&self) -> &[u8] {
        &self.buf
    }

    pub fn next_frame(&mut self) -> io::Result<Frame> {
        let tag = self.u8()?;
        match tag {
            TAG_ENTRY => {
                let kind = match self.u8()? {
                    0 => EntryType::File,
                    1 => EntryType::Dir,
                    2 => EntryType::Link,
                    3 => EntryType::Special,
                    _ => return Err(bad("unknown entry type")),
                };
                let encrypted = match self.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(bad("unknown entry flags")),
                };
                let size = self.u64()?;
                let len = self.len(MAX_NAME)?;
                let name = self.text(len)?;
                Ok(Frame::Entry(EntryHeader {
                    kind,
                    encrypted,
                    size: (size != UNKNOWN_SIZE).then_some(size),
                    name,
                }))
            }
            TAG_DATA => {
                let len = self.len(MAX_CHUNK)?;
                self.fill(len)?;
                Ok(Frame::Data(len))
            }
            TAG_END => Ok(Frame::End),
            TAG_FAIL => {
                let kind = match self.u8()? {
                    1 => FailKind::Encrypted,
                    2 => FailKind::Unsupported,
                    3 => FailKind::Corrupt,
                    4 => FailKind::BadName,
                    5 => FailKind::Lockdown,
                    _ => return Err(bad("unknown failure kind")),
                };
                let len = self.len(MAX_MESSAGE)?;
                let message = String::from_utf8_lossy(&self.take(len)?).into_owned();
                Ok(Frame::Fail { kind, message })
            }
            _ => Err(bad("unknown frame")),
        }
    }

    fn u8(&mut self) -> io::Result<u8> {
        let mut b = [0u8; 1];
        self.inner.read_exact(&mut b)?;
        Ok(b[0])
    }

    fn u64(&mut self) -> io::Result<u64> {
        let mut b = [0u8; 8];
        self.inner.read_exact(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    fn len(&mut self, max: usize) -> io::Result<usize> {
        let mut b = [0u8; 4];
        self.inner.read_exact(&mut b)?;
        let n = usize::try_from(u32::from_le_bytes(b)).map_err(|_| bad("length"))?;
        if n > max {
            return Err(bad("frame longer than allowed"));
        }
        Ok(n)
    }

    fn fill(&mut self, len: usize) -> io::Result<()> {
        self.buf.resize(len, 0);
        self.inner.read_exact(&mut self.buf)
    }

    fn take(&mut self, len: usize) -> io::Result<Vec<u8>> {
        let mut v = vec![0u8; len];
        self.inner.read_exact(&mut v)?;
        Ok(v)
    }

    fn text(&mut self, len: usize) -> io::Result<String> {
        String::from_utf8(self.take(len)?).map_err(|_| bad("entry name is not UTF-8"))
    }
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut out = Vec::new();
        let e = EntryHeader {
            kind: EntryType::File,
            encrypted: false,
            size: Some(5),
            name: "Mod/a.ini".into(),
        };
        write_entry(&mut out, &e).unwrap();
        write_data(&mut out, b"hello").unwrap();
        write_entry(
            &mut out,
            &EntryHeader {
                kind: EntryType::Dir,
                encrypted: true,
                size: None,
                name: "Mod".into(),
            },
        )
        .unwrap();
        write_fail(&mut out, FailKind::Corrupt, "bad block").unwrap();
        write_end(&mut out).unwrap();

        let mut r = FrameReader::new(out.as_slice());
        assert_eq!(r.next_frame().unwrap(), Frame::Entry(e));
        assert_eq!(r.next_frame().unwrap(), Frame::Data(5));
        assert_eq!(r.data(), b"hello");
        let Frame::Entry(dir) = r.next_frame().unwrap() else {
            panic!("expected an entry");
        };
        assert_eq!(
            (dir.kind, dir.encrypted, dir.size),
            (EntryType::Dir, true, None)
        );
        assert_eq!(
            r.next_frame().unwrap(),
            Frame::Fail {
                kind: FailKind::Corrupt,
                message: "bad block".into()
            }
        );
        assert_eq!(r.next_frame().unwrap(), Frame::End);
    }

    #[test]
    fn oversized_frames_are_refused_before_allocating() {
        let mut out = vec![TAG_DATA];
        out.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(FrameReader::new(out.as_slice()).next_frame().is_err());

        let mut out = vec![TAG_ENTRY, 0, 0];
        out.extend_from_slice(&0u64.to_le_bytes());
        out.extend_from_slice(&(MAX_NAME as u32 + 1).to_le_bytes());
        assert!(FrameReader::new(out.as_slice()).next_frame().is_err());
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(FrameReader::new(&b"?"[..]).next_frame().is_err());
        assert!(FrameReader::new(&b""[..]).next_frame().is_err());
        let mut name = vec![TAG_ENTRY, 0, 0];
        name.extend_from_slice(&0u64.to_le_bytes());
        name.extend_from_slice(&2u32.to_le_bytes());
        name.extend_from_slice(&[0xFF, 0xFE]);
        assert!(FrameReader::new(name.as_slice()).next_frame().is_err());
    }
}
