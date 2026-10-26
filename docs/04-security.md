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
| T6 | **Deep-link abuse** (deferred to "Later" in the roadmap, 2026-10-26) | `gmm://install?url=https://evil/` | Deep links carry GB ids only, plus a confirmation dialog |
| T7 | **Supply chain** | Malicious crate/npm package | Lockfiles, `cargo-deny`, minimal deps, license + advisory checks |
| T8 | **Update channel compromise** | Fake GMM update | Signed Tauri updates, keys offline, GitHub Releases only |
| T9 | **Data loss** | Deleting the wrong mods | Nothing is deleted without an itemized confirmation (stronger for hand-installed mods); unmanaged mods are never auto-deleted; deletes are permanent by the user's choice (R8) |
| T10 | **Account / ToS risk** | Anti-cheat flags modded client | Per-game risk notice; GMM never injects; no anti-cheat bypass, ever |

## Hard rules (enforced in code and CI)

- **R1 — No telemetry.** No analytics SDK, no crash uploader. Logs are local only, with the user
  profile path redacted to `~`. CI fails if a known telemetry crate or package shows up in a
  lockfile. The one log (2026-10-26, `gmm_core::ErrorLog`, `%APPDATA%\GMM\logs\`) holds
  errors only, one capped line each, rotated at 256 KiB; the home folder in every spelling
  (`\`, `/`, JSON-escaped, `\\?\`) becomes `~` and the account name as a path part elsewhere
  becomes `<user>` (unit-tested). Report a Bug sends nothing: GMM builds a pre-filled
  GitHub issue address (the user's text, versions and setup states, never a path), opens it
  in the browser only after a native confirmation, and the user attaches the log by hand.
- **R2 — Network allowlist.** Only `gmm-net` opens connections (enforced with clippy
  `disallowed_types` on HTTP clients and sockets). Every URL and every redirect hop must be
  HTTPS on port 443 to a compiled-in host: `gamebanana.com`, `files.gamebanana.com`,
  `images.gamebanana.com`, or a numbered download mirror `filecache<digits>.gamebanana.com`.
  IP literals and URLs with credentials are refused. `github.com` and
  `objects.githubusercontent.com` get added only when GMM's own updater is built (Phase 7).
  A test-only feature that allows plain HTTP to `127.0.0.1` (for mock servers) is checked in CI
  never to reach a shipped binary. `Allowlist::deny_all()` (2026-10-25) refuses every
  address before a connection; it can only take hosts away, and the IPC fuzz harness uses it. The webview makes **no** network
  requests: CSP `connect-src ipc: http://ipc.localhost`, and images arrive via the `gmmimg`
  protocol served from the Rust cache (`gmm_core::ImageCache`: only HTTPS URLs on
  `images.gamebanana.com` that a command handed out as opaque keys, fetched through `gmm-net`).
- **R3 — Filesystem scope.** All I/O goes through `gmm-fsguard::ScopedPath`.
  - **Read/write:** GMM's app dirs (`%APPDATA%\GMM`, `%LOCALAPPDATA%\GMM`, or `GMM_HOME` when
    that environment variable names an absolute folder), and `<XXMI>/<IMP>/Mods/` for each
    supported importer.
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
  - **A local archive's folder is read once** (Phase 6, installing from a file the user dropped
    or picked): that folder is registered read-only on a separate guard for the one copy into
    GMM's cache (a regular `.zip`/`.7z`/`.rar` file, size-capped), and nothing else in it is
    read. The path comes from the OS drop event or a native dialog, both handled in Rust (R5).
  - **`d3dx_user.ini` is the one file GMM writes outside `Mods/`** (amended 2026-10-25, the
    project owner's decision). 3DMigoto keeps each mod's saved in-game toggles there, keyed by
    the mod's path, and drops them while the mod is switched off (`DISABLED `), so GMM keeps
    them across a switch (`gmm-core/src/toggles.rs`). Fenced in: the file is reached through a
    guard of its own whose only path is `<IMP>/d3dx_user.ini` (nothing else in `<IMP>/` becomes
    writable); GMM copies a mod's lines into its database before switching it off and only ever
    *adds* lines it saved from that same file, never changing or removing one; a copy of the
    old file goes to `cache/backups/d3dx_user/` before each write (the newest ten are kept);
    the write is atomic (a temporary sibling, then a rename); and nothing is written while the
    game is running (checked by process image name, `gmm_xxmi::game_running`): the write-back
    then waits and happens before GMM's next launch of the game or at the next switch. The
    file-access harness allows exactly this file (and its temporary sibling) and checks that a
    switch off and on puts the dropped line back.
- **R4 — Never read secrets or personal data.** Denylisted by name, even inside allowed roots:
  - Steam `config/loginusers.vdf`, `config/config.vdf`, `local.vdf`, `ssfn*`, `userdata/`
  - Game logs (`Player.log`, `output_log.txt`) and launcher login caches
  - Browser profiles, `~/.ssh`, credential stores
  - XXMI config secrets: `github_token`, `proxy`, `Security.user_signature`, `*_signature`. The
    config is parsed into a struct that only declares the allowed fields, and the raw file is
    never logged, cached or copied.
- **R5 — The webview names nothing on disk.** IPC commands take opaque ids. The only paths that
  enter are native folder-picker results, opened from Rust (`tauri-plugin-dialog`'s Rust API;
  the webview has no dialog permission), and Rust validates them before saving them. Two
  arguments look like paths but are only compared, never opened: an unmanaged mod's `path`
  (must equal an entry of a fresh library listing exactly) and the folder given to
  `remove_game_folder` (must equal a stored row). Image URLs never reach the webview either:
  it gets `gmmimg` keys. Files dropped on the window (Phase 6) reach Rust through Tauri's
  window drop event; the webview only learns the file's name. Profile share codes are the one
  free text the webview sends besides names and search text: `gmm-core` treats them as
  untrusted (64 KiB, 1,000 mods, 200-character names, known game, versioned format) and they
  only ever match against mods already installed.
- **R6 — No debug or automation bridges in release.** Devtools and test hooks sit behind a `dev`
  feature that release CI asserts is off.
- **R7 — Process launching is minimal.** GMM starts exactly two kinds of process, each from
  one place, both with fixed arguments. (1) The XXMI Launcher exe inside a confirmed XXMI
  root, with `--nogui --xxmi <IMP>`. (2) Its own RAR helper, `gmm-extract`, from the folder of
  the running program, with one argument (`list` or `extract`), an empty environment and the
  archive (opened read-only through `gmm-fsguard`) as its stdin; never a path from a download.
  The helper is started only in `crates/gmm-archive/src/helper.rs` and sandboxed (see
  "Archive hardening" below).
  Two more things are handed to the OS shell, from the same module: `open_folder` opens a
  directory in the file manager (Windows: `ShellExecuteExW` with the `explore` verb, which
  doesn't apply to files; elsewhere `xdg-open` with the one argument), and it refuses
  anything but an existing absolute directory; the app only asks for a mod's folder inside a
  Mods folder, resolved with `Guard::os_path_of_dir`. `open_page` opens an `http(s)` address
  in the default browser (verb `open`); it refuses other schemes, addresses without a host,
  spaces, quotes, angle brackets, control characters and anything over 2,048 characters.
  The app builds those addresses itself (`gamebanana.com/mods/<id>`, XXMI's releases page)
  or looks them up from a link key it handed out in a rewritten description, and shows each
  one in a native dialog from Rust first; the webview can neither name the address nor answer
  the dialog. Nothing from a mod archive is
  ever executed. All launching is in `crates/gmm-xxmi/src/launch.rs`; the exe path is fixed
  relative to the validated root and the arguments come from the importer id in code (refused
  if one would need quoting). `gmm-core::start` refuses an exe the user hasn't confirmed
  (`gmm launch`, first run). XXMI's launcher requires administrator rights (its manifest says
  `requireAdministrator`; a plain process start fails with Windows error 740), so on Windows
  it is started with `ShellExecuteExW` and the default verb, like Explorer starting XXMI's own
  Quick Start shortcut: Windows shows its UAC prompt, and declining it is reported, not retried.
  The same module also answers "is the game running?" for the `d3dx_user.ini` write (R3):
  a Windows ToolHelp snapshot of process image names, compared with the manifest's exe names;
  nothing else about other processes is read or kept (always "no" elsewhere for now).
  Reviewed `unsafe` code is limited to three files: this module, the helper's job object
  (`gmm-archive/src/helper.rs`) and the helper itself (`gmm-extract/src/main.rs` with its
  `ffi` and `lockdown` modules: the UnRAR calls and the helper's own lockdown). The workspace
  sets `unsafe_code = "deny"`; CI fails if `allow(unsafe_code)` appears in any other file, and
  if `Command::new` appears outside `gmm-xxmi/src/launch.rs` and `gmm-archive/src/helper.rs`
  (tests excepted). Other platforms start XXMI with `std::process::Command` in the launch
  module (past the clippy ban there and in the helper module only).
- **R2a — Staying welcome on GameBanana** ([10](10-gamebanana-api.md)). No key, no account,
  an honest User-Agent. One API budget per install (about 2 requests/s, background reads
  included), merged duplicate requests, the response cache, and saved answers when
  GameBanana can't answer. A 429, a 503 with `Retry-After` or a Cloudflare challenge pauses
  every GameBanana client of the app at once (`gmm-net/src/cooldown.rs`) and is never
  retried in a loop; GMM never tries to pass a challenge.
- **R8 — Confirmed, thorough deletes** (amended 2026-10-25, the project owner's decision:
  an uninstall the user confirmed should leave nothing behind, and filling the Recycle Bin
  only makes them clean up again). Every delete GMM makes for the user (uninstall, the
  Library's select-mode delete, clearing the download cache, the previous folders an update
  or roll back replaces) is **permanent**, through `gmm_fsguard::Guard::delete`, which applies
  the same read-write-root checks as any write, refuses a root itself, removes links without
  following them and clears read-only attributes first. Uninstalling the last version of a
  GameBanana mod also deletes its archives from the download cache. Nothing is deleted
  without a confirmation that lists what goes: the app's dialog shows each mod, its folder
  and size and says the delete is permanent; hand-installed folders get a stronger warning
  (GameBanana can't give them back). `gmm uninstall <name>` resolves every name first, lists
  each folder with its full path, file count and size, and asks `[y/N]`; without a terminal
  only `--yes` confirms. The removal then takes the exact listed path, nothing looser.

## Archive hardening (gmm-archive)

- Detect the format from magic bytes, never the extension. Supported: zip, 7z (pure-Rust
  decoders in process; 7z BZip2 is left out for licensing) and RAR4 / RAR5 (the `gmm-extract`
  helper, below). Encrypted archives are rejected.
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
    listed by name; in the app, "Install anyway" on the Downloads review that lists them).
    Overridden files are left out of `Mods/`: they stay only inside the
    cached archive and are never extracted anywhere GMM loads from, and never run. Only
    blocked files inside the chosen mod folder count; an installer `.exe` next to the mod
    folder in the archive is simply not copied.
  - Warn: `d3dx.ini` / `d3d11.dll` inside a mod (XXMI also flags rogue `d3dx.ini`), `.ini`
    references outside the mod folder (`..\`), unusually large files.
  - Scan report stored with the mod and shown on its library entry.
- **RAR** (Phase 5) is read by the `gmm-extract` helper process: RARLAB's UnRAR 7.23, vendored
  and built from source. Design and the libarchive / UnRAR choice:
  [09](09-archive-formats.md#rar-plan-decision-record).
  - The helper gets the archive as stdin and no path or output folder; it reopens only that
    file (by the name of its stdin handle) because UnRAR reads archives by name.
  - Windows: GMM starts it suspended, puts it in a job object (one process, 2 GiB committed
    memory, 10 minutes of CPU, no desktop / clipboard / global-atom / system-setting access,
    killed when GMM drops the job) and only then resumes it. Before reading a byte the
    helper removes all its privileges, lowers itself to Low integrity (no writes to the
    user's files or registry) and turns on process mitigations (no dynamic code, no
    extension-point DLLs, no remote or low-label images, strict handle checks). If any step
    fails it refuses to run.
  - Linux: no-new-privileges, a file-size cap of 0 (no file writes), address-space and CPU
    caps. ⏳ seccomp (Phase 7 hardening).
  - UnRAR runs in test mode, so it never writes a file: every entry and its data come back
    over a pipe as frames (`gmm_archive::rar_protocol`, every length bounded before
    allocating). GMM checks the whole list first (pass 1, the same checks as zip and 7z),
    then a second run streams the data and GMM writes each file itself through
    `gmm-fsguard`, counting real bytes against the limits. The entries must match between
    the passes, and anything unexpected from the helper ends the extraction.
  - Refused: links of every kind (symlinks, junctions, hard links, "file copy" references),
    special files, encrypted entries or headers, multi-part archives, and RAR 7 dictionaries
    over 4 GB.
  - Residual risk, accepted for now: a Low-integrity Windows process can still read files the
    user can read and open network connections. An AppContainer would remove both
    (Phase 7 hardening).

## Catalogue / UI hardening

- `_sText`, `_sLicense` and update HTML go through two layers before display (T3). In Rust
  (`src-tauri/src/html.rs`) every address leaves the HTML: image `src`s become `gmmimg`
  URLs (GameBanana's image host only; other images lose their `src`), `http(s)` link
  `href`s become `#gmm-link:<key>`, other `href`s and every `srcset` are dropped; values are
  entity-decoded before they're judged and escaped when written back. Unit tests cover
  entity-encoded `javascript:`, unquoted and single-quoted values, comments, broken tags and
  escaping. In the webview, `components/RichText.tsx` runs DOMPurify 3.4.17 on its own
  instance with an allowlist (text formatting, lists, headings, tables, `a`, `img`;
  attributes `href`, `src`, `alt`, `title`, sizes; no `style`, `class`, data or ARIA
  attributes, no forms, frames or scripts), a URI pattern that only admits those two
  shapes, and a hook that removes any `img` whose `src` isn't exactly a `gmmimg` key and
  any `href` that isn't exactly a link key. Clicks never navigate: a link key goes to
  `open_link`, which shows the real address before opening it (R7). `RichText` is the one
  place allowed past Biome's `noDangerouslySetInnerHtml` (with a reason). The CSP stays the
  last line. Still to come: the Vitest XSS corpus through `sanitize` (Phase 5 tests).
- **Bundled Markdown** (2026-10-25): the Documentation guides (`docs/guides/`), the
  README's Credits and Disclaimer and THIRD-PARTY-NOTICES.md are compiled into the app
  (`?raw`) and rendered by `src/markdown.tsx`, which builds React elements only (no HTML is
  ever parsed or injected). Links to another guide open it in the app; every other address
  is shown as plain text (its `title`), never opened or loaded.
- **Stored previews** (2026-10-25): `installed_mods.preview_url` keeps a GameBanana image
  address from the mod page; `library` registers it with the image cache and hands the
  webview only the `gmmimg` key (R5), and the cache still refuses anything off GameBanana's
  image hosts. Settings' image Clear (`clear_image_cache`) deletes only files named like
  cache keys inside `cache/images/`.
- **Capability** (`apps/desktop/src-tauri/capabilities/main.json`): the main window gets
  exactly GMM's own commands (`allow-<command>`, generated from the list in `build.rs`), and
  nothing else: not `core:default`, no event, window, `fs`, `shell`, `http` or `dialog`
  permissions. Checked in the running app (2026-10-24): `plugin:dialog|open` and
  `plugin:event|listen` are refused. Install progress uses a Tauri `Channel`, whose data
  fetch Tauri exempts from the ACL. `tauri-plugin-dialog` brings `tauri-plugin-fs` in as a
  crate dependency, but no fs permission is granted. The custom title bar (2026-10-26) uses
  GMM's own window commands (`window_minimize`, `window_toggle_maximize`,
  `window_is_maximized`, `window_close`, `window_start_drag`; no arguments, each acts on the
  calling window only) instead of Tauri's window plugin, so `plugin:window|close` is still
  refused.
- **CSP** (`tauri.conf.json`): `default-src 'self'; img-src 'self' gmmimg:
  http://gmmimg.localhost data:; style-src 'self'; connect-src ipc: http://ipc.localhost;
  script-src 'self'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'`,
  plus `freezePrototype`. `http://gmmimg.localhost` is how WebView2 addresses the custom
  protocol. Checked in a release build (2026-10-24): a `fetch` to gamebanana.com, an image
  from images.gamebanana.com and an injected inline script are all blocked; `gmmimg` images
  load. The CSP only applies to pages Tauri serves, so `tauri dev` (Vite's server) runs
  without it; `devCsp` is set for when Tauri applies it. Check CSP changes with a release
  build.
- **Navigation guard** (`main.rs`): the webview may only show GMM's own pages
  (`http://tauri.localhost` / `tauri://localhost`, and the Vite server in debug builds). Any
  other navigation, such as a link in a mod description, is refused. Checked: clicking a
  gamebanana.com link leaves the window where it was.
- **Devtools** only with `--features dev` (`tauri/devtools`); CI fails if `tauri/devtools`
  is enabled without it (R6). Note that WebView2 itself accepts a remote-debugging port from
  the `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` environment variable in any build; that needs
  control of the user's environment, which is outside the threat model.
- No `style` attributes in the UI (`style-src 'self'`); use classes. The one exception is a
  value only known at runtime (the progress bar's width), set through React's `style` prop,
  which writes a CSSOM property; the CSP allows that and still blocks style attributes in
  markup. The UI font is bundled, so `font-src` needs nothing beyond `'self'`.

## Test harnesses, grown phase by phase

| Phase | Harness | Proves |
|---|---|---|
| 1 | Malicious archive corpus (`crates/gmm-archive/tests/corpus.rs`, archives built in code) incl. Windows-specific tricks; randomized corruption tests (`tests/robustness.rs`) | Extractor rejects each case, never panics, never writes outside its root |
| 5 | **RAR corpus** (✅ done, `crates/gmm-extract/tests/rar.rs`, RAR5 and RAR4 archives built byte by byte, run through the real helper): traversal and drive-letter names, symlinks, hard links, Unix special files, case collisions, entry / depth / path-length limits, declared and actual size limits, encrypted headers and files, truncated and corrupted archives, a missing helper, and the helper refusing any other arguments. The frame reader's bounds have unit tests (`rar_protocol.rs`). Real mods checked by hand (2026-10-24): five RAR files from GameBanana, 0.7 to 113 MiB, all extracted | T2, R7 |
| 1 | Property tests (✅ done): `RelPath`, archive corruption, VDF parser and (Phase 2) the migoto ini parser never panic. `cargo-fuzz` targets (extractor, migoto ini parser, VDF/ACF, GB JSON, XXMI config) as a nightly job: ⏳ | No panics or escapes |
| 1 | Lint gates (✅ done): clippy `disallowed_methods` (`std::fs`, `Command::new`) and `disallowed_types` (HTTP clients, raw sockets, `winreg::RegKey`) outside their crates, including the desktop crate; `cargo-deny` (advisories included); Phase 3: `pnpm audit` and the npm license check (`scripts/check-npm-licenses.mjs`) in CI; Phase 5: CI checks that `allow(unsafe_code)` stays in its three reviewed files and `Command::new` in its two launch places | R1–R3, R7 can't be bypassed silently |
| 0b | **XXMI config secret test** (✅ done): fixture configs with canary `github_token`, proxy password and signatures; asserts they never appear in parsed config, `GameStatus` JSON or `Debug`, and parse errors don't echo content. Phase 2: the conformance suite checks the DB file, cache and mod folders too. ⏳ logs and IPC output when those exist | R4 |
| 0b | Detection on a fake machine (✅ done): Steam libraries, HoYoPlay data with a default path for an uninstalled game, Epic manifest, hostile `installdir` (`..\..\Windows`), planted `loginusers.vdf` canary | R3/R4, correct detection |
| 2 | **Egress harness** (✅ done, `crates/gmm-cli/tests/privacy.rs`): the real `gmm` binary runs every network command behind an in-test proxy that logs and refuses each request. Every `CONNECT` target must pass `Allowlist::production()` on port 443, at least one must be seen, and local commands (detect, status, config, list, conflicts, storage, launch, and since 2026-10-26 `report-bug` and `log`) must make none. On Linux CI each `gmm` process also runs under `strace`, and any IPv4/IPv6 connection that isn't loopback (traffic bypassing the proxy) fails the run. ⏳ a Windows equivalent of the bypass check (firewall rule) | R2 |
| 2 | **File-access harness** (✅ done, same test): a fake profile with canary files (Steam `loginusers.vdf`, `config.vdf`, `userdata/`, a game `Player.log`, a browser `logins.json`, `~/.ssh/id_rsa`) and secrets in XXMI's config; GMM's folders redirected with `GMM_HOME`. Asserts nothing in the profile outside `Mods/` changed except `d3dx_user.ini` (2026-10-25: a hand-installed mod is switched off, its toggle line dropped as 3DMigoto would, and switched on again; the line must be back and the rest of the file unchanged, or, when ZZZ is running on the test machine, the file must stay exactly as the "game" left it), and no canary content reached GMM's files. Under `strace` (Linux CI) it also fails if any canary is opened or stat-ed, or any write lands outside `GMM_HOME`, `Mods/` and `d3dx_user.ini` (with its temporary sibling). ⏳ Process Monitor on Windows | R3/R4 |
| 5 | **IPC fuzz harness** (✅ done 2026-10-25, `apps/desktop/src-tauri/src/ipc_fuzz.rs`): every command, through Tauri's real IPC path (the mock runtime, the app's own context and capability, the real command list from `handler()`), against a fake XXMI setup in a temporary folder with GMM's folders beside it and a network allowlist that refuses everything (`Allowlist::deny_all`). Each command gets no arguments, a body that isn't an object, and each argument replaced by hostile values (wrong JSON types, negative and overflowing numbers, 256 KiB strings, traversal, absolute and `\\?\` paths, near misses of a real folder name: other case, trailing dot or separator, leading space, `DISABLED ` prefix, a NUL; JSON nested past serde's limit; oversized share codes; 10,000 keys). About 1,500 calls. Asserts: every call answers (no panic, no hang), path tricks are refused, nothing outside GMM's folders changed (the Mods folder, `d3dx_user.ini` and a canary beside it compared byte for byte), every command passes the capability while plugin and unknown commands are refused, and every command in `build.rs` is covered or listed as left out (the folder and archive pickers and `open_xxmi_releases`, which open native dialogs, `open_log_folder`, which opens the file manager, `clear_xxmi_folder`, and the window commands, which act on the window itself and take no arguments; `open_bug_report` only gets shapes refused before its dialog). Since 2026-10-26 the Report a Bug commands and `queue_proceed` are in it too. A check that the fake setup is Ready keeps it from passing by stopping early; an injected real delete makes it fail. CI checks that `tauri/test` never reaches the shipped build | R5 |
| 2 | Slow-down tests (✅ `gmm-net`, `gmm-catalog`): a 429 pauses every sibling client with no further request, a Cloudflare challenge counts, a plain 403 doesn't, a download keeps its partial file, saved answers stand in (never for a 404), identical requests are fetched once | R2a |
| 6 | Share-code tests (✅ `gmm-core/tests/core.rs`): garbage, a newer format, another game, an unknown game, an empty name and an oversized code are refused | R5 |
| 3 | Image cache tests (✅ done, `gmm-core/src/images.rs`): only HTTPS URLs on `images.gamebanana.com` and GameBanana's file hosts get keys (look-alike hosts refused); bad keys, SVG/HTML bodies and HTTP errors are refused and nothing is written; cache survives restarts; oldest images pruned at the cap | R2, R5 |
| 3 | `bindings_are_current` (✅ done, `gmm-desktop`): the TypeScript types match the Rust types | Bridge types can't drift |
| 2 | Adapter conformance suite on fixtures (✅ done: per-state fixtures in 0b; `gmm-core/tests/conformance.rs` runs detect → install → toggle → conflicts → storage → uninstall per game, asserts no file outside `Mods/` changed and no XXMI config canary reached GMM's database, cache or mod folders) | Correct detection, no writes outside `Mods/`, R4 for the database |
| 5 | XSS corpus through the description renderer. Rust side ✅ (`src-tauri/src/html.rs` unit tests: entity-encoded schemes, quoting styles, comments, broken tags, escaping); checked live 2026-10-25 on ten real descriptions (no outside address left in any attribute). Webview side ✅ 2026-10-25 (`src/components/RichText.test.tsx`, Vitest): 46 hostile inputs (scripts, event handlers, `javascript:` and `data:` in every spelling, SVG and MathML, frames, forms, `meta` / `base` / `link`, styles, mutation-XSS shapes) must come out with only allowed tags and attributes, images only as `gmmimg` keys and links only as link keys. It found that `width`, `height`, `colspan` and `rowspan` were always stripped (DOMPurify judged them as addresses); fixed with `ADD_URI_SAFE_ATTR` | T3 |
| 5 | Component tests (✅ 2026-10-25, Vitest + Testing Library on jsdom, `apps/desktop/src/**/*.test.tsx`) against a fake bridge (`src/test/fakeBridge.ts`) answering from synthetic fixtures typed against `bindings.ts`: the Library (cards, version groups, sensitive filter, conflict prompt, select-mode delete, Versions section), the install confirmation (variants picked before any download, one queued install per file, the conflict choice; since 2026-10-26), the review of an install that needs action (versions, blocked files, going ahead or cancelling), the storage table, the slideshow, the Markdown renderer (no markup from the source), a snapshot of the mod page header and license | UI keeps its safety rules (R8 confirmations, ids only) |
| 5 | Contrast and overlap audit (✅ 2026-10-25, `apps/desktop/tests/ui-audit.mjs`, Playwright + Chromium in CI): 22 states in both themes (every page, dialogs, panels, first run; since 2026-10-26 also a Downloads row that needs action, its review and the confirmation's conflict and variant steps) with the fake bridge; WCAG AA contrast composited through translucent layers (text on artwork skipped), no overlapping text after clipping, no sideways scroll. A self-check plants one problem of each kind first and fails if any goes unreported | Readability in both themes |
| 5 | Shell openers (✅ `gmm-xxmi/src/launch.rs` unit tests): `open_page` refuses `file:`, `javascript:`, `ms-settings:`, Windows paths, host-less addresses, spaces, quotes, control characters and over-long addresses; `open_folder` refuses relative paths, files and missing folders | R7 |
| 6 | Error log and bug report tests (✅ 2026-10-26, `gmm-core/src/errlog.rs`, `report.rs`): the home folder and account name are removed in every path spelling while a mod named like the user stays; entries stay one capped line; the log rotates and clears; the issue address starts with the fixed form, round-trips the body, is cut to fit `MAX_PAGE_URL` and passes `check_page_url` | R1, R7 |
| Later | Hostile deep-link URLs (with deep links, deferred 2026-10-26) | T6 |
| 7 | Release checks: feature-flag assertion (R6), update signature verification, installer contents review | T7/T8 |

`just audit-privacy` runs the egress and file-access harnesses locally against a release build
(with strace on Linux when `GMM_PRIVACY_STRACE=1` is set),
so anyone can reproduce them.

## Process

- `SECURITY.md` with private reporting (GitHub Security Advisories).
- PRs that add a host, a filesystem root, a process launch, a Cargo feature or a dependency with
  build scripts need a `security-review` label and a check against this doc.
- The harness allowlists are generated from the same constants the code uses, so this doc, the
  code and the tests can't drift apart.
