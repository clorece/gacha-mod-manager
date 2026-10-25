// SPDX-License-Identifier: MIT OR Apache-2.0

//! Starting the `gmm-extract` helper (security rule R7, the second kind of process GMM
//! starts). Fixed program (GMM's own helper next to the running exe), one fixed argument
//! (`list` or `extract`), an empty environment. The archive arrives as the helper's stdin,
//! already opened read-only through `gmm-fsguard`; the helper gets no path and needs no file
//! access at all. On Windows it starts suspended, joins a job object (one process, memory and
//! CPU-time caps, no UI access, killed when GMM drops it), and only then runs; it lowers its
//! own rights before reading (gmm-extract's `lockdown.rs`).

// The helper is GMM's own program, started with fixed arguments (docs/04-security.md, R7).
#![allow(clippy::disallowed_methods)]

use std::fs::File;
use std::io::{BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

use crate::error::ArchiveError;
use crate::rar_protocol::{FrameReader, Mode};

/// The helper's file name.
#[cfg(windows)]
const EXE: &str = "gmm-extract.exe";
#[cfg(not(windows))]
const EXE: &str = "gmm-extract";

/// Where GMM's RAR helper is.
#[derive(Debug, Clone)]
pub struct Helper {
    exe: PathBuf,
}

impl Helper {
    /// The helper shipped next to the running program. Under `cargo test` the running program
    /// is in `target/<profile>/deps/`, so its parent folder is tried too.
    pub fn beside_current_exe() -> Result<Self, ArchiveError> {
        let current = std::env::current_exe().map_err(|e| ArchiveError::Helper(e.to_string()))?;
        let dir = current
            .parent()
            .ok_or_else(|| ArchiveError::Helper("no folder for the running program".into()))?;
        let candidates = [Some(dir), dir.parent().filter(|_| dir.ends_with("deps"))];
        for d in candidates.into_iter().flatten() {
            let exe = d.join(EXE);
            if exe.is_file() {
                return Ok(Self { exe });
            }
        }
        Err(ArchiveError::HelperMissing(dir.join(EXE)))
    }

    /// A helper at an exact path (tests run the one cargo just built).
    pub fn at(exe: impl Into<PathBuf>) -> Self {
        Self { exe: exe.into() }
    }

    pub fn path(&self) -> &Path {
        &self.exe
    }

    /// Starts the helper on `archive`, sandboxed.
    pub(crate) fn spawn(&self, archive: File, mode: Mode) -> Result<Running, ArchiveError> {
        let mut cmd = Command::new(&self.exe);
        cmd.arg(mode.arg())
            .env_clear()
            .stdin(Stdio::from(archive))
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // The DLL loader wants SystemRoot even with an otherwise empty environment.
            if let Some(root) = std::env::var_os("SystemRoot") {
                cmd.env("SystemRoot", root);
            }
            cmd.creation_flags(sandbox::CREATE_SUSPENDED | sandbox::CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| match e.kind() {
            ErrorKind::NotFound => ArchiveError::HelperMissing(self.exe.clone()),
            _ => ArchiveError::Helper(format!("couldn't start {}: {e}", self.exe.display())),
        })?;
        #[cfg(windows)]
        let job = match sandbox::confine(&child) {
            Ok(job) => job,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ArchiveError::Helper(format!(
                    "couldn't sandbox the helper: {e}"
                )));
            }
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ArchiveError::Helper("no output pipe".into()));
        };
        Ok(Running {
            child,
            frames: FrameReader::new(BufReader::with_capacity(256 * 1024, stdout)),
            #[cfg(windows)]
            _job: job,
        })
    }
}

/// A running helper. Dropping it kills the helper if it is still running.
pub(crate) struct Running {
    child: Child,
    pub(crate) frames: FrameReader<BufReader<ChildStdout>>,
    #[cfg(windows)]
    _job: sandbox::Job,
}

