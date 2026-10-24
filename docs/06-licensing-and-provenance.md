# 06 — Licensing & provenance

Goal: ship GMM as an original work under a license **we** choose, with a record that shows it was
written from scratch. Not legal advice. If the project grows or takes money, get a short review
from a lawyer familiar with open-source licensing.

## Licenses of the projects we looked at (checked 2026-10-22)

| Project | License | How GMM relates to it |
|---|---|---|
| deadlock-mod-manager | GPL-3.0, no extra terms | Looked at its feature list, README, `SECURITY.md` and dependency list as **prior art**. No code, assets or text used |
| XXMI Launcher (SpectrumQT) | GPL-3.0 | Separate program the **user** installs. GMM detects it, reads a few config fields and launches it by command line. Its source was read to learn **facts** (paths, setting keys) |
| WWMI-Package, ZZMI-Package | GPL-3.0 | Installed by XXMI itself. GMM only reads version markers and writes mod folders into `Mods/` |
| XXMI-Libs-Package / 3Dmigoto | GPL-3.0 (3Dmigoto source); shaders distributed as game fixes are excluded from its license; bundles Nektra Deviare-InProcess (GPL-3.0) | Installed by XXMI. GMM reads `d3d11.dll`'s version resource only, never loads it |
| GameBanana mods | Per-mod (e.g. CC BY-NC-ND 4.0), exposed as `_sLicense` / `_aLicenseChecklist` | GMM never re-hosts. The user downloads from GameBanana directly. Credits and license are shown on each mod page |

## What copyright does and doesn't cover

- **Free to use:** ideas, features, workflows, architecture, the choice of public frameworks
  (Tauri, React, SQLite), and facts such as "XXMI loads mods from `Mods/` and skips `DISABLED*`"
  or "ZZZ's GameBanana id is 19567". Building "a mod manager with profiles, a GameBanana browser and
  conflict detection" does not derive from DMM.
- **Protected:** expression. That means source code (including close paraphrases and
  line-by-line ports into another language), UI artwork and icons, documentation text,
  translation strings, and distinctive UI layouts copied pixel for pixel.

## Clean-room rules for development

1. **No GPL source open while writing GMM code.** Don't read DMM or XXMI source while
   implementing the matching GMM component. Don't paste it into prompts for Claude either.
   Research ended with these design docs. From now on, implement from **our** specs
   ([02](02-architecture.md), [03](03-game-adapters.md)), which record facts in our own words.
2. **Need more facts? Get them from behaviour, not code.** Observe the game and the XXMI launcher
   from outside: diff files before and after a launch, inspect the SQLite DB, read the INI
   files. Write the findings into `03-game-adapters.md` first, then implement from that.
3. **Game settings stay with XXMI.** GMM doesn't patch game files (such as ZZZ's
   `GENERAL_DATA.bin`, whose encoding we only know from XXMI's code). That removes the highest
   clean-room risk entirely.
4. **No assets from other projects.** Make our own icons, logo and screenshots. Game art and logos
   belong to HoYoverse and Kuro Games and can't be used as branding. Mod preview images are
   loaded live from GameBanana, never bundled.
5. **Own wording everywhere.** README, docs, UI strings and translations are written fresh.

## Running GPL tools without becoming GPL

GMM talks to XXMI the way a shell talks to a program. The user installs XXMI from its own
releases. GMM starts `XXMI Launcher.exe` with command-line arguments in a separate process. The
FSF's GPL FAQ treats separate programs that communicate through `exec` and command-line arguments
as separate works, not one combined program. Reading XXMI's data files (config JSON, `d3dx.ini`)
and writing mod folders into `Mods/` is data exchange, not linking. To stay clearly on that side
of the line:

- Never link XXMI code or 3Dmigoto DLLs into GMM, and never load them into GMM's own process.
- Never bundle or redistribute XXMI. The setup guide links to its official releases page.
- Name XXMI and credit its authors in the setup guide and About page, with a link to its repo.

## Choosing GMM's license

Since no GPL code is used, we can pick any license:

| Option | Effect |
|---|---|
| **MIT OR Apache-2.0** (dual, common in Rust) | Anyone can reuse it, including in closed apps. Apache adds a patent grant |
| **GPL-3.0** | Forks must stay open source |
| **MPL-2.0** | Middle ground: changes to our files stay open; other code can be combined freely |

**Decision (2026-10-22): MIT OR Apache-2.0.**
- The repo root gets two files, `LICENSE-MIT` and `LICENSE-APACHE`, the standard Rust-ecosystem
  layout. The README says: "Licensed under either of Apache License 2.0 or MIT license at your
  option."
