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
| **.rar** (RAR4, RAR5) | ⏳ Phase 5, first item | Planned: a sandboxed helper process (below) | Target: RAR4 and RAR5, solid archives | Encrypted, multi-volume (`.part1.rar`) |
| Archive inside an archive | ⏳ Planned (quality of life) | Same pipeline, limits apply to both levels | When the outer archive holds only one archive, offer to open it | Deeper nesting |
| .tar, .tar.gz, .gz | ❌ Not planned | Not seen on GameBanana for these games | — | Would come almost free with the RAR helper if ever needed |
| Self-extracting .exe | ❌ Never | GMM never runs anything from a download | — | Refused as a blocked file |

Local archives (drag and drop, Phase 6) use the same formats and the same pipeline.

## What the user sees

| Case | When | Message |
|---|---|---|
| RAR (until Phase 5) | In the install confirmation, before anything downloads (the file name on GameBanana says `.rar`; the first bytes confirm it after download) | "This mod is packed as a RAR archive. GMM can't open RAR files yet; support is planned before the beta." with View on GameBanana |
| Encrypted archive | After download, before anything is extracted | "This archive is password-protected. GMM can't install it." The archive stays in the cache |
| Unsupported method (e.g. BZip2 inside a 7z) | During extraction; nothing is installed | "This archive uses a compression method GMM doesn't support (BZip2)." |
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

**Helper design**
- GMM starts the helper with fixed arguments only: the archive's path inside GMM's cache and
  an output folder inside `cache/extract/`. Nothing from the archive is ever run.
- Sandboxed: on Windows a restricted token and a job object (no network, memory and time
  limits, killed when GMM exits); on Linux seccomp.
- Two passes, like the in-process formats: the helper first reports every entry's name, type
  and sizes; GMM runs its checks (paths, links, sizes, ratios, entry count) on that list and
  only then lets it extract. GMM verifies the real sizes afterwards.
- Security rule R7 says GMM starts exactly one kind of process (XXMI). The helper becomes the
  second, GMM's own binary with fixed arguments; R7 is amended in the same change, and CI
  checks that process launching stays in its two places.

**Tests**: the malicious-archive corpus gets RAR cases (path traversal, links, bombs, lying
sizes, encrypted headers), plus real RAR mods in a manual check.
