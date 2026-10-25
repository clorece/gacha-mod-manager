// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm-extract`: GMM's RAR reader. GMM starts it with one argument (`list` or `extract`) and
//! the archive as stdin (opened read-only by GMM). It lowers its own rights first
//! ([`lockdown`]), then reads the archive with UnRAR in test mode (decompress and verify,
//! never write) and sends every entry and its data to stdout as frames
//! ([`gmm_archive::rar_protocol`]). It creates no files, and GMM checks everything it says.
//!
//! Reviewed `unsafe` (docs/04-security.md, R7): calls into the vendored UnRAR (`ffi`), the
//! data callback, and the Win32 / libc calls in `lockdown` and `stdin_path`.
#![allow(unsafe_code)]

mod ffi;
mod lockdown;

use std::ffi::c_int;
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use gmm_archive::rar_protocol::{self as proto, EntryHeader, EntryType, FailKind, Mode};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mode = match (args.next().as_deref().and_then(Mode::from_arg), args.next()) {
        (Some(mode), None) => mode,
        _ => {
            eprintln!("gmm-extract is started by GMM; it isn't meant to be run by hand");
            return ExitCode::from(2);
        }
    };
    let stdout = io::stdout();
    let mut out = BufWriter::with_capacity(256 * 1024, stdout.lock());
    if let Err(e) = lockdown::apply() {
        let _ = proto::write_fail(&mut out, FailKind::Lockdown, &e.to_string());
        let _ = out.flush();
        return ExitCode::from(3);
    }
    match run(mode, &mut out).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        // GMM stopped reading (it found a problem and dropped us): nothing left to say.
        Err(_) => ExitCode::from(4),
    }
}

/// Shared with UnRAR's callback through its `UserData` pointer.
struct Context<'a> {
    out: &'a mut dyn Write,
    /// A failed write to GMM; reported after UnRAR returns.
    io_error: Option<io::Error>,
    password_asked: bool,
    large_dictionary: bool,
}

/// Owns the UnRAR archive handle.
struct Handle(*mut std::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from RAROpenArchiveEx and is closed once.
        unsafe { ffi::RARCloseArchive(self.0) };
    }
}

fn run(mode: Mode, out: &mut dyn Write) -> io::Result<()> {
    let Some(mut path) = stdin_path() else {
        return proto::write_fail(out, FailKind::Corrupt, "the archive isn't a readable file");
    };
    let mut ctx = Context {
        out,
        io_error: None,
        password_asked: false,
        large_dictionary: false,
    };
    // SAFETY: zeroed is valid for this plain-data struct (null pointers, no callback).
    let mut open: ffi::OpenArchiveDataEx = unsafe { std::mem::zeroed() };
    open.arc_name_w = path.as_mut_ptr();
    open.open_mode = match mode {
        Mode::List => ffi::RAR_OM_LIST,
        Mode::Extract => ffi::RAR_OM_EXTRACT,
    };
    open.callback = Some(callback);
    open.user_data = &mut ctx as *mut Context as ffi::LParam;
    // SAFETY: `open` is fully initialized; `path` and `ctx` outlive the handle (both live
    // until this function returns, after `handle` is dropped).
    let raw = unsafe { ffi::RAROpenArchiveEx(&mut open) };
    let (result, flags) = (open.open_result, open.flags);
    if raw.is_null() || result != 0 {
        let code = c_int::try_from(result).unwrap_or(ffi::ERAR_BAD_ARCHIVE);
        return fail(&mut ctx, code);
    }
    let handle = Handle(raw);
    if flags & ffi::ROADF_ENCHEADERS != 0 {
        return proto::write_fail(
            ctx.out,
            FailKind::Encrypted,
            "the archive's headers are encrypted",
        );
    }
    if flags & ffi::ROADF_VOLUME != 0 {
        return proto::write_fail(ctx.out, FailKind::Unsupported, "multi-part RAR archives");
    }

    // SAFETY: zeroed is valid for this plain-data struct; it's large, so it lives on the heap.
    let mut header: Box<ffi::HeaderDataEx> = Box::new(unsafe { std::mem::zeroed() });
    // Long names land here instead of being cut at 1023 characters.
    let mut name_buf: Vec<ffi::WChar> = vec![0; 8192];
    header.file_name_ex = name_buf.as_mut_ptr();
    header.file_name_ex_size = name_buf.len() as u32;
    loop {
        // SAFETY: `handle` is open; `header` and its name buffer are valid and writable.
        let status = unsafe { ffi::RARReadHeaderEx(handle.0, &mut *header) };
        if status == ffi::ERAR_END_ARCHIVE {
            break;
        }
        if status != ffi::ERAR_SUCCESS {
            return fail(&mut ctx, status);
        }
        let flags = header.flags;
        if flags & (ffi::RHDF_SPLITBEFORE | ffi::RHDF_SPLITAFTER) != 0 {
            return proto::write_fail(ctx.out, FailKind::Unsupported, "multi-part RAR archives");
        }
        let Some(entry) = describe(&header, &name_buf) else {
            return proto::write_fail(ctx.out, FailKind::BadName, "an entry name can't be read");
        };
        proto::write_entry(ctx.out, &entry)?;
        let op = if mode == Mode::Extract && entry.kind == EntryType::File && !entry.encrypted {
            ffi::RAR_TEST
        } else {
            ffi::RAR_SKIP
        };
        // SAFETY: `handle` is open and positioned at this entry; test mode writes nothing,
        // the data arrives through `callback`.
        let status = unsafe {
            ffi::RARProcessFileW(handle.0, op, std::ptr::null_mut(), std::ptr::null_mut())
        };
        if let Some(e) = ctx.io_error.take() {
            return Err(e);
        }
        if status != ffi::ERAR_SUCCESS {
            return fail(&mut ctx, status);
        }
    }
    drop(handle);
    proto::write_end(ctx.out)
}

