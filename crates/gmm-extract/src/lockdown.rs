// SPDX-License-Identifier: MIT OR Apache-2.0

//! The helper lowers its own rights before it reads a byte of the archive. If any step fails
//! it refuses to continue (GMM reports the failure).
//!
//! - Windows: all privileges removed, integrity level lowered to Low (no writes to the user's
//!   files, registry or higher-integrity windows), and process mitigations on: no dynamic code,
//!   no extension-point DLLs, no remote or low-integrity images, strict handle checks. GMM has
//!   already put the process in a job object (one process, memory and CPU-time caps, no UI
//!   access).
//! - Linux: no new privileges, no file writes (`RLIMIT_FSIZE` 0), address-space and CPU-time
//!   caps.

use std::io;

#[cfg(windows)]
pub fn apply() -> io::Result<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, AllocateAndInitializeSid, FreeSid, PSID,
        SECURITY_MANDATORY_LABEL_AUTHORITY, SID_AND_ATTRIBUTES, SID_IDENTIFIER_AUTHORITY,
        SetTokenInformation, TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_PRIVILEGES, TOKEN_MANDATORY_LABEL,
        TOKEN_QUERY, TokenIntegrityLevel,
    };
    use windows_sys::Win32::System::SystemServices::{
        SE_GROUP_INTEGRITY, SECURITY_MANDATORY_LOW_RID,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcessToken, ProcessDynamicCodePolicy,
        ProcessExtensionPointDisablePolicy, ProcessImageLoadPolicy, ProcessStrictHandleCheckPolicy,
        SetProcessMitigationPolicy,
    };

    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            // SAFETY: closing the token handle opened below, once.
            unsafe { CloseHandle(self.0) };
        }
    }

    let mut raw: HANDLE = std::ptr::null_mut();
    // SAFETY: our own process's token, with the rights the steps below need.
    let ok = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_ADJUST_DEFAULT | TOKEN_QUERY,
            &mut raw,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Token(raw);

    // 1. No privileges at all.
    // SAFETY: DisableAllPrivileges ignores the (null) new-state argument.
    let ok = unsafe {
        AdjustTokenPrivileges(
            token.0,
            1,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    // SAFETY: reads this thread's last error right after the call.
    if ok == 0 || unsafe { GetLastError() } != 0 {
        return Err(io::Error::last_os_error());
    }

    // 2. Low integrity.
    let authority = SID_IDENTIFIER_AUTHORITY {
        Value: SECURITY_MANDATORY_LABEL_AUTHORITY.Value,
    };
    let mut sid: PSID = std::ptr::null_mut();
    // SAFETY: one sub-authority (the Low RID); the SID is freed below.
    let ok = unsafe {
        AllocateAndInitializeSid(
            &authority,
            1,
            SECURITY_MANDATORY_LOW_RID as u32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut sid,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let label = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid,
            Attributes: SE_GROUP_INTEGRITY as u32,
        },
    };
    // SAFETY: `label` and the SID it points to are alive for the call; the size covers the
    // struct (the SID is referenced, not inlined).
    let ok = unsafe {
        SetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            (&label as *const TOKEN_MANDATORY_LABEL).cast(),
            size_of::<TOKEN_MANDATORY_LABEL>() as u32,
        )
    };
    let err = io::Error::last_os_error();
    // SAFETY: the SID came from AllocateAndInitializeSid.
    unsafe { FreeSid(sid) };
    if ok == 0 {
        return Err(err);
    }

    // 3. Process mitigations. Each policy struct is a single u32 of flag bits.
    let policies: [(i32, u32); 4] = [
        // ProhibitDynamicCode
        (ProcessDynamicCodePolicy, 0b1),
        // DisableExtensionPoints
        (ProcessExtensionPointDisablePolicy, 0b1),
        // NoRemoteImages | NoLowMandatoryLabelImages | PreferSystem32Images
        (ProcessImageLoadPolicy, 0b111),
        // RaiseExceptionOnInvalidHandleReference | HandleExceptionsPermanentlyEnabled
        (ProcessStrictHandleCheckPolicy, 0b11),
    ];
    for (policy, flags) in policies {
        // SAFETY: each of these policies takes a 4-byte flags struct.
        let ok = unsafe {
            SetProcessMitigationPolicy(policy, (&flags as *const u32).cast(), size_of::<u32>())
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn apply() -> io::Result<()> {
    fn limit(resource: libc::__rlimit_resource_t, value: libc::rlim_t) -> io::Result<()> {
        let lim = libc::rlimit {
            rlim_cur: value,
            rlim_max: value,
        };
        // SAFETY: setrlimit with a valid struct.
        if unsafe { libc::setrlimit(resource, &lim) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    // prctl is variadic and reads its arguments as unsigned longs.
    let (on, zero): (libc::c_ulong, libc::c_ulong) = (1, 0);
    // SAFETY: PR_SET_NO_NEW_PRIVS takes integer arguments only.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, on, zero, zero, zero) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // No file can grow past 0 bytes (the output pipe isn't a file). RLIMIT_NPROC is left
    // alone: Linux counts threads against it, and UnRAR decompresses on several threads.
    limit(libc::RLIMIT_FSIZE, 0)?;
    // Address space, not memory in use: glibc reserves an arena per decompression thread.
    limit(libc::RLIMIT_AS, 8 << 30)?;
    limit(libc::RLIMIT_CPU, 10 * 60)?;
    Ok(())
}

/// Other platforms have no lockdown yet, so the helper refuses to run there.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn apply() -> io::Result<()> {
    Err(io::Error::other("no sandbox for this platform"))
}
