# 04 — Security & privacy

Goal: a user, or a skeptic, can **verify** that GMM touches only the files and hosts it says it
does. Promises in a policy aren't enough. A privacy policy that drifts away from what the code
does is the failure we design out.

## Threat model

| # | Threat | Example | Primary control |
|---|---|---|---|
| T1 | **The app itself overreaches** (our bug, feature creep, a compromised dependency) | Reads launcher/Steam login data, XXMI's GitHub token, home dir; phones home | R1–R5, fsguard, net allowlist, egress + file-access harnesses |
| T2 | **Malicious mod archive** | Zip-slip, symlink escape, zip bomb, bundled `.exe`/`.bat`, rogue `d3dx.ini`/`d3d11.dll` | Hardened extractor + policy scan |
| T3 | **Malicious catalogue content** | XSS in a mod description that then calls IPC | Sanitization, strict CSP, IPC takes ids not paths |
| T4 | **Download tampering / redirect abuse** | Redirect to another host, swapped file | Host allowlist after every redirect, size caps, GB AV status + MD5, local SHA-256 |
| T5 | **Executing the wrong program** | A planted "XXMI Launcher.exe" in a user-writable folder | Launch only the exe inside a confirmed XXMI root; first launch needs user confirmation; show the path |
| T6 | **Deep-link abuse** (Phase 4) | `gmm://install?url=https://evil/` | Deep links carry GB ids only, plus a confirmation dialog |
| T7 | **Supply chain** | Malicious crate/npm package | Lockfiles, `cargo-deny`, minimal deps, license + advisory checks |
| T8 | **Update channel compromise** | Fake GMM update | Signed Tauri updates, keys offline, GitHub Releases only |
| T9 | **Data loss** | Deleting the wrong mods | Deletes go to the Recycle Bin; unmanaged mods are never auto-deleted |
| T10 | **Account / ToS risk** | Anti-cheat flags modded client | Per-game risk notice; GMM never injects; no anti-cheat bypass, ever |

## Hard rules (enforced in code and CI)

- **R1 — No telemetry.** No analytics SDK, no crash uploader. Logs are local only, with the user
  profile path redacted to `~`. CI fails if a known telemetry crate or package shows up in a
  lockfile.
- **R2 — Network allowlist.** Only `gmm-net` opens connections (enforced with clippy
  `disallowed_types` on HTTP clients and sockets). Every URL and every redirect hop must be
  HTTPS on port 443 to a compiled-in host: `gamebanana.com`, `files.gamebanana.com`,
  `images.gamebanana.com`, or a numbered download mirror `filecache<digits>.gamebanana.com`.
  IP literals and URLs with credentials are refused. `github.com` and
  `objects.githubusercontent.com` get added only when GMM's own updater is built (Phase 5).
  A test-only feature that allows plain HTTP to `127.0.0.1` (for mock servers) is checked in CI
  never to reach a shipped binary. The webview makes **no** network
  requests: CSP `connect-src ipc: http://ipc.localhost`, and images arrive via the `gmmimg://`
  protocol served from the Rust cache.