impl Running {
    /// Waits for a clean exit after the last frame.
    pub(crate) fn finish(mut self) -> Result<(), ArchiveError> {
        let status = self
            .child
            .wait()
            .map_err(|e| ArchiveError::Helper(e.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(ArchiveError::Helper(format!(
                "the helper stopped with {status}"
            )))
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
mod sandbox {
    //! The helper's job object. Reviewed `unsafe`: Win32 calls on handles this module owns
    //! (docs/04-security.md, R7).
    #![allow(unsafe_code)]

    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_LIMIT_PROCESS_TIME, JOB_OBJECT_UILIMIT_DESKTOP,
        JOB_OBJECT_UILIMIT_DISPLAYSETTINGS, JOB_OBJECT_UILIMIT_EXITWINDOWS,
        JOB_OBJECT_UILIMIT_GLOBALATOMS, JOB_OBJECT_UILIMIT_HANDLES,
        JOB_OBJECT_UILIMIT_READCLIPBOARD, JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS,
        JOB_OBJECT_UILIMIT_WRITECLIPBOARD, JOBOBJECT_BASIC_UI_RESTRICTIONS,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicUIRestrictions,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    pub(super) use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};

    /// Most memory the helper may commit. RAR5 dictionaries in mod archives are far smaller.
    const MEMORY_LIMIT: usize = 2 << 30;
    /// Most CPU time (user mode) per run, in 100 ns units: 10 minutes.
    const CPU_LIMIT: i64 = 10 * 60 * 10_000_000;

    /// Owns the job handle; closing it kills the helper (KILL_ON_JOB_CLOSE).
    pub(crate) struct Job(HANDLE);

    // SAFETY: a job handle can be used and closed from any thread.
    unsafe impl Send for Job {}

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: we own the handle and close it once.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Puts the suspended `child` into a new job, then lets it run.
    pub(super) fn confine(child: &Child) -> io::Result<Job> {
        // SAFETY: null name and attributes create an anonymous job owned by this process.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Job(handle);

        // SAFETY: zeroed is a valid value for these plain-data structs.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_PROCESS_TIME
            | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        limits.BasicLimitInformation.PerProcessUserTimeLimit = CPU_LIMIT;
        limits.ProcessMemoryLimit = MEMORY_LIMIT;
        set(
            &job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>(),
        )?;

        let ui = JOBOBJECT_BASIC_UI_RESTRICTIONS {
            UIRestrictionsClass: JOB_OBJECT_UILIMIT_DESKTOP
                | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
                | JOB_OBJECT_UILIMIT_EXITWINDOWS
                | JOB_OBJECT_UILIMIT_GLOBALATOMS
                | JOB_OBJECT_UILIMIT_HANDLES
                | JOB_OBJECT_UILIMIT_READCLIPBOARD
                | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS
                | JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
        };
        set(
            &job,
            JobObjectBasicUIRestrictions,
            &ui as *const _ as *const _,
            size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>(),
        )?;

        let process = child.as_raw_handle() as HANDLE;
        // SAFETY: both handles are valid and owned (the child's by `Child`).
        if unsafe { AssignProcessToJobObject(job.0, process) } == 0 {
            return Err(io::Error::last_os_error());
        }
        resume(child.id())?;
        Ok(job)
    }

    fn set(job: &Job, class: i32, info: *const core::ffi::c_void, len: usize) -> io::Result<()> {
        let len = u32::try_from(len).map_err(|_| io::Error::other("job info size"))?;
        // SAFETY: `info` points to a live struct of `len` bytes matching `class`.
        if unsafe { SetInformationJobObject(job.0, class, info, len) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Resumes the suspended process's threads (it has exactly one, its main thread).
    fn resume(pid: u32) -> io::Result<()> {
        // SAFETY: a thread snapshot of the whole system; closed below.
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snap == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: zeroed is valid; dwSize is set as the API requires.
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        let mut resumed = 0;
        // SAFETY: `snap` is a valid snapshot and `entry` a properly sized buffer.
        let mut more = unsafe { Thread32First(snap, &mut entry) } != 0;
        while more {
            if entry.th32OwnerProcessID == pid {
                // SAFETY: opening a thread of our own child with the right to resume it.
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if !thread.is_null() {
                    // SAFETY: `thread` is a valid handle, closed right after.
                    unsafe {
                        ResumeThread(thread);
                        CloseHandle(thread);
                    }
                    resumed += 1;
                }
            }
            // SAFETY: as above.
            more = unsafe { Thread32Next(snap, &mut entry) } != 0;
        }
        // SAFETY: closing the snapshot handle we opened.
        unsafe { CloseHandle(snap) };
        if resumed == 0 {
            return Err(io::Error::other("the helper's thread wasn't found"));
        }
        Ok(())
    }
}
