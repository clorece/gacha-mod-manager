# Architecture

## Stack

| Part | Choice |
|---|---|
| App shell | Tauri 2 (WebView2 on Windows, WebKitGTK on Linux), MSI installer |
| Core logic | Rust workspace: every file, archive, network and process action happens here |
| UI | React 19 + TypeScript + Vite, Tailwind 4, Nunito (bundled); primitives built by hand in `src/ui` |
| UI ↔ Rust types | `ts-rs` generates `apps/desktop/src/bindings.ts`; a test fails when it's stale |
| Database | SQLite through `sqlx`, migrations in `crates/gmm-db/migrations/` |
| Archives | `zip`, `sevenz-rust2` in process; RAR through UnRAR in a sandboxed helper |
| HTTP | `reqwest` + rustls, wrapped by `gmm-net` (host allowlist) |

The `gmm` command line uses the same crates as the app. Every feature works from the CLI.

## What GMM does and doesn't do

| GMM does | GMM doesn't |
|---|---|
| Browse and search GameBanana | Host or re-upload mods |
| Download, check and extract mods into `<XXMI>/<IMP>/Mods/` | Inject DLLs or touch game processes |
| Switch mods on and off, find conflicts, show storage | Edit game files or game settings |
| Detect games and XXMI's setup, explain fixes | Install or update XXMI |
| Start XXMI (`--nogui --xxmi <IMP>`) | Run anything that comes from a mod |

## Crates

| Crate | Job |
|---|---|
| `gmm-fsguard` | All file access: allowed roots, safe relative paths, link-escape checks, a denylist for secrets |
| `gmm-net` | All network access: GameBanana-only allowlist (checked on every redirect), rate limits, one shared "slow down" pause, resumable verified downloads |
| `gmm-archive` | Safe zip/7z/RAR extraction (two passes, size limits) and the policy scan; starts the RAR helper |
| `gmm-extract` | The RAR helper: a separate sandboxed program with UnRAR that streams entries back and never writes a file |
| `gmm-catalog` | GameBanana client: lists, search, mod pages, files, updates, categories, trending, counters |
| `gmm-adapters` | The game manifests (`games/*.toml`) and their validation |
| `gmm-detect` | Finds games: Steam, HoYoPlay, GRYPHLINK, Epic, manual folders |
| `gmm-xxmi` | Reads XXMI's setup (allowed fields only), works out each game's setup state, launches XXMI |
| `gmm-migoto` | Reads 3DMigoto `.ini` files: override hashes, namespaces, key bindings |
| `gmm-db` | The SQLite database |
| `gmm-core` | The features: install pipeline and queue, library, versions, updates and roll back, profiles, adopt, conflicts, storage, launch, image cache, error log, bug report |
| `gmm-cli` | The `gmm` command line |
| `gmm-desktop` (`apps/desktop/src-tauri`) | The app: one Tauri command per CLI action, the install queue worker, the `gmmimg` image protocol, window frame. React UI in `apps/desktop/src` |

## Install & toggle model

GMM installs straight into XXMI's Mods folder, under its own `GMM/` subfolder. XXMI's
`d3dx.ini` already loads nested folders (`include_recursive = Mods`):

```
<XXMI>/ZZMI/Mods/
  GMM/
    Ellen - Maid Outfit [gb-652373]/        ← on
      gmm.json                              ← GMM's record of the mod
    DISABLED Miyabi - Casual [gb-610001]/   ← off (XXMI skips DISABLED*)
  SomeHandInstalledMod/                     ← installed by hand, left alone
```

- **On/off** is a rename with or without the `DISABLED ` prefix, XXMI's own convention.
- **Variants** (several mods in one archive) each get their own folder, so they switch
  independently.
- **Installs are atomic**: files are staged in `Mods/GMM/DISABLED .staging/` and renamed into
  place. Leftovers from a crash are cleaned up on the next run.
- **Folder names** are `<name> [gb-<id>]`, cleaned of characters Windows forbids and capped at
  80 characters. GMM never overwrites an existing folder.
- **Folder names must stay stable.** 3DMigoto saves each mod's in-game toggles in
  `<IMP>/d3dx_user.ini`, keyed by the mod's path, and drops keys whose mod is gone. So GMM
  picks a folder name once and only ever renames it to switch it on or off.
- **Saved toggles survive a switch** (`gmm-core/src/toggles.rs`): before switching a mod off,
  GMM copies its lines from `d3dx_user.ini`; after switching it on, it adds back any that are
  missing. A backup of the file is kept, and nothing is written while the game runs.
- **Uninstall** deletes the folder for good, after a confirmation that lists it. Uninstalling a
  mod's last version also deletes its cached archives.
- **Update / roll back** rebuild the mod from another file and swap it in under the same folder
  name and on/off state. The old archive stays cached, so a roll back downloads nothing.
- **Profiles** are named sets of switched-on mods per game.
- **Adopt** turns a hand-installed folder into a managed mod (it gets a `gmm.json` and a
  database row).
- **The disk is the truth for on/off.** A folder renamed by hand updates the database; a
  missing folder shows as missing.

## Install flow