/// Data and questions from UnRAR while it reads.
extern "system" fn callback(
    msg: u32,
    user: ffi::LParam,
    p1: ffi::LParam,
    p2: ffi::LParam,
) -> c_int {
    // SAFETY: `user` is the `Context` pointer set in `run`, alive for the whole read, and
    // UnRAR calls back on the thread that called it.
    let ctx = unsafe { &mut *(user as *mut Context) };
    match msg {
        ffi::UCM_PROCESSDATA => {
            let Ok(len) = usize::try_from(p2) else {
                return -1;
            };
            // SAFETY: UnRAR passes the address and length of its decompressed block.
            let data = unsafe { std::slice::from_raw_parts(p1 as *const u8, len) };
            match proto::write_data(ctx.out, data) {
                Ok(()) => 1,
                Err(e) => {
                    ctx.io_error = Some(e);
                    -1
                }
            }
        }
        ffi::UCM_NEEDPASSWORD | ffi::UCM_NEEDPASSWORDW => {
            ctx.password_asked = true;
            -1
        }
        ffi::UCM_LARGEDICT => {
            // A dictionary above UnRAR's default limit (the RAR 7 extension): refused.
            ctx.large_dictionary = true;
            -1
        }
        // No other volumes, and nothing else is answered.
        ffi::UCM_CHANGEVOLUME | ffi::UCM_CHANGEVOLUMEW => -1,
        _ => -1,
    }
}