- **R3 — Filesystem scope.** All I/O goes through `gmm-fsguard::ScopedPath`.
  - **Read/write:** GMM's app dirs (`%APPDATA%\GMM`, `%LOCALAPPDATA%\GMM`), and
    `<XXMI>/<IMP>/Mods/` for each supported importer.
  - **Read-only:** the rest of `<XXMI>/<IMP>/` (`d3dx.ini`, `d3d11.dll` version,
    `Core/<IMP>/main.ini`), XXMI's config file (allowlisted fields only, see below), and the game
    folders (marker files only).
  - **Detection read allowlist (exact files, implemented in `gmm-detect`):** Steam
    `steamapps/libraryfolders.vdf` and `steamapps/appmanifest_<appid>.acf`; HoYoPlay
    `%APPDATA%\Cognosphere\HYP\<version>\data\gamedata.dat` (only `installPath` values are
    extracted; the file also holds download transaction ids and settings, which are never kept
    or logged); Epic `Manifests/*.item` (only `DisplayName`, `InstallLocation`); later the
    Kuro `kr_starter_game.json`. Game folders: only marker files (exe names, required children)
    are checked.
  - **XXMI reads (`gmm-xxmi`):** the config file (allowlisted fields only, see R4), the launcher
    exe's first two bytes (`MZ` check), and per importer `d3dx.ini`, the core ini it includes,
    `d3d11.dll` (version resource), and the `Mods/` folder listing (names and sizes).
  - **Registry reads (exact values, read-only):** `HKCU\Software\Valve\Steam\SteamPath`, and for
    `Uninstall\*` keys whose `DisplayName` is `XXMI Launcher`, only `InstallLocation`. GMM never
    writes to the registry. All registry access sits in `gmm-detect/src/env.rs`; clippy bans
    `winreg::RegKey` everywhere else.
  - Canonicalize, check the prefix, refuse symlinks/junctions that leave the root. `std::fs` use
    outside `gmm-fsguard` fails CI.
  - **The database is the one file opened by a library.** `gmm-db` gets the path of
    `%APPDATA%\GMM\gmm.db` from `Guard::os_path_for_library`, which applies the same checks
    as a write (read-write root, denylist, no escape) and hands the path to SQLite. SQLite then
    opens that file and its `-wal`/`-shm` siblings in the same folder. SQLite extension
    loading is not compiled in.
  - **GMM never writes to a game folder.** XXMI owns game settings.
- **R4 — Never read secrets or personal data.** Denylisted by name, even inside allowed roots:
  - Steam `config/loginusers.vdf`, `config/config.vdf`, `local.vdf`, `ssfn*`, `userdata/`
  - Game logs (`Player.log`, `output_log.txt`) and launcher login caches
  - Browser profiles, `~/.ssh`, credential stores
  - XXMI config secrets: `github_token`, `proxy`, `Security.user_signature`, `*_signature`. The
    config is parsed into a struct that only declares the allowed fields, and the raw file is
    never logged, cached or copied.
- **R5 — The webview names nothing on disk.** IPC commands take opaque ids. The only paths that
  come from the UI are native folder-picker results, and Rust validates them before registering
  them as roots.
- **R6 — No debug or automation bridges in release.** Devtools and test hooks sit behind a `dev`
  feature that release CI asserts is off.
- **R7 — Process launching is minimal.** GMM starts exactly one kind of process: the XXMI
  Launcher exe inside a confirmed XXMI root, with fixed arguments (`--nogui --xxmi <IMP>`).
  "Open folder" uses the OS shell to open a directory, never a file. Nothing from a mod archive is
  ever executed.
- **R8 — Recoverable deletes.** Uninstall and storage deletes go through the Recycle Bin.
  Unmanaged folders are never deleted without an explicit, itemized confirmation.

## Archive hardening (gmm-archive)

- Detect the format from magic bytes, never the extension. Supported: zip, 7z (pure-Rust
  decoders only; 7z BZip2 is left out for licensing). Encrypted archives are rejected.
- Validate **every** entry's metadata before writing anything (two passes).
- Extract into a fresh temp dir under the app cache (`cache/extract/<id>`), then copy the
  chosen mod into `Mods/GMM/DISABLED .staging/<id>/` and rename it into place. On any error
  the destination is removed.
- Reject absolute paths, `..`, drive letters, NUL bytes, reserved Windows names (`CON`, `NUL`,
  `COM1`…), alternate data streams (`:`), and anything that normalizes outside the root.
- Reject symlinks, reparse points and special files inside archives, and names that collide
  on a case-insensitive file system (`a.ini` vs `A.INI`, or a file and a folder with the same
  name).
- Limits: total uncompressed ≤ 4 GiB, ≤ 20,000 entries, ratio ≤ 200:1 (per entry for zip, and
  for the whole archive) above a 16 MiB floor, path length ≤ 240, depth ≤ 16. Sizes are
  checked against the metadata first, then against the **actual** bytes written, so archives
  that lie about sizes are still caught. Nested archives are not auto-extracted.
