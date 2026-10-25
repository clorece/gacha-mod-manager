# 09 — Archive formats

Which compressed formats GMM can install from, what happens with the rest, and the plan for
RAR. Extraction safety (path checks, size limits, the policy scan) is in
[04](04-security.md#archive-hardening-gmm-archive); this doc is about formats.

## What mods ship as

Sample of 15 mod files across ZZZ and WuWa, 2026-10-24/25 (popular, new and multi-version
mods): **9 zip, 4 rar, 2 7z**. Nothing else appeared. GameBanana files for these games are
single archives; multi-part uploads weren't seen.

## Compatibility

GMM detects the format from the file's first bytes, never from its extension.

| Format | Status | How | Covers | Not covered (clear error) |
|---|---|---|---|---|
| **.zip** | ✅ Supported | `zip` crate, pure Rust | Stored, Deflate, Deflate64, LZMA | Encrypted (ZipCrypto, AES), BZip2, Zstandard, XZ, split zips (`.z01`) |
| **.7z** | ✅ Supported | `sevenz-rust2`, pure Rust | LZMA, LZMA2, Deflate, PPMd | Encrypted, BZip2 (left out for licensing, see [06](06-licensing-and-provenance.md)), multi-volume (`.7z.001`) |
| **.rar** (RAR4, RAR5) | ✅ Supported (2026-10-24) | UnRAR 7.23 in the sandboxed `gmm-extract` helper process (below) | RAR4 and RAR5 including solid archives and the RAR 7 format | Encrypted, multi-volume (`.part1.rar`), "file copy" references, links, RAR 7 dictionaries over 4 GB |
| Archive inside an archive | ⏳ Planned (quality of life) | Same pipeline, limits apply to both levels | When the outer archive holds only one archive, offer to open it | Deeper nesting |
| .tar, .tar.gz, .gz | ❌ Not planned | Not seen on GameBanana for these games | — | Would come almost free with the RAR helper if ever needed |
| Self-extracting .exe | ❌ Never | GMM never runs anything from a download | — | Refused as a blocked file |

Local archives (drag and drop, Phase 6) use the same formats and the same pipeline.

## What the user sees

| Case | When | Message |
|---|---|---|
| Encrypted archive | After download, before anything is extracted | "This archive is password-protected. GMM can't install it." The archive stays in the cache |
| Unsupported method (e.g. BZip2 inside a 7z, a RAR 7 dictionary over 4 GB) | During extraction; nothing is installed | "This archive uses a compression method GMM doesn't support (BZip2)." |
| Multi-part archive | Before download when the name shows it | "This mod is split into several files, which GMM can't install." |
| Not an archive | After download | "This file isn't an archive GMM can open." |
| Archive with blocked files | In the install confirmation | The files are listed; install without them or cancel |

## RAR plan (decision record)

**Need**: about a quarter of sampled mods are RAR, so the beta can't ship without it.

**Options considered**

| Option | License | Pros | Cons |
|---|---|---|---|
| **libarchive** (C), in a helper process | BSD-2-Clause (already allowed) | Reads RAR4 and RAR5, plus zip/7z/tar if ever useful; widely used; fits the license policy as is | C parser on untrusted input (hence the separate, sandboxed process); building it on Windows needs a vendored C build |
| UnRAR source (RARLAB), via the `unrar` crate, in a helper process | UnRAR license: free, allows extraction, forbids building a RAR compressor; not OSI | The reference decoder, best compatibility | Needs a per-crate license exception and a notice; C++ |
| A pure-Rust RAR decoder | — | Would stay in process | No mature RAR5 decoder known (checked 2026-10) |
| Windows' own `tar.exe` (libarchive inside) | — | Nothing to ship | Only on recent Windows 11, differs on Linux, a system program as a dependency |

**Plan**: libarchive in a small helper program (`gmm-extract`), shipped with the app. Try it
against the sampled RAR mods (including a 113 MiB one) before committing; if it fails on real
archives, switch to UnRAR with a license exception.

**Outcome (2026-10-24): UnRAR.** libarchive 3.8.10's RAR readers, built into the helper, read
4 of 5 real RAR mods and failed `claretalt.rar` (Goth Claret Outfit, mod 731223) with
"Block checksum error"; Windows' own `tar.exe` (libarchive 3.8.9) fails it the same way,
while the download's MD5 matched GameBanana's. So the helper now carries RARLAB's UnRAR
7.23 source instead (vendored, built with the `cc` crate; provenance in
`crates/gmm-extract/vendor/README.md`), with a per-crate license exception and its license
text in `THIRD-PARTY-NOTICES.md`. The `unrar` crates on crates.io were not used: they carry
an older UnRAR (7.1.0) and declare the UnRAR code as MIT / Apache-2.0.

| Real mod (GameBanana) | Archive | libarchive 3.8.10 | UnRAR 7.23 |
|---|---|---|---|
| Goth Claret Outfit (731223) | `claretalt.rar`, 40.2 MiB, RAR5 | ❌ block checksum error | ✅ 52 files, 217.8 MiB |
| Phoebe x Robin (595638) | `phoebe_x_robin.rar`, 17.5 MiB | ✅ | ✅ 45 files, 84.3 MiB |
| Hsin Bob Hair (729726) | `woju_mod_hsin_w_cute_bob_hair.rar`, 19.2 MiB | ✅ | ✅ 29 files, 51.1 MiB (two `.url` files blocked by the scan) |
| phoebe (574999) | `2_e2be0.rar`, 731 KiB | ✅ | ✅ 1 file, 4.0 MiB |
| Miasmic Hunter Color Choices (675310) | `yanagi_yuying_pack_color_options.rar`, 113 MiB | ✅ | ✅ 60 files, 377.9 MiB, both passes in about 3 s |

**Helper design, as built** (details in [04](04-security.md#archive-hardening-gmm-archive))
- GMM starts `gmm-extract` (shipped next to `gmm.exe` and the app) with one fixed argument,
  `list` or `extract`, an empty environment, and the archive as stdin. It gets no path and
  no output folder: UnRAR runs in test mode and the helper sends every entry and its data
  back over a pipe, so it never writes a file. Nothing from the archive is ever run.
- Sandboxed: on Windows a job object (one process, memory and CPU-time caps, no UI access,
  killed with GMM) and, inside the helper before it reads anything, no privileges, Low
  integrity and process mitigations; on Linux no-new-privileges and resource caps
  (seccomp still to do).
- Two passes: the helper lists every entry's name, type and size; GMM runs its checks on the
  whole list (paths, links, sizes, entry count, collisions) and only then runs the helper
  again to stream the data, which GMM writes itself, counting the real bytes.
- Security rule R7 was amended: GMM starts two kinds of process (XXMI and this helper), each
  from one place, and CI checks it stays that way.
- Builds: `cargo build --workspace` builds the helper; `tauri dev` and `tauri build` build it
  first (`beforeDevCommand` / `beforeBuildCommand`). A missing helper is reported ("GMM's RAR
  reader is missing"), and nothing is extracted.

**Tests**: `crates/gmm-extract/tests/rar.rs` builds RAR5 and RAR4 archives byte by byte (no
RAR writer exists as a library) and runs them through the real helper: traversal names,
links, special files, collisions, limits, lying sizes, encrypted headers and files,
truncation and corruption (19 cases). Compression bombs can't be built without a RAR
compressor; the ratio checks are the same code as for zip and 7z. Real RAR mods: the five
in the table above, checked by hand with `gmm fetch`.