- `Cargo.toml` (every crate): `license = "MIT OR Apache-2.0"`. `package.json`:
  `"license": "MIT OR Apache-2.0"`.
- Contributions are inbound=outbound. Add this to `CONTRIBUTING.md`: "Unless you explicitly state
  otherwise, any contribution you submit is dual licensed as above, without any additional terms
  or conditions."

Our dependencies must fit this license. `cargo-deny` and an npm license checker enforce an
allowlist (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, MPL-2.0, Unicode-3.0, CC0-1.0). Any GPL,
LGPL or AGPL dependency fails the build. Watch out for:
- **`unrar` crate:** uses the freeware *UnRAR license*, which bans using the code to build a RAR
  compressor. Shipping it is fine, but it isn't OSI-approved. Keep RAR support behind a Cargo
  feature and list it in the third-party notices.
- **GPL crates or npm packages:** blocked by the allowlist. Find a permissive alternative.
- **MPL-2.0 dependencies** are allowed: they stay MPL, and we must not modify their files without
  publishing those changes.
- **Per-crate exceptions** live in `deny.toml` with a reason. Currently one: `webpki-root-certs`
  (Mozilla's CA certificate list, `CDLA-Permissive-2.0`, a permissive data license).
- **Left out for licensing:** 7z BZip2 support (`libbz2-rs-sys`, `bzip2-1.0.6` license).
- **Direct dependencies so far** (all pass `cargo deny`): `zip`, `sevenz-rust2`, `reqwest` +
  `rustls` (ring), `tokio`, `serde`/`serde_json`, `toml`, `url`, `sha2`, `md-5`, `hex`, `clap`,
  `dirs`, `thiserror`, `pelite` (MIT; reads `d3d11.dll`'s version resource), `winreg` (MIT;
  Windows registry reads), `sqlx` 0.9 (MIT OR Apache-2.0; SQLite only, no macros), which pulls
  in `libsqlite3-sys` (MIT) with SQLite's own C source **bundled**. SQLite itself is in the
  public domain; list it in `THIRD-PARTY-NOTICES` anyway. Building it needs a C compiler
  (the MSVC Build Tools the Rust Windows toolchain already uses). Dev-only: `proptest`, `tempfile`, `wiremock`.
- **Facts about other software** come from observing it on the dev machine (file layouts,
  config keys, locale strings), recorded in our own words in `docs/03-game-adapters.md`. The
  HoYoPlay `gamedata.dat` handling, for example, is based only on what the file visibly
  contains (`"installPath"` strings), not on any other tool's code.

## Names and trademarks

- Don't use "Deadlock Mod Manager", "DMM", or names that sound like them.
- "Zenless Zone Zero", "Wuthering Waves" and "GameBanana" may be used **descriptively** ("manage
  mods for Wuthering Waves") but not in the product name or logo. Add a disclaimer: "Not
  affiliated with or endorsed by HoYoverse, Kuro Games or GameBanana. All trademarks belong to
  their owners."
- Before publishing, search GitHub, the Windows Store, Flathub and the USPTO/EUIPO trademark
  databases for the chosen name. "Gacha Mod Manager" is only a working name.

## Provenance record (start on the first commit)

- Initialize git in the real project folder. History from day one is the best evidence of
  independent authorship.
- `LICENSE-MIT` + `LICENSE-APACHE`, plus SPDX headers in source files (`// SPDX-License-Identifier: MIT OR Apache-2.0`).
- **DCO sign-off** (`git commit -s`) for all contributors, and a PR template checkbox: "I did not
  copy or port code from other projects; any third-party code is listed in
  THIRD-PARTY-NOTICES."
- Generate `THIRD-PARTY-NOTICES` from the dependency tree (`cargo about`, plus an npm license
  report) in CI.
- AI-assisted code: commits made with Claude carry a `Co-Authored-By` line. Reviews check that
  generated code doesn't resemble another project's code, especially for components
  where prior art exists (archive extraction, GameBanana client).
- Research notes about other projects (`docs/research/`) stay **out of the public repo** or are
  clearly marked as prior-art notes. They aren't needed to understand GMM.

## Before the first public release

- [x] License chosen: MIT OR Apache-2.0
- [ ] `LICENSE-MIT`, `LICENSE-APACHE`, crate/package metadata and SPDX headers in place
- [ ] `cargo deny check licenses` and the npm license check pass
- [ ] `THIRD-PARTY-NOTICES` generated and shown in Settings → About
- [ ] No assets, strings or code from other projects (spot-check by searching distinctive strings)
- [ ] Name cleared; disclaimer in README and About
- [ ] Setup guide and About credit XXMI and link to its official repo/releases
- [ ] GameBanana contacted about API use / 1-click
- [ ] `docs/research/` removed from the public repo or marked as prior art
