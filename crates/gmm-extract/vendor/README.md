# Vendored source

## `unrar/`: UnRAR 7.23

- From RARLAB's official source archive `unrarsrc-7.2.7.tar.gz`
  (<https://www.rarlab.com/rar/unrarsrc-7.2.7.tar.gz>, downloaded 2026-10-24; `version.hpp`
  says 7.23, 27 June 2026, the current stable release).
  SHA-256 of the archive: `01d903a7dcf413cb2925696d7796e48e38d471f79bfe7ef3ad2aebf6c12dbefd`.
- Copied unchanged: every `.cpp` and `.hpp`, plus `license.txt`, `readme.txt` and
  `acknow.txt`. Left out: the Visual Studio projects, `makefile`, `dll.rc`, `dll.def`,
  `rarpch.cpp` (precompiled-header stub).
- Built by `../build.rs` with the same files and defines as RARLAB's UnRAR.dll project
  (`RARDLL`, `UNRAR`, `SILENT`, plus `RAR_SMP`). GMM calls only the DLL interface in
  `dll.hpp`, in test mode: UnRAR decompresses and checks each file and hands the data to a
  callback; it never writes a file.
- License: the UnRAR license (`unrar/license.txt`). The source may be used in any software to
  handle RAR archives, free of charge, but not to develop a RAR-compatible archiver or
  re-create the RAR compression algorithm. GMM only extracts. The license text is reproduced
  in `THIRD-PARTY-NOTICES.md` at the repository root.

To update: download the newest stable `unrarsrc-*.tar.gz` from
<https://www.rarlab.com/rar_add.htm> (check `version.hpp` says a release, not a beta),
replace the files above, compare the source lists in `UnRARDll.vcxproj` with `build.rs`,
record the version and SHA-256 here, and run `cargo test -p gmm-extract`.

## Why not libarchive

The first attempt (2026-10-24) built libarchive 3.8.10's RAR readers into this helper. It read
4 of 5 real RAR mods but failed one (`claretalt.rar`, GameBanana mod 731223) with a block
checksum error; Windows' own `tar.exe` (libarchive 3.8.9) fails the same archive. UnRAR reads
all five. See `docs/09-archive-formats.md`.
