// SPDX-License-Identifier: MIT OR Apache-2.0

//! UnRAR's DLL interface, declared by hand from `vendor/unrar/dll.hpp` (UnRAR 7.23). The
//! header packs its structs to 1 byte (`#pragma pack(1)`), hence `repr(C, packed)`. Only the
//! parts the helper uses.

use std::ffi::{c_char, c_int, c_uint, c_void};

/// `wchar_t`: UTF-16 on Windows, UTF-32 elsewhere.
#[cfg(windows)]
pub type WChar = u16;
#[cfg(not(windows))]
pub type WChar = u32;

/// `LPARAM` (Windows) or `long` (UnRAR's Unix build): pointer-sized on 64-bit targets.
pub type LParam = isize;

pub type Callback = extern "system" fn(msg: c_uint, user: LParam, p1: LParam, p2: LParam) -> c_int;

pub const ERAR_SUCCESS: c_int = 0;
pub const ERAR_END_ARCHIVE: c_int = 10;
pub const ERAR_NO_MEMORY: c_int = 11;
pub const ERAR_BAD_DATA: c_int = 12;
pub const ERAR_BAD_ARCHIVE: c_int = 13;
pub const ERAR_UNKNOWN_FORMAT: c_int = 14;
pub const ERAR_EOPEN: c_int = 15;
pub const ERAR_EREAD: c_int = 18;
pub const ERAR_MISSING_PASSWORD: c_int = 22;
pub const ERAR_EREFERENCE: c_int = 23;
pub const ERAR_BAD_PASSWORD: c_int = 24;
pub const ERAR_LARGE_DICT: c_int = 25;

pub const RAR_OM_LIST: c_uint = 0;
pub const RAR_OM_EXTRACT: c_uint = 1;

pub const RAR_SKIP: c_int = 0;
pub const RAR_TEST: c_int = 1;

pub const RHDF_SPLITBEFORE: c_uint = 0x01;
pub const RHDF_SPLITAFTER: c_uint = 0x02;
pub const RHDF_ENCRYPTED: c_uint = 0x04;
pub const RHDF_DIRECTORY: c_uint = 0x20;

pub const ROADF_VOLUME: c_uint = 0x0001;
pub const ROADF_ENCHEADERS: c_uint = 0x0080;

pub const UCM_CHANGEVOLUME: c_uint = 0;
pub const UCM_PROCESSDATA: c_uint = 1;
pub const UCM_NEEDPASSWORD: c_uint = 2;
pub const UCM_CHANGEVOLUMEW: c_uint = 3;
pub const UCM_NEEDPASSWORDW: c_uint = 4;
pub const UCM_LARGEDICT: c_uint = 5;

/// `HostOS` values.
pub const HOST_UNIX: c_uint = 3;

#[repr(C, packed)]
pub struct OpenArchiveDataEx {
    pub arc_name: *mut c_char,
    pub arc_name_w: *mut WChar,
    pub open_mode: c_uint,
    pub open_result: c_uint,
    pub cmt_buf: *mut c_char,
    pub cmt_buf_size: c_uint,
    pub cmt_size: c_uint,
    pub cmt_state: c_uint,
    pub flags: c_uint,
    pub callback: Option<Callback>,
    pub user_data: LParam,
    pub op_flags: c_uint,
    pub cmt_buf_w: *mut WChar,
    pub mark_of_the_web: *mut WChar,
    pub reserved: [c_uint; 23],
}

#[repr(C, packed)]
pub struct HeaderDataEx {
    pub arc_name: [c_char; 1024],
    pub arc_name_w: [WChar; 1024],
    pub file_name: [c_char; 1024],
    pub file_name_w: [WChar; 1024],
    pub flags: c_uint,
    pub pack_size: c_uint,
    pub pack_size_high: c_uint,
    pub unp_size: c_uint,
    pub unp_size_high: c_uint,
    pub host_os: c_uint,
    pub file_crc: c_uint,
    pub file_time: c_uint,
    pub unp_ver: c_uint,
    pub method: c_uint,
    pub file_attr: c_uint,
    pub cmt_buf: *mut c_char,
    pub cmt_buf_size: c_uint,
    pub cmt_size: c_uint,
    pub cmt_state: c_uint,
    pub dict_size: c_uint,
    pub hash_type: c_uint,
    pub hash: [c_char; 32],
    pub redir_type: c_uint,
    pub redir_name: *mut WChar,
    pub redir_name_size: c_uint,
    pub dir_target: c_uint,
    pub mtime_low: c_uint,
    pub mtime_high: c_uint,
    pub ctime_low: c_uint,
    pub ctime_high: c_uint,
    pub atime_low: c_uint,
    pub atime_high: c_uint,
    pub arc_name_ex: *mut WChar,
    pub arc_name_ex_size: c_uint,
    pub file_name_ex: *mut WChar,
    pub file_name_ex_size: c_uint,
    pub reserved: [c_uint; 982],
}

unsafe extern "system" {
    pub fn RAROpenArchiveEx(data: *mut OpenArchiveDataEx) -> *mut c_void;
    pub fn RARCloseArchive(handle: *mut c_void) -> c_int;
    pub fn RARReadHeaderEx(handle: *mut c_void, header: *mut HeaderDataEx) -> c_int;
    pub fn RARProcessFileW(
        handle: *mut c_void,
        operation: c_int,
        dest_path: *mut WChar,
        dest_name: *mut WChar,
    ) -> c_int;
}
