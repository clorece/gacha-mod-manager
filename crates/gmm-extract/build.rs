// SPDX-License-Identifier: MIT OR Apache-2.0

//! Builds the vendored UnRAR source (vendor/README.md) as a static library with its DLL
//! interface (`RARDLL`), linked into the gmm-extract helper. Same files and defines as
//! RARLAB's own UnRAR.dll project and makefile.

const SOURCES: &[&str] = &[
    "archive",
    "arcread",
    "blake2s",
    "cmddata",
    "consio",
    "crc",
    "crypt",
    "dll",
    "encname",
    "errhnd",
    "extinfo",
    "extract",
    "filcreat",
    "file",
    "filefn",
    "filestr",
    "find",
    "getbits",
    "global",
    "hash",
    "headers",
    "largepage",
    "match",
    "options",
    "pathfn",
    "qopen",
    "rar",
    "rarvm",
    "rawread",
    "rdwrfn",
    "rijndael",
    "rs",
    "rs16",
    "scantree",
    "secpassword",
    "sha1",
    "sha256",
    "smallfn",
    "strfn",
    "strlist",
    "system",
    "threadpool",
    "timefn",
    "ui",
    "unicode",
    "unpack",
    "volume",
];

/// Windows-only sources in the UnRAR.dll project.
const WINDOWS_SOURCES: &[&str] = &["isnt", "motw"];

fn main() {
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++14")
        // Always optimized: the helper decompresses up to gigabytes, even in debug builds.
        .opt_level(2)
        .warnings(false)
        .extra_warnings(false)
        .define("RARDLL", None)
        .define("UNRAR", None)
        .define("SILENT", None)
        .define("RAR_SMP", None)
        .define("_FILE_OFFSET_BITS", "64")
        .define("_LARGEFILE_SOURCE", None);
    let files = SOURCES
        .iter()
        .chain(if windows { WINDOWS_SOURCES } else { &[] })
        .map(|s| format!("vendor/unrar/{s}.cpp"));
    build.files(files);
    if windows {
        build.flag("/EHsc").define("_CRT_SECURE_NO_WARNINGS", None);
        // UnRAR's Windows code calls into these system libraries.
        for lib in ["advapi32", "shell32", "ole32", "powrprof"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    } else {
        for flag in [
            "-Wno-logical-op-parentheses",
            "-Wno-switch",
            "-Wno-dangling-else",
            "-Wno-parentheses",
        ] {
            build.flag_if_supported(flag);
        }
        println!("cargo:rustc-link-lib=pthread");
    }
    build.compile("gmm_unrar");
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=build.rs");
}