- **Policy scan:**
  - Block by default: `.exe .dll .bat .cmd .ps1 .vbs .js .wsf .sh .scr .msi .jar .com .lnk .reg`.
    The user can override per mod (`gmm install --skip-blocked`, after the blocked files are
    listed by name). Overridden files are left out of `Mods/`: they stay only inside the
    cached archive and are never extracted anywhere GMM loads from, and never run. Only
    blocked files inside the chosen mod folder count; an installer `.exe` next to the mod
    folder in the archive is simply not copied.
  - Warn: `d3dx.ini` / `d3d11.dll` inside a mod (XXMI also flags rogue `d3dx.ini`), `.ini`
    references outside the mod folder (`..\`), unusually large files.
  - Scan report stored with the mod and shown on its library entry.
- RAR (C `unrar`) extraction runs in a subprocess with a restricted token / job object on Windows
  (seccomp on Linux). That's Phase 2 hardening; RAR is off until then.

## Catalogue / UI hardening

- `_sText` and `_sLicense` HTML is sanitized with DOMPurify (strict allowlist: no `style`, no
  iframes, no forms, no event handlers). Links open in the system browser after a confirmation.
- Tauri capabilities: the main window gets only our own commands. No `fs`, `shell` or `http`
  plugin permissions are exposed to JS.
- CSP: `default-src 'self'; img-src 'self' gmmimg: data:; connect-src ipc: http://ipc.localhost;
  script-src 'self'; object-src 'none'; frame-src 'none'`.

## Test harnesses, grown phase by phase

| Phase | Harness | Proves |
|---|---|---|
| 1 | Malicious archive corpus (`crates/gmm-archive/tests/corpus.rs`, archives built in code) incl. Windows-specific tricks; randomized corruption tests (`tests/robustness.rs`) | Extractor rejects each case, never panics, never writes outside its root |
| 1 | Property tests (✅ done): `RelPath`, archive corruption, VDF parser never panics. `cargo-fuzz` targets (extractor, migoto ini parser, VDF/ACF, GB JSON, XXMI config) as a nightly job: ⏳ | No panics or escapes |
| 1 | Lint gates (✅ done): clippy `disallowed_methods` (`std::fs`, `Command::new`) and `disallowed_types` (HTTP clients, raw sockets, `winreg::RegKey`) outside their crates; `cargo-deny`. ⏳ `cargo-audit`, `pnpm audit` | R1–R3, R7 can't be bypassed silently |
| 0b | **XXMI config secret test** (✅ done): fixture configs with canary `github_token`, proxy password and signatures; asserts they never appear in parsed config, `GameStatus` JSON or `Debug`, and parse errors don't echo content. Extend to logs, DB and IPC output as those appear | R4 |
| 0b | Detection on a fake machine (✅ done): Steam libraries, HoYoPlay data with a default path for an uninstalled game, Epic manifest, hostile `installdir` (`..\..\Windows`), planted `loginusers.vdf` canary | R3/R4, correct detection |
| 2 | **Egress harness**: run the CLI scenario behind a logging proxy, with all other outbound traffic blocked (Windows Firewall rule on the test binary in CI; network namespace on Linux CI). Assert every host ∈ allowlist | R2 |
| 2 | **File-access harness**: run the scenario with a fake profile full of canary files (Steam `loginusers.vdf`, `Player.log`, browser profile). Trace with Process Monitor (Windows, scripted) or `strace` (Linux). Fail if any canary is opened, or any write lands outside the write roots | R3/R4 |
| 2 | IPC fuzz: hostile args to every Tauri command | R5 |
| 2 | Adapter conformance suite on fixtures (per-state fixtures ✅ done in 0b; install/toggle/uninstall ⏳) | Correct detection, no writes outside `Mods/` |
| 3 | XSS corpus through the description renderer | T3 |
| 4 | Hostile deep-link URLs | T6 |
| 5 | Release checks: feature-flag assertion (R6), update signature verification, installer contents review | T7/T8 |

`just audit-privacy` runs the egress and file-access harnesses locally against a release build,
so anyone can reproduce them.

## Process

- `SECURITY.md` with private reporting (GitHub Security Advisories).
- PRs that add a host, a filesystem root, a process launch, a Cargo feature or a dependency with
  build scripts need a `security-review` label and a check against this doc.
- The harness allowlists are generated from the same constants the code uses, so this doc, the
  code and the tests can't drift apart.
