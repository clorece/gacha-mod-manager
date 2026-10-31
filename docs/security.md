# Security & privacy

The goal: anyone can **check** that GMM only touches the files and hosts it says it does. The
rules below are enforced in code and CI, and the test harnesses prove them.

## Threat model

| # | Threat | Main defence |
|---|---|---|
| T1 | GMM itself overreaches (a bug, a bad dependency) | R1–R5, fsguard, network allowlist, egress and file-access harnesses |
| T2 | A malicious mod archive (path escapes, zip bombs, bundled `.exe`) | Hardened extraction and the policy scan |
| T3 | Malicious mod descriptions (XSS) | HTML rewritten in Rust, DOMPurify, strict CSP, IPC takes ids only |
| T4 | Tampered downloads or redirects | Host allowlist on every redirect, exact sizes, GameBanana's virus scan + MD5, SHA-256 |
| T5 | Starting the wrong program | Only the exe inside a confirmed XXMI folder, confirmed by the user |
| T6 | Deep-link abuse | No deep links exist; if added, they carry GameBanana ids only and need a confirmation |
| T7 | Supply chain | Lockfiles, `cargo-deny`, `pnpm audit`, license checks, few dependencies |
| T8 | A fake GMM update | Only GMM's own release installers are downloaded, and only one signed with GMM's update key (ed25519, private half in a repository secret) for that exact version is run |
| T9 | Deleting the wrong mods | Every delete lists what goes and needs a confirmation |
| T10 | Account or Terms of Service risk | GMM never injects anything or touches anti-cheat |

## Hard rules

- **R1 — No telemetry.** No analytics, no crash uploader. The only log is local
  (`logs/errors.log`): errors only, rotated at 256 KiB, with the home folder written as `~`.
  Report a Bug only opens a pre-filled GitHub issue in the browser, after a confirmation; GMM
  sends nothing itself, and the user drags the log file in by hand.
