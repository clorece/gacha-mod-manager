// SPDX-License-Identifier: MIT OR Apache-2.0

//! RAR4 and RAR5, read by the sandboxed `gmm-extract` helper (libarchive). Two passes like the
//! other formats: the helper lists every entry, GMM checks the whole list, and only then does
//! a second run stream the data, which GMM writes itself through `gmm-fsguard`, counting real
//! bytes against the limits. The helper never writes a file.

use std::io::{self, Read};

use gmm_fsguard::{Guard, RelPath, ScopedPath};

use crate::error::ArchiveError;
use crate::extract::{ExtractedEntry, Writer};
use crate::helper::Helper;
use crate::limits::Limits;
use crate::plan::{EntryKind, Planner};
use crate::rar_protocol::{EntryType, FailKind, Frame, FrameReader, Mode};

pub(crate) fn extract(
    guard: &Guard,
    archive: &ScopedPath,
    dest: &ScopedPath,
    limits: &Limits,
    helper: &Helper,
) -> Result<Vec<ExtractedEntry>, ArchiveError> {
    let archive_len = guard.metadata(archive)?.len();

    // Pass 1: the list, checked as a whole before anything is written.
    let mut listing = helper.spawn(guard.open_read(archive)?, Mode::List)?;
    let mut planner = Planner::new(limits);
    let mut planned: Vec<(String, RelPath, EntryKind)> = Vec::new();
    loop {
        match next(&mut listing.frames)? {
            Frame::Entry(e) => {
                if e.encrypted {
                    return Err(ArchiveError::Encrypted);
                }
                let kind = kind_of(e.kind);
                let rel = planner.check(&e.name, kind, e.size.unwrap_or(0))?;
                planned.push((e.name, rel, kind));
            }
            Frame::Data(_) => return Err(protocol("data while listing")),
            Frame::End => break,
            Frame::Fail { kind, message } => return Err(failure(kind, message)),
        }
    }
    listing.finish()?;

    // Pass 2: the same entries in the same order, now with their data.
    let mut stream = helper.spawn(guard.open_read(archive)?, Mode::Extract)?;
    let mut writer = Writer::new(guard, dest, limits, archive_len);
    let mut index = 0;
    let mut frame = next(&mut stream.frames)?;
    loop {
        match frame {
            Frame::Entry(e) => {
                let Some((name, rel, kind)) = planned.get(index) else {
                    return Err(changed());
                };
                if &e.name != name || kind_of(e.kind) != *kind {
                    return Err(changed());
                }
                index += 1;
                if *kind == EntryKind::Dir {
                    writer.dir(rel)?;
                    frame = next(&mut stream.frames)?;
                } else {
                    let mut data = EntryData {
                        frames: &mut stream.frames,
                        at: 0,
                        len: 0,
                        after: None,
                    };
                    writer.file(rel, &mut data, None)?;
                    frame = match data.after.take() {
                        Some(f) => f,
                        None => next(&mut stream.frames)?,
                    };
                }
            }
            Frame::Data(_) => return Err(protocol("data without an entry")),
            Frame::End => break,
            Frame::Fail { kind, message } => return Err(failure(kind, message)),
        }
    }
    if index != planned.len() {
        return Err(changed());
    }
    stream.finish()?;
    Ok(writer.into_entries())
}

/// One file's data: the data frames up to the next other frame, which is kept for the caller.
struct EntryData<'a, R> {
    frames: &'a mut FrameReader<R>,
    at: usize,
    len: usize,
    after: Option<Frame>,
}

impl<R: Read> Read for EntryData<'_, R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.at == self.len {
            if self.after.is_some() {
                return Ok(0);
            }
            match self.frames.next_frame()? {
                Frame::Data(n) => {
                    self.at = 0;
                    self.len = n;
                }
                Frame::Fail { kind, message } => {
                    // Surfaces as `Corrupt` through the writer; the message names the cause.
                    return Err(io::Error::other(failure(kind, message).to_string()));
                }
                other => {
                    self.after = Some(other);
                    return Ok(0);
                }
            }
        }
        let n = out.len().min(self.len - self.at);
        out[..n].copy_from_slice(&self.frames.data()[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

fn next<R: Read>(frames: &mut FrameReader<R>) -> Result<Frame, ArchiveError> {
    frames.next_frame().map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof => ArchiveError::Helper("the helper stopped early".into()),
        _ => protocol(&e.to_string()),
    })
}

fn kind_of(t: EntryType) -> EntryKind {
    match t {
        EntryType::File => EntryKind::File,
        EntryType::Dir => EntryKind::Dir,
        EntryType::Link => EntryKind::Symlink,
        EntryType::Special => EntryKind::Special,
    }
}

fn failure(kind: FailKind, message: String) -> ArchiveError {
    match kind {
        FailKind::Encrypted => ArchiveError::Encrypted,
        FailKind::Unsupported => ArchiveError::UnsupportedMethod(message),
        FailKind::Corrupt | FailKind::BadName => ArchiveError::Corrupt(message),
        FailKind::Lockdown => ArchiveError::Helper(format!("sandbox setup failed: {message}")),
    }
}

fn protocol(what: &str) -> ArchiveError {
    ArchiveError::Helper(format!("unexpected output from the helper: {what}"))
}

fn changed() -> ArchiveError {
    ArchiveError::Corrupt("the archive's entries differ between the two passes".into())
}