fn fail(ctx: &mut Context, code: c_int) -> io::Result<()> {
    if let Some(e) = ctx.io_error.take() {
        return Err(e);
    }
    let (kind, msg) = match code {
        ffi::ERAR_MISSING_PASSWORD | ffi::ERAR_BAD_PASSWORD => {
            (FailKind::Encrypted, "the archive is password-protected")
        }
        _ if ctx.password_asked => (FailKind::Encrypted, "the archive is password-protected"),
        ffi::ERAR_LARGE_DICT => (FailKind::Unsupported, "a RAR 7 dictionary larger than 4 GB"),
        _ if ctx.large_dictionary => (FailKind::Unsupported, "a RAR 7 dictionary larger than 4 GB"),
        ffi::ERAR_UNKNOWN_FORMAT => (FailKind::Unsupported, "an unknown RAR format version"),
        ffi::ERAR_NO_MEMORY => (FailKind::Corrupt, "not enough memory to decompress"),
        ffi::ERAR_BAD_DATA => (FailKind::Corrupt, "damaged data (checksum error)"),
        ffi::ERAR_BAD_ARCHIVE => (FailKind::Corrupt, "not a valid RAR archive"),
        ffi::ERAR_EOPEN | ffi::ERAR_EREAD => (FailKind::Corrupt, "the archive couldn't be read"),
        ffi::ERAR_EREFERENCE => (
            FailKind::Unsupported,
            "a file stored as a reference to another",
        ),
        _ => (FailKind::Corrupt, "UnRAR reported an error"),
    };
    proto::write_fail(ctx.out, kind, &format!("{msg} (UnRAR error {code})"))
}

/// Unix file types, for archives made on Unix (their attributes are a `st_mode`).
const S_IFMT: u32 = 0o170_000;
const S_IFREG: u32 = 0o100_000;
const S_IFDIR: u32 = 0o040_000;
const S_IFLNK: u32 = 0o120_000;
/// Windows FILE_ATTRIBUTE_REPARSE_POINT (symlinks, junctions).
const REPARSE_POINT: u32 = 0x400;

/// The entry's name, type, size and encryption flag; `None` when the name doesn't decode or
/// fills the whole name buffer (it might have been cut).
fn describe(h: &ffi::HeaderDataEx, name_buf: &[ffi::WChar]) -> Option<EntryHeader> {
    let len = name_buf.iter().position(|&c| c == 0)?;
    let name = wide_to_string(&name_buf[..len])?.replace('\\', "/");
    let (flags, attrs, host, redir) = (h.flags, h.file_attr, h.host_os, h.redir_type);
    let kind = if redir != 0 {
        // Symlinks, junctions, hard links and file copies all point at something else.
        EntryType::Link
    } else if flags & ffi::RHDF_DIRECTORY != 0 {
        EntryType::Dir
    } else if host == ffi::HOST_UNIX {
        match attrs & S_IFMT {
            S_IFREG | 0 => EntryType::File,
            S_IFDIR => EntryType::Dir,
            S_IFLNK => EntryType::Link,
            _ => EntryType::Special,
        }
    } else if attrs & REPARSE_POINT != 0 {
        EntryType::Link
    } else {
        EntryType::File
    };
    let size = u64::from(h.unp_size) | (u64::from(h.unp_size_high) << 32);
    Some(EntryHeader {
        kind,
        encrypted: flags & ffi::RHDF_ENCRYPTED != 0,
        size: Some(size),
        name,
    })
}

#[cfg(windows)]
fn wide_to_string(w: &[u16]) -> Option<String> {
    String::from_utf16(w).ok()
}

#[cfg(not(windows))]
fn wide_to_string(w: &[u32]) -> Option<String> {
    w.iter().map(|&c| char::from_u32(c)).collect()
}

/// The path of the file GMM handed over as stdin, NUL-terminated, for UnRAR to open (it reads
/// archives by name). Nothing but that file is opened.
#[cfg(windows)]
fn stdin_path() -> Option<Vec<u16>> {
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW,
    };
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE};

    // SAFETY: returns this process's stdin handle (or null / invalid, checked below).
    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if handle.is_null() || handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return None;
    }
    let mut buf = vec![0u16; 32_768];
    // SAFETY: `buf` is writable for its length; the call fails cleanly on pipes and consoles.
    let n = unsafe {
        GetFinalPathNameByHandleW(
            handle,
            buf.as_mut_ptr(),
            buf.len() as u32,
            FILE_NAME_NORMALIZED,
        )
    } as usize;
    if n == 0 || n >= buf.len() {
        return None;
    }
    buf.truncate(n);
    buf.push(0);
    Some(buf)
}

#[cfg(not(windows))]
fn stdin_path() -> Option<Vec<u32>> {
    Some("/proc/self/fd/0\0".chars().map(u32::from).collect())
}