- **R2 — Network allowlist.** Only `gmm-net` opens connections (clippy bans other HTTP
  clients and sockets). Every request and redirect must be HTTPS to `gamebanana.com`,
  `files.gamebanana.com`, `images.gamebanana.com` or `filecache<N>.gamebanana.com`, plus
  GitHub's releases list for the update check and, for the updater, GMM's own installers
  (`github.com/clorece/gacha-mod-manager/releases/download/v<version>/GMM-<version>-x64.msi`
  and its `.sig`, then GitHub's file host). Each of the three uses its own client that can
  reach nothing else. The webview makes no network requests at all
  (CSP `connect-src ipc:`); images come through the `gmmimg` protocol.
- **R2a — Staying welcome on GameBanana.** No key, no account, an honest User-Agent, a small
  request budget, a response cache, and a shared pause when GameBanana says "slow down". See
  [gamebanana-api.md](gamebanana-api.md).
- **R3 — Filesystem scope.** All file access goes through `gmm-fsguard`, which refuses
  anything outside its registered roots, including through links or junctions.
  - **Read/write:** GMM's own folders, and `<XXMI>/<IMP>/Mods/` for each game.
  - **Read-only:** XXMI's config (allowed fields only), `d3dx.ini`, `d3d11.dll`'s version, the
    importer's core ini, and game folders (marker files only).
  - **Detection reads exact files only:** Steam's `libraryfolders.vdf` and
    `appmanifest_<appid>.acf`, HoYoPlay's `gamedata.dat` (only install paths kept), Epic's
    `*.item` manifests (only name and install location).
  - **Registry (read-only):** Steam's `SteamPath`, the install location of XXMI's uninstall
    entry, and GRYPHLINK's `install_path`. All registry code is in `gmm-detect/src/env.rs`.
  - **`d3dx_user.ini` is the one file GMM writes outside `Mods/`**: it only adds back saved
    toggle lines it copied from that same file, keeps a backup, writes atomically and never
    while the game runs.
  - GMM never writes to a game folder.
- **R4 — Never read secrets.** Denied by name even inside allowed folders: Steam login files
  (`loginusers.vdf`, `config.vdf`, `ssfn*`, `userdata/`), game logs, launcher login caches,
  browser profiles, `~/.ssh`. XXMI's config is parsed into a struct that only declares the
  fields GMM needs, so `github_token`, `proxy` and signatures are never read into memory.
- **R5 — The webview names nothing on disk.** Commands take ids. Folder and file pickers open
  from Rust, and Rust validates what they return. Dropped files reach Rust directly; the
  webview only learns the file name.
- **R6 — No debug bridges in release.** Devtools and test hooks need a `dev` feature; CI
  checks release builds don't have it.
- **R7 — Minimal process launching.** GMM starts three programs, each from one place, with fixed
  arguments: XXMI Launcher (`--nogui --xxmi <IMP>`, from `gmm-xxmi/src/launch.rs`), its own
  RAR helper (from `gmm-archive/src/helper.rs`), and on Windows, Windows Installer for its own
  update (System32's `msiexec.exe /i "<msi>" /passive /norestart AUTOLAUNCHAPP=1`, also from
  `launch.rs`, only after the installer's signature checked out; see T8). It also asks the OS to open a web page
  (`http(s)` only, shown in a confirmation first), a mod folder, or GMM's log folder (after a
  bug report opens, to attach the log). Nothing from a mod is ever
  run. `unsafe` code is allowed in three reviewed files only; CI checks both.
- **R8 — Confirmed, permanent deletes.** Every delete (uninstall, select-mode delete, clearing
  caches, folders an update replaces) goes through `Guard::delete` and needs a confirmation
  that lists each folder and its size. Hand-installed folders get a stronger warning, since
  GameBanana can't give them back. Deletes skip the Recycle Bin.

## Archive hardening

- The format comes from the file's first bytes, never its extension. Zip and 7z are decoded in
  process; RAR4/RAR5 go through the helper. Encrypted archives are refused.
- Every entry is checked before anything is written (two passes). Refused: absolute paths,
  `..`, drive letters, reserved Windows names, `:` streams, links, special files, and names
  that clash on a case-insensitive disk.
- Limits: 4 GiB total, 20,000 entries, a 200:1 compression ratio above 128 MiB (mostly blank textures
  compress far better than 200:1), paths up to 240
  characters, 16 levels deep. Real bytes are counted, so archives that lie about sizes are
  caught.
- **Policy scan:**
  - Blocked: `.exe .dll .bat .cmd .ps1 .vbs .js .wsf .sh .scr .msi .jar .com .lnk .reg`. The
    user can install without them; they're never extracted where XXMI loads from.
  - Warned about: `d3dx.ini` or `d3d11.dll` inside a mod, `.ini` files pointing outside the
    mod folder, unusually large files.
- **The RAR helper** (`gmm-extract`, RARLAB's UnRAR) gets the archive on stdin and no output
  folder. UnRAR runs in test mode and sends each entry back over a pipe; GMM checks and writes
  every file itself. On Windows it runs in a job object (one process, memory and CPU caps),
  with no privileges, at Low integrity, with process mitigations on. On Linux it runs with
  no-new-privileges and resource caps (no seccomp filter yet).

## Catalogue / UI hardening

- **Description HTML**: Rust replaces every image and link address with a key
  (`src-tauri/src/html.rs`), then DOMPurify in the webview keeps only safe tags and
  attributes (`components/RichText.tsx`). Links open only through a native confirmation.
- **Bundled Markdown** (guides, README sections, notices) is rendered into React elements; no
  HTML is parsed.
- **Capability**: the window may call GMM's own commands and nothing else (no Tauri `fs`,
  `shell`, `http`, `dialog`, event or window plugins).
- **CSP**: `default-src 'self'; img-src 'self' gmmimg: http://gmmimg.localhost data:;
  style-src 'self'; connect-src ipc: http://ipc.localhost; script-src 'self'; object-src
  'none'; frame-src 'none'; base-uri 'none'; form-action 'none'`, plus `freezePrototype`.
- **Navigation guard**: the window only shows GMM's own pages; any other navigation is refused.
- **Devtools** only with `--features dev`.

## Test harnesses

All run in CI.

| Harness | Proves |
|---|---|
| Malicious archive corpus (`gmm-archive/tests/corpus.rs`) and corruption tests | Bad archives are refused, never panic, never write outside their folder (T2) |
| RAR corpus (`gmm-extract/tests/rar.rs`, archives built byte by byte) | The same for RAR, through the real helper (T2, R7) |
| Property tests: paths, VDF parser, migoto ini parser | No panics on any input |
| Lint gates: clippy bans on `std::fs`, `Command::new`, HTTP clients, `winreg`; `cargo-deny`; `pnpm audit`; npm license check; `unsafe` and `Command::new` location checks | R1–R3 and R7 can't be bypassed quietly |
| XXMI config secret test (canary token, proxy password, signatures) | Secrets never appear in output, JSON, logs or the database (R4) |
| Detection on a fake machine, including hostile paths and a planted Steam login file | R3, R4 |
| **Egress harness** (`gmm-cli/tests/privacy.rs`): every command behind a logging proxy; on Linux also under `strace` | Only allowlisted hosts are contacted; local commands make no connection (R2) |
| **File-access harness** (same file): canary secrets in a fake profile | Nothing outside GMM's folders, `Mods/` and `d3dx_user.ini` changes; no canary is opened (R3, R4) |
| **IPC fuzz harness** (`src-tauri/src/ipc_fuzz.rs`): ~1,500 hostile calls through Tauri's real IPC path | No panic or hang, path tricks refused, nothing outside GMM's folders changes (R5) |
| Slow-down tests (`gmm-net`, `gmm-catalog`) | A 429 or challenge pauses every client (R2a) |
| Image cache and opener tests | Only GameBanana images get keys; only plain web addresses and real folders open (R2, R7) |
| Updater tests (`gmm-core` app_update, allowlist, `launch.rs`) | Only this key's signature of this version and this file passes, including one made by the release script; the updater's client reaches only GMM's installers; only an existing `.msi` goes to Windows Installer (T8, R2, R7) |
| Adapter conformance suite (`gmm-core/tests/conformance.rs`) | Detect → install → switch → conflicts → uninstall per game, with no writes outside `Mods/` |
| XSS corpus (`RichText.test.tsx`, `html.rs` tests) | Hostile HTML comes out with only allowed tags and keys (T3) |
| Component tests (Vitest) and the contrast audit (`tests/ui-audit.mjs`, Playwright) | Confirmations stay in place; text is readable in both themes |

`just audit-privacy` runs the egress and file-access harnesses against a release build
(set `GMM_PRIVACY_STRACE=1` on Linux to add `strace`).

Not done yet: `cargo-fuzz` targets, Windows equivalents of the `strace` checks, a seccomp
filter for the RAR helper.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting (the repository's Security tab), not a public
issue. PRs that add a network host, a filesystem root, a process launch, a Cargo feature or a
dependency with a build script need the `security-review` label.