```
setup check: XXMI and the game's importer must exist
mod page ─► refuse trashed, withheld or obsolete mods, and mods for another game
pick a file ─► GameBanana's virus scan must say "clean"
          ─► download into the cache (allowlisted hosts only, exact size, MD5 when given)
          ─► extract into cache/extract/<id> with all the hardening rules ─► policy scan
          ─► find mod roots: folders holding an .ini with [TextureOverride or [ShaderOverride
          ─► one root = one mod; several = variants (the user picks)
          ─► blocked files stop the install unless the user leaves them out
          ─► stage ─► gmm.json ─► database row ─► rename into Mods/GMM/<folder>
```

In the app, the install confirmation downloads nothing. Each picked file goes into the install
queue (the Downloads page), which downloads, extracts and scans in the background. An install
that needs a choice (several versions, blocked files, an outdated file) waits there as **Needs
action** until the user reviews it.

## Conflict detection

`gmm-migoto` reads every `.ini` of every enabled mod and collects the hashes of
`[TextureOverride*]` / `[ShaderOverride*]` sections that replace geometry or textures. Two
enabled mods conflict when they replace the same hash (with a matching `match_first_index`, or
none). Switching on a mod that conflicts asks first; GMM never edits a mod's files.

## Storage accounting

`gmm storage` and Settings › Storage show each mod's size, each game's totals, other files in
`Mods/`, GMM's caches and free disk space. Sizes are measured on disk each time.

## Catalog flow (GameBanana apiv11)

GMM reads GameBanana's public `apiv11` (no key, no login). Game ids: ZZZ = 19567,
WuWa = 20357, Endfield = 21842.

| Purpose | Endpoint |
|---|---|
| Categories | `GET /apiv11/Mod/Categories?_idGameRow={game}` (or `_idCategoryRow={category}` for characters) |
| Browse | `GET /apiv11/Mod/Index?_aFilters[Generic_Game]={game}` + category, name filter, sort, page |
| Search | `GET /apiv11/Util/Search/Results?_sModelName=Mod&_idGameRow={game}&_sSearchString=…` |
| Mod page | `GET /apiv11/Mod/{id}/ProfilePage` |
| Files | `GET /apiv11/Mod/{id}/DownloadPage` |
| Updates | `GET /apiv11/Mod/{id}/Updates` |
| Trending | `GET /apiv11/Game/{game}/TopSubs` |
| Counters | `GET /apiv11/Mod/{id}?_csvProperties=_nDownloadCount,…` |

Downloads go from `gamebanana.com/dl/<fileId>` to `files.gamebanana.com` and then a numbered
mirror; they can be paused and resumed. GameBanana's JSON is loosely typed, so every field is
decoded leniently. Responses are cached in the database; how GMM stays within GameBanana's
limits is in [gamebanana-api.md](gamebanana-api.md).

**Images** never load from the internet in the webview. Rust registers each GameBanana image
address, hands the webview a `gmmimg` key instead, downloads the image on first use and keeps
it in `cache/images/` (512 MiB cap).

**Description HTML** is rewritten in Rust (every address replaced by a key) and then sanitized
with DOMPurify in the webview. Links open only after a native confirmation that shows the real
address.

## Launch flow

When a game is `Ready`, **Launch** runs
`<XXMI>/Resources/Bin/XXMI Launcher.exe --nogui --xxmi <IMP>`, the same command as XXMI's own
"Quick Start" shortcut. GMM checks the exe again first and asks the user to confirm its path
the first time (and whenever it changes). XXMI needs administrator rights, so Windows shows a
UAC prompt. XXMI then starts the game and loads the mods; GMM doesn't touch the game process.

Mods only load when the game starts through XXMI. Starting it from Steam or the game's own
launcher runs it without mods.

## Data model

`crates/gmm-db`, one SQLite file. Tables:

| Table | Holds |
|---|---|
| `settings` | Picked XXMI folder, confirmed XXMI exe, theme and other preferences |
| `manual_game_folders` | Game folders the user added |
| `installed_mods` | One row per GMM-installed mod folder: GameBanana ids, file, version, on/off, size, category, preview |
| `download_cache` | Cached archives and their SHA-256 |
| `http_cache` | Saved GameBanana responses (pruned after 7 days) |
| `game_activity` | When GMM last launched each game |
| `profiles` | Saved profiles |
| `saved_toggles` | A switched-off mod's `d3dx_user.ini` lines |

Hand-installed mods, setup states and detected games aren't stored; they're read fresh each
time. Nothing from XXMI's config file is ever stored.

## App directories

| Purpose | Windows | Linux |
|---|---|---|
| Database | `%APPDATA%\GMM\` | `~/.local/share/GMM/` |
| Error log | `%APPDATA%\GMM\logs\` | `~/.local/share/GMM/logs/` |
| Caches (downloads, images, backups) | `%LOCALAPPDATA%\GMM\cache\` | `~/.local/share/GMM/cache/` |

If the `GMM_HOME` environment variable names an absolute folder, everything goes under it
instead (`data/` and `cache/`).

Some packaged (MSIX) terminals redirect `%APPDATA%`; running `gmm` from one can make the
database look like it's outside its folder, and GMM refuses it. Use a normal terminal or set
`GMM_HOME`.
