# 02 — Architecture

## Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 | Small native app; WebView2 on Windows, WebKitGTK on Linux; capability-based IPC; NSIS/MSI installers |
| Core logic | Rust workspace | All file, archive, network and process work lives here |
| UI | React 19 + TypeScript + Vite, Tailwind 4, Nunito (bundled); primitives built by hand in `src/ui` (no component library); TanStack Query/Virtual, Embla (carousel) as the screens need them | Widely used, well documented; a dozen primitives didn't justify a library |
| UI ↔ Rust types | `ts-rs`: `apps/desktop/src/bindings.ts` is generated from the Rust types | The two can't drift; a test fails when the file is stale |
| Local DB | SQLite via `sqlx`, migrations in repo | One file in the app data dir |
| Archives | `zip`, `sevenz-rust2` (pure Rust, in process); RAR through UnRAR 7.23 (vendored C++) in the sandboxed `gmm-extract` helper; all wrapped by our `gmm-archive` crate | All extraction goes through one hardened path, and GMM writes every file itself |
| HTTP | `reqwest` (rustls), wrapped by our `gmm-net` crate | One place to enforce the host allowlist |
| Recycle Bin | `trash` crate | Deletes are recoverable |
| Tooling | pnpm (via corepack), Biome (lint + format), cargo workspace, `just` task runner | |

The **`gmm` CLI** is built from the same core crates. Every backend feature ships in the CLI first
(Phases 1–2). The app (Phases 3–5) only presents what the CLI can already do.

## What GMM does and doesn't do

| GMM does | GMM doesn't |
|---|---|
| Browse/search GameBanana for supported games | Host, mirror or re-upload mods |
| Download, scan, extract into `<XXMI>/<IMP>/Mods/` | Inject DLLs or hook game processes |
| Enable/disable (XXMI's `DISABLED` prefix), detect conflicts, show storage | Edit game files or game settings (XXMI does that) |
| Detect games and XXMI setup state; guide the user | Install or update XXMI (link to its releases) |
| Start XXMI via its CLI (`--nogui --xxmi <IMP>`) | Run any executable that comes from a mod |

## Component map

```mermaid
flowchart LR
  UI[React UI] -- typed IPC: ids, never paths --> CMD[Tauri commands]
  CLI[gmm CLI] --> CORE
  CMD --> CORE[gmm-core: library, install, toggle, conflicts, storage, jobs]
  CORE --> REG[gmm-adapters: registry of games]
  CORE --> DET[gmm-detect: game installs]
  CORE --> CAT[gmm-catalog: CatalogSource]
  CAT --> GB[GameBanana apiv11 client]
  CORE --> ARC[gmm-archive: extract + classify + scan]
  ARC -- stdin: archive, stdout: frames --> HLP[gmm-extract: sandboxed UnRAR helper]
  CORE --> MIG[gmm-migoto: ini parser, hash index]
  CORE --> XX[gmm-xxmi: XXMI setup + per-game state]
  XX --> DET
  CORE --> IMG[image cache: gmmimg:// protocol]
  GB & IMG --> NET[gmm-net: allowlisted HTTP]
  CORE & ARC & DET & XX --> FS[gmm-fsguard: ScopedPath]
  CORE --> DB[(SQLite)]
```

## Repo layout

✅ = exists today; the rest is planned.

```
gacha-mod-manager/
├─ apps/desktop/              # ✅ the app: src/ (React), src-tauri/ (gmm-desktop: Tauri shell,
│                             #    command layer, gmmimg protocol); bindings.ts is generated
├─ crates/
│  ├─ gmm-fsguard/            # ✅ ScopedPath, RelPath, root registry, denylist
│  ├─ gmm-net/                # ✅ allowlisted HTTP, rate limiting, resumable verified downloads
│  ├─ gmm-archive/            # ✅ hardened zip/7z/rar extraction, policy scan, RAR helper
│  │                          #    protocol and launcher (sandboxed job on Windows)
│  ├─ gmm-extract/            # ✅ the RAR helper binary: vendored UnRAR, lockdown, frames
│  ├─ gmm-catalog/            # ✅ CatalogSource trait, cache, GameBanana client
│  ├─ gmm-adapters/           # ✅ game manifests (games/*.toml), schema, validation, registry
│  ├─ gmm-detect/             # ✅ Steam / HoYoPlay / Epic / manual detection, VDF parser,
│  │                          #    the only registry reads (env.rs)
│  ├─ gmm-xxmi/               # ✅ XXMI root + allowlisted config read, importer checks, setup states
│  ├─ gmm-db/                 # ✅ SQLite + migrations: settings, installed mods, caches
│  ├─ gmm-cli/                # ✅ `gmm` binary
│  ├─ gmm-core/               # ✅ install pipeline, library (list, toggle, uninstall),
│  │                          #    conflicts, storage, launch, app folders, image cache
│  └─ gmm-migoto/             # ✅ 3DMigoto .ini parser: overrides, hashes, namespaces, keybinds
├─ docs/                      # these docs (docs/research/ is local only)
├─ CLAUDE.md                  # working agreements (doc updates per phase, commits, code rules)
├─ LICENSE-MIT, LICENSE-APACHE, README.md, CONTRIBUTING.md
├─ scripts/check-npm-licenses.mjs  # npm license policy check (same allowlist as deny.toml)
├─ package.json, pnpm-workspace.yaml, pnpm-lock.yaml  # pnpm workspace (apps/*)
└─ deny.toml, clippy.toml, rust-toolchain.toml, rustfmt.toml, justfile
```

Test fixtures live next to each crate (`crates/*/tests/`), built in code at test time (fake
Steam libraries, fake XXMI roots, malicious archives) so the repo holds no binary blobs or real
user data.

## Core concepts

- **Game**: a compiled-in adapter (`zzz`, `wuwa`), mapped to one XXMI importer (`ZZMI`, `WWMI`).
- **Game install** (`gmm_detect::GameInstall`): a detected copy of the game. Fields: game id,
  source (Steam / HoYoPlay / Epic / manual), canonical game folder (read-only to GMM), and
  Steam appid if any.
- **Setup state** (`gmm_xxmi::GameStatus`): per game, one of `XxmiNotFound`,
  `ImporterMissing`, `ImporterBroken`, `GamePathNotSet`, `PathMismatch`,
  `LaunchModeMismatch`, `Ready`, with a summary, fix steps, notes, versions and mod counts
  (see [03](03-game-adapters.md)). Computed fresh each time; nothing is stored.
- **Catalog entry**: cached GameBanana mod metadata and file list.
- **Installed mod**: a folder under `<XXMI>/<IMP>/Mods/GMM/` with a `gmm.json` sidecar
  (`gmm_core::Sidecar`: format, game, name, variant, GB mod id, file id, file name, file date,
  version, SHA-256 of the archive, install time, GMM version) and a row in `installed_mods`.
  3DMigoto only loads `.ini` files, so the sidecar is ignored by XXMI.
- **Unmanaged mod**: any other folder (or loose `.ini`) in `Mods/`, i.e. installed by hand.
  GMM shows it, includes it in conflicts and storage, and can adopt it (Phase 6: the user
  picks the GameBanana mod it is, from GMM's suggestions; it then becomes a managed mod). It only touches one the user names: `gmm enable`/`gmm disable <name or path>` use
  the same `DISABLED ` rename (enabling removes any `DISABLED` prefix style, e.g.
  `DISABLED_Old` → `Old`; switching on checks conflicts first), and `gmm uninstall <name>`
  lists the exact folder with its size and asks before moving it to the Recycle Bin. A bare
  name matching two mods (`Dup` and `GMM/Dup`) is refused until the full path is given.
- **Download cache**: original archives in `%LOCALAPPDATA%\GMM\cache\downloads`, kept for quick
  reinstall/rollback. Size-capped and clearable from the Storage screen.

## Install & toggle model

GMM installs **directly into XXMI's folder**, under its own subfolder. XXMI's `d3dx.ini` already
has `include_recursive = Mods`, so nested folders load:

```
<XXMI>/ZZMI/Mods/
  GMM/
    Ellen - Maid Outfit [gb-652373]/        ← enabled
      gmm.json
      Ellen.ini, *.buf, *.ib, *.dds ...
    DISABLED Miyabi - Casual [gb-610001]/   ← disabled (XXMI: exclude_recursive = DISABLED*)
  SomeHandInstalledMod/                     ← unmanaged, left alone
```

- **Enable/disable** = rename with or without the `DISABLED ` prefix. This is XXMI's own
  convention, so the state also makes sense to someone browsing the folder or using XXMI's tools.
- **Variants** (several full mods in one archive): each chosen variant becomes its own folder
  (`… [gb-id] (Variant B)`), so they toggle independently. The conflict checker usually flags
  enabling two variants of the same mod.
- **Writes are atomic per mod:** the archive is extracted under the cache
  (`%LOCALAPPDATA%GMMchextract<id>`), the chosen variant's files (minus blocked ones)
  are copied to `Mods/GMM/DISABLED .staging/<id>/<n>/` with their `gmm.json`, the DB row is
  written, and the folder is renamed into place (the row is removed again if the rename
  fails). Staging lives under `Mods/`, so the rename stays on one volume, and its `DISABLED`
  prefix means 3DMigoto skips it for certain (`exclude_recursive = DISABLED*`), without
  waiting on the in-game `.staging` check. Staging and extraction folders older than an hour
  are leftovers from a crash and are removed the next time GMM runs.
- **Folder names** are `<name> [gb-<id>]` plus ` (<variant>)` for one of several variants.
  GameBanana names are untrusted: characters Windows forbids become spaces, a leading
  `DISABLED` or `.` is dropped, the name is capped at 80 characters and validated like any path
  component. GMM never overwrites: a name already in the DB, or an existing folder (enabled or
  `DISABLED`) GMM didn't install, stops the install.
- **Uninstall** = move the mod folder to the Recycle Bin (`Guard::move_to_trash`, `trash`
  crate) and remove the DB row. The archive stays in the download cache.
- **Update / roll back** (`gmm_core::change_file`, Phase 6) = every folder of the mod is
  rebuilt from the other file in staging and swapped in under the **same folder name and
  on/off state** (the current folder moves into staging, the new one is renamed into place,
  the row is pointed at the new file; a failure swaps that folder back). Versions are matched
  by name. The previous folders go to the Recycle Bin; their archive stays cached, so a roll
  back downloads nothing.
- **Profiles** (Phase 6) = named sets of switched-on mods per game (table `profiles`);
  applying one is a series of ordinary on/off renames. GMM's mods are kept by folder name,
  hand-installed ones by their path without a `DISABLED` prefix.
- **Adopt** (Phase 6) = a hand-installed folder keeps its name, moves into `Mods/GMM/` only
  if it was elsewhere, gets a `gmm.json` and a row; a failed write moves it back.
- **Local archives** (Phase 6) = copied into the cache (the archive's folder registered
  read-only for that one read), then the same extraction, scan, classification and install
  as a download, without GameBanana ids. The cached copy is removed afterwards.
- **The disk is the truth for on/off.** `gmm list` compares each DB row with `Mods/GMM/`:
  a folder renamed by hand updates the row; a folder that's gone is shown as missing (and
  uninstalling it only removes the record); both names present is refused as ambiguous.
- **Folder names must stay stable (observed 2026-10-23).** 3DMigoto saves each mod's in-game
  toggle state in `<IMP>/d3dx_user.ini`, keyed by the mod's **path**, e.g.
  `$\mods\phoenix\resources\phoenix.ini\headband = 0`. Renaming or moving a mod folder,
  including adding the `DISABLED` prefix, orphans those keys, and 3DMigoto drops orphaned keys
  the next time it saves. So GMM picks a mod's folder name once at install and never renames
  it except to toggle it. Profiles snapshot `d3dx_user.ini` (Phase 6) so toggle choices
  survive a disable/enable cycle.

## Conflict detection

Implemented: `crates/gmm-migoto` (parser) and `gmm-core::conflicts` (`gmm conflicts`, the check in
`gmm enable`, and the warning after `gmm install`).

3DMigoto mods conflict when two enabled mods override the same resource. `gmm-migoto` parses every
`.ini` in each enabled mod (managed or not; files and folders named `DISABLED*` are skipped, as
3DMigoto does) and keeps:

- `[TextureOverride*]` / `[ShaderOverride*]` sections with a `hash` (lowercased), their
  `match_first_index`, and whether they replace geometry (`ib`, `vb<n>`, `handling`,
  `override_vertex_count`, `override_byte_stride`, `draw*`) or textures (`ps-t<n>`, `vs-t<n>`
  and the other stages' texture slots). Sections that only watch a hash (set a variable, run a
  command list) replace nothing and never conflict
- the top-level `namespace` declaration
- `[Key*]` sections (`key`, `back`, `type`; for the keybind viewer later)

The parser never fails (junk lines are skipped, input is capped at 4 MiB) and is
property-tested for panics. `if`/`elif`/`endif` are ignored: a key counts wherever it appears in
its section. Two overrides overlap when the hash matches and either has no
`match_first_index` or both have the same one (ZZZ mods often split one buffer into parts).

Severity:

| Level | Rule | UI |
|---|---|---|
| **Conflict** | Two enabled mods override the same hash with geometry/texture replacement | CLI: `gmm enable` refuses unless `--anyway`; `gmm install` warns (the new mod stays on). UI: blocking dialog on enable: "Disable X and enable Y" / "Enable anyway" / "Cancel" |
| **Warning** | Same GameBanana character (subcategory), but no shared hashes | Badge only. ⏳ Not built yet: needs the mod's category stored at install |
| **Ignored** | Shared library mods: ZZMI ships SlotFix, TTLib, HP bar under `Core/`, which is outside `Mods/` and never read; Mods-folder libraries are matched by `namespace` against the manifest's `[conflicts] ignore_namespaces` (empty for ZZZ and WuWa so far) | Listed as "library (ignored)" |

The index is rebuilt on every check (fast enough for the CLI; caching it in SQLite keyed by mod
folder + mtime is a later optimisation). Conflicts are advisory: GMM never edits a mod's ini.

## Storage accounting

Implemented in `gmm-core::storage` (`gmm storage`). For each game: every managed and unmanaged
mod's size (largest first), the totals, other files (loose non-mod files in `Mods/` and staging
leftovers), and free space on the drive. For the cache: downloads (with partial downloads),
leftover extraction folders, and the GameBanana responses in the database. Sizes are measured on
disk each time without following links (a walk stops at 500,000 entries and reports "at
least"); a managed mod's recorded size is corrected when it changed. Free space comes from
`Guard::available_space` (`fs4`). `gmm storage --clear-downloads` moves every cached archive to
the Recycle Bin and drops the records; installed mods stay. Planned for the Storage screen
([07](07-frontend-design.md)): multi-select disable, delete (Recycle Bin), or clear one mod's
cached archive.

## Catalog flow (GameBanana apiv11)

Game ids: ZZZ = 19567, WuWa = 20357. All endpoints below were verified against the live API
on 2026-10-22 and are implemented in `crates/gmm-catalog`:

| Purpose | Endpoint |
|---|---|
| Root categories | `GET /apiv11/Mod/Categories?_idGameRow={gameId}&_sSort=a_to_z&_bShowEmpty=true` |
| Subcategories (characters) | `GET /apiv11/Mod/Categories?_idCategoryRow={categoryId}&_sSort=a_to_z&_bShowEmpty=true` |
| Browse | `GET /apiv11/Mod/Index?_nPage=N&_nPerpage=N&_aFilters[Generic_Game]={gameId}` + optional `_aFilters[Generic_Category]={id}` and `_sSort=` one of `Generic_Newest`, `Generic_LatestUpdated` (only updated mods), `Generic_MostDownloaded`, `Generic_MostLiked`, `Generic_MostViewed`. Unknown sorts → HTTP 400 `INPUT_ERRORS` |
| Search | `GET /apiv11/Util/Search/Results?_sModelName=Mod&_sOrder=best_match&_idGameRow={gameId}&_sSearchString=…&_nPage=N` (fixed 15 per page) |
| Mod page | `GET /apiv11/Mod/{id}/ProfilePage` (404 `NO_SUCH_RECORD` if missing) |
| Files | `GET /apiv11/Mod/{id}/DownloadPage` |
| Updates/changelog | `GET /apiv11/Mod/{id}/Updates?_nPage=N&_nPerpage=N` (`_aChangeLog[]` of `{text, cat}`) |
| Trending | `GET /apiv11/Game/{gameId}/TopSubs`: a plain array, three per period (`_sPeriod`: `today`, `week`, `month`, `3month`, `6month`, `year`, `alltime`), each with `_idRow`, `_sName`, `_sInitialVisibility`, `_nLikeCount`, a 220/800 px image and the root category (no views, downloads or subcategory). Observed 2026-10-25 |
| Counters only | `GET /apiv11/Mod/{id}?_csvProperties=_nLikeCount,_nViewCount,_nPostCount,_nDownloadCount` returns just those fields (observed 2026-10-25); used for download counts on cards |

Downloads: `_sDownloadUrl` (`https://gamebanana.com/dl/<fileId>`) redirects to
`files.gamebanana.com`, then to a numbered mirror such as `filecache42.gamebanana.com`. Range
requests work (HTTP 206), so downloads can be paused and resumed. Images live on
`images.gamebanana.com`, as `_sBaseUrl` + `_sFile` (original) or `_sFile100/220/530/800`
(scaled, with `_wFile*`/`_hFile*` sizes).

GameBanana's JSON is loosely typed (numbers as strings, `[]` for empty objects), so the client
decodes every field leniently.

Rules ([10](10-gamebanana-api.md) has the reasoning): HTTP cache (SQLite `http_cache`
table in `gmm-db`) with lists cached for minutes and detail pages for an hour, and saved
answers (up to 7 days) standing in when GameBanana is unreachable, failing or asked GMM to
slow down; identical requests in flight merged into one; an API budget of about 2
requests/second per install shared with background reads; a "slow down" (429, 503 with
`Retry-After`, a Cloudflare challenge) pauses every GameBanana client of the app
(`gmm-net/src/cooldown.rs`: `Retry-After`, else 30 s doubling, 5 s to 15 min) instead of
being retried; 5xx retried with backoff; and an identifying `User-Agent`. GameBanana sends no rate-limit headers. Store each mod's
license, never re-host files, respect `_sInitialVisibility` / content ratings (NSFW hidden by
default), and sanitize `_sText` HTML before display. The full mod page field mapping is in
[07](07-frontend-design.md).

**Images** go through a Rust-side cache served to the webview via a custom protocol
(`gmmimg`), implemented in `gmm_core::ImageCache` and registered in the app's `main.rs`. The
webview never contacts the internet and never names a URL: when a catalogue command returns
data, every image URL (preview images and their scaled variants, avatars, category icons) is
registered with the cache and replaced by `http://gmmimg.localhost/<key>` (Windows/WebView2)
or `gmmimg://localhost/<key>` (elsewhere), where the key is the URL's SHA-256. Only HTTPS
URLs on `images.gamebanana.com` and, for description images, GameBanana's file hosts
(`files.gamebanana.com`, `filecacheNN.gamebanana.com`) get a key; others are dropped. On first request the image is
fetched through `gmm-net` (its own client with its own rate limit, 8/s with bursts of 16, so a
page of thumbnails doesn't hold up API calls; 8 MiB per image), kept as
`cache/images/<key>`, and served only if its first bytes are JPEG, PNG, WebP or GIF (with
`X-Content-Type-Options: nosniff`). An unknown key gets a 404; keys cached by an earlier run
are still served. The cache is capped at 512 MiB, and the oldest images go first.

**Description HTML** (`_sText`, `_sLicense`, update text) is rewritten in Rust before it
reaches the webview (`apps/desktop/src-tauri/src/html.rs`, called from `catalog_mod` and
`catalog_updates`): a scanner walks the start tags and replaces every `src` with a `gmmimg`
URL (or drops it when the image isn't on one of GameBanana's image or file hosts), every `href` with
`#gmm-link:<key>` for an `http(s)` address (or drops it), and drops `srcset`. Link keys
(first 16 bytes of the address's SHA-256, hex) live in `AppState.links` for the run. The
webview then sanitizes with DOMPurify (`components/RichText.tsx`) and opens a link only
through `open_link`. Observed 2026-10-25 across ten mods: description images are as often
on `filecacheNN.gamebanana.com`, `files.gamebanana.com` or third-party hosts (imgur,
i.meee.com.tw) as on `images.gamebanana.com`; GameBanana's own hosts are shown, third
parties never (R2).

**Featured gallery** (`gmm_core::featured`): the top submissions in GameBanana's order,
minus sensitive ones unless the user opted in, each read from its mod page (which adds views,
downloads, the character and the content-rating flag, checked again), backfilled with the
most liked of the newest 50 mods up to five. A mod page that fails is skipped; the call fails
only when neither list can be read.

## Desktop app (`apps/desktop`)

`gmm-desktop` (`apps/desktop/src-tauri`, a workspace member) is a Tauri 2 shell with one
command per CLI action, in `src/commands.rs`: `games`, `statuses`, `catalog_list`,
`catalog_search`, `catalog_mod`, `catalog_updates`, `catalog_categories`, `install`,
`library`, `set_enabled`, `uninstall_preview`, `uninstall`, `conflicts`, `storage`,
`clear_download_cache`, `launch_plan`, `launch`, `settings`, `pick_xxmi_folder`,
`clear_xxmi_folder`, `pick_game_folder`, `remove_game_folder`; plus, for the Phase 5
screens, `prepare_install`, `switch_version`, `catalog_featured`, `catalog_stats` and
`clear_http_cache` there, the queue in `src/queue.rs` (`queue_install`, `queue_jobs`,
`queue_watch`, `queue_pause`, `queue_resume`, `queue_cancel`, `queue_dismiss`,
`queue_clear_finished`), `startup_check` in `src/startup.rs`, and the openers in
`src/open.rs` (`open_mod_page`, `open_xxmi_releases`, `open_link`, `open_mod_folder`),
and `network_status`. Phase 6 adds `queue_change_file` (queue), `rollback_files`,
`mod_keybindings`, `adopt_candidates`, `adopt` (commands.rs), the profile commands in
`src/profiles.rs` (`profiles`, `profile_save`, `profile_apply`, `profile_rename`,
`profile_delete`, `profile_export`, `profile_import`) and the local install in
`src/local.rs` (`drop_watch`, `pick_archive`, `prepare_local`, `install_local`,
`discard_local`). Each calls the same `gmm-core` functions as the CLI.

- **Arguments are ids**: game ids, GameBanana ids, GMM mod ids, and `ModKey`
  (`{type: "managed", id}` or `{type: "unmanaged", path}`, where `path` must exactly match an
  entry of a fresh library listing: `gmm_core::set_unmanaged_enabled_at`,
  `listed_unmanaged`). Folder pickers open from Rust (`tauri-plugin-dialog`, Rust side only)
  and the result is validated (`gmm_xxmi::open_xxmi`, `gmm_detect::resolve_game_folder`)
  before it's saved. `remove_game_folder` takes a folder `settings` listed and only matches
  it against stored rows.
- **Errors** are a tagged `CmdError`: `choose_variant` (with the variant names) and
  `blocked` (with the file names) from `install`, `conflicts` from `set_enabled` (with the
  conflicts; call again with `anyway`), `not_ready`, and `other`.
- **Install progress** goes over a Tauri `Channel` passed to `install` and
  `prepare_install` (no event permissions needed), at most every 200 ms for download
  progress (`ProgressThrottle`; the download reports every chunk); `queue_watch`,
  `startup_check` and `catalog_stats` use channels the same way.
- **Install confirmation, then the queue**: `prepare_install` fetches the file into the
  download cache, analyses it and returns a `gmm_core::InstallPlan` (versions, blocked files,
  installed versions, conflicts between versions, target folder), then removes the
  extraction folder; nothing is installed. The confirmed choice goes to `queue_install`. One
  worker task (`queue::spawn_worker`) runs queued installs in order through `gmm_core::install`
  (the archive comes from the cache), with the job's `StopSignal` for pause and cancel, and
  sends the list to every watcher when it changes (progress at most every 200 ms). A job
  carries what the Downloads row shows (`JobLabel`: name, author, file, size, preview).
  After a multi-version install that should start switched on, the worker runs
  `switch_version` on the first version picked, so the ones that clash with it are off.
  `gmm_core::fetch_archive` lets one download per GameBanana file run at a time (a
  process-wide lock per file id): a second fetch of the same file waits, then finds it in
  the cache or resumes the partial file.
- **Openers** (`src/open.rs`): a page address is built in Rust from an id (or looked up from a
  link key) and shown in a native dialog (`tauri-plugin-dialog`, Rust side) before
  `gmm_xxmi::open_page` hands it to the browser; `open_mod_folder` resolves the folder with
  `gmm_core::mod_folder` (a directory inside the Mods folder, through
  `Guard::os_path_of_dir`) and `gmm_xxmi::open_folder` opens it.
- **Background reads**: `catalog_stats` uses `AppState.gb_background`, a second
  `GameBanana` on a sibling client (`Client::sibling`: its own 1 request/s, bursts of 2,
  and it spends the API client's budget too) sharing the response cache. The image client is
  a sibling with its own 8/s budget. All three share one connection pool and one cooldown.
- **Slow-down notice**: `network_status` (local only) says whether GMM is holding off
  GameBanana and whether saved answers were shown lately; `NetworkNotice` polls it every
  5 s. The queue worker turns a slow-down into a `waiting` job that carries on by itself
  (up to four waits), keeping the partial download.
- **Local installs**: a file dropped on the window arrives as Tauri's window drop event in
  Rust (`main.rs`), which keeps the path and sends only the file name to the webview
  (`drop_watch`); "Install from a file" opens a native dialog from Rust (`pick_archive`).
  One archive waits at a time, with the workspace its paths belong to.
- **Launch** is two steps: `launch_plan` shows the exe and whether it was confirmed;
  `launch(game, confirm)` records the confirmation and starts XXMI.
- **Uninstall** is two steps: `uninstall_preview` returns each folder's full path (and size,
  file count for hand-installed mods) for the confirmation dialog; `uninstall` then moves
  exactly those to the Recycle Bin.
- **Types**: everything that crosses the bridge derives `ts_rs::TS` behind each crate's
  `ts` feature (`gmm-catalog`, `gmm-detect`, `gmm-xxmi`, `gmm-core`), and
  `src-tauri/src/bindings.rs` writes them into `src/bindings.ts` (64-bit integers as
  `number`). `src/api.ts` wraps `invoke` with those types.
- **Frontend** (`apps/desktop/src`, look and layout from [08](08-design-system.md)):
  - `styles.css`: the tokens for both themes (`.t-dark` / `.t-light` on `<html>`, also
    exposed to Tailwind as `bg-card`, `text-muted`, ...), base styles, scrollbars, and the
    classes for the primitives, shell and pages. `theme.ts` applies dark / light / match
    Windows (kept per window in `localStorage` for now).
  - `ui/`: the primitives (`Button`, `IconButton`, `Switch`, `Checkbox`, `Radio`,
    `Segmented`, `Dropdown`, `Badge`, `Chip`, `Notice`, `Dialog` and `SidePanel` on the
    native `<dialog>`, `ToastProvider` / `useToast`, `ProgressBar`, `Spinner`, `Icon`).
  - `App.tsx` + `shell/`: the startup check first, then the sidebar (`Sidebar` with the
    active-download count, `GamePicker`), page switching and the install confirmation;
    `data.ts` loads games, setup states and every game's library once for all pages;
    `queue.ts` keeps the install queue (`queue_watch`), refreshes a game's library when one
    of its installs finishes and dismisses installed rows after 5 s.
  - `pages/`: `Startup` (the startup check), `Dashboard` (bubbles, Launch, recently
    installed, setup), `Downloads` (the queue by game, with Pause / Resume / Cancel),
    `Library` (game cards, search, enable / disable all, mod cards, conflict prompt),
    `Placeholders` (the remaining Phase 5 pages and Report a Bug), `StyleGuide` (dev builds
    only, dropped from release builds by `import.meta.env.DEV`; also opens the real install
    confirmation by mod id until the Store exists).
  - `components/`: `InstallDialog` (the install confirmation from `InstallPlan`),
    `LocalInstallDialog` (the same for a local archive), `RichText` (GameBanana HTML through
    DOMPurify), `NetworkNotice`, `Launch` (exe confirmation dialog), `SetupCard`,
    `ModSettings` (side panel: switch, Updates / Roll back, Key bindings, Actions, details,
    Uninstall with the itemized confirmation; the Phase 6 sections are in `ModExtras`),
    `AdoptDialog`, `ProfilesPanel`.
- Logic both front ends need lives below them, not in either: `Page::hide_sensitive`
  (`gmm-catalog`, the NSFW filter), `game_for_mod`, `Machine::launch_plan` and
  `conflicts_after_install` (`gmm-core`).
- The app uses `AppDirs::system()` like the CLI (so `GMM_HOME` works for both), one shared
  `Db`, one API `gmm_net::Client`, the background client and the image cache's own client.

Security wiring (capability, CSP, navigation guard, devtools) is in
[04](04-security.md#catalogue--ui-hardening).

## Install flow

Implemented in `crates/gmm-core` (`gmm install`). Steps marked ⏳ come with later features.

```
setup gate: XXMI and the importer must exist (else refuse); any other non-Ready state warns
mod page ─► refuse trashed / withheld / obsolete mods, and mods whose _aGame isn't this game
pick GB file (newest clean one, or --file) ─► _sAvResult must be "clean"
            ─► download into cache/downloads (allowlisted host after every redirect, exact
               _nFilesize, MD5 if given), or reuse the cached copy; record SHA-256 in download_cache
            ─► extract to cache/extract/<id> (all hardening rules; RAR through the
               gmm-extract helper, GMM writing the files) ─► policy scan
            ─► classify: a folder is a mod root when it directly holds an .ini with one of the
               manifest's markers ([TextureOverride / [ShaderOverride, case-insensitive) and no
               folder above it does. DISABLED* names and d3dx.ini don't count
            ─► one root = one mod; several = variants: --variant N|name (repeatable) or
               --all-variants, otherwise the install stops and lists them
            ─► blocked files inside the chosen mod stop the install (--skip-blocked leaves
               them out); blocked files elsewhere in the archive don't matter
            ─► ⏳ conflict check against enabled mods
            ─► copy to Mods/GMM/DISABLED .staging/<id>/ ─► gmm.json ─► DB row ─► rename into
               Mods/GMM/<folder> ─► remove extraction + staging folders
```

## Launch flow

GMM shows the game's XXMI state on its page. When it's `Ready`, **Launch with XXMI** runs
`<XXMI>/Resources/Bin/XXMI Launcher.exe --nogui --xxmi <IMP>`. That's XXMI's own command line:
its "Add Desktop Shortcut" creates `<IMP> Quick Start.lnk` with exactly these arguments
(observed). `gmm status` prints this command for ready games, and `gmm launch <game>` runs it: only
when the game is `Ready`, only after re-checking that the exe is a regular file inside the XXMI
root starting with `MZ`, and only once the user has confirmed that exact path (interactive
prompt or `--yes`; stored as `xxmi.confirmed_exe`, asked again if the path changes). XXMI's
launcher requires administrator rights, so on Windows GMM starts it through the shell
(`ShellExecuteExW`, as Explorer does for the Quick Start shortcut) and Windows shows its UAC
prompt. XXMI then starts the game (through Steam if its Game Launch says so) and injects. GMM doesn't watch or touch the game process.

The game page always shows this notice:

> Mods only load when the game is started through XXMI. Launching from Steam or the game's own
> launcher alone starts the game without mods. Sorry, we know it's an extra step. Use
> **Launch with XXMI** here, or XXMI's own Quick Start shortcut.

For Steam copies, XXMI's **Game Launch = Steam** makes XXMI start the game through Steam itself,
so users don't need to start XXMI first and the game second. The setup check enforces the
matching Game Launch setting (`LaunchModeMismatch`).

## Data model (SQLite)

`crates/gmm-db`, one file at `%APPDATA%\GMM\gmm.db`. The schema is in
`crates/gmm-db/migrations/` and is applied when the database opens; an older GMM refuses a
database that a newer one has migrated. sqlx 0.9 with SQLite bundled, WAL journal, foreign
keys on, every query a static SQL string with bound parameters (sqlx rejects anything else).
Times are Unix seconds. Built so far (migrations 0001 to 0003):

```
settings(key PK, value)                  -- xxmi.root (a folder the user picked),
                                         -- xxmi.confirmed_exe (R7); typed accessors only
manual_game_folders(game_id, folder, added_at, PK(game_id, folder))
installed_mods(id PK, game_id, folder_name, name, gb_mod_id NULL, gb_file_id NULL,
               variant NULL, version NULL, file_date NULL, enabled BOOL, archive_sha256 NULL,
               size_bytes, installed_at, scan_report_json NULL,
               root_category_id NULL, root_category NULL,   -- 0002: GameBanana's categories
               category_id NULL, category NULL,             -- at install (category = character)
               UNIQUE(game_id, folder_name))
download_cache(sha256 PK, gb_mod_id NULL, gb_file_id NULL, path_rel UNIQUE, size_bytes,
               added_at, last_used_at)
http_cache(url PK, body, fetched_at)     -- each endpoint passes its own max age on read;
                                         -- rows older than 7 days, or beyond 5,000, are
                                         -- pruned on open
game_activity(game_id PK, last_played)   -- 0002: when GMM's Launch last started each game
profiles(id PK, game_id, name, entries_json, created_at, updated_at, UNIQUE(game_id, name))
                                         -- 0003: switched-on mods per profile (gmm-core JSON)
```

- `installed_mods.folder_name` never includes the `DISABLED ` prefix; `enabled` says which
  name is on disk. Only managed mods are stored. Unmanaged mods are listed from disk each
  time, so GMM never has a stale idea of what someone put there by hand.
- Setup states, detected installs and XXMI roots are **not** stored: they're cheap to work
  out and must reflect the machine as it is now. Only what the user told GMM (manual folders,
  a picked XXMI folder, the confirmed exe) is remembered.
- The database never holds anything from XXMI's config file (R4).

Planned with later features: `ini_index(mod folder, mtime, hashes_json, namespaces_json,
keys_json)` with `gmm-migoto`; parsed `catalog_mods`/`catalog_files` rows only if the UI needs
queries the raw response cache can't answer.

## App directories (Windows)

| Purpose | Path |
|---|---|
| Config + DB | `%APPDATA%\GMM\` |
| Caches (downloads, images, HTTP) | `%LOCALAPPDATA%\GMM\cache\` |
| Logs (local only, home path redacted) | `%LOCALAPPDATA%\GMM\logs\` |

On Linux these map to the XDG equivalents (`dirs`, through `gmm_core::AppDirs`, in both the
CLI and the app).
If `GMM_HOME` names an absolute folder, the database goes to `GMM_HOME/data/` and the cache to
`GMM_HOME/cache/` instead (portable use; the privacy harness relies on it). Logs don't exist yet.

Observed 2026-10-24: a `gmm` started from inside a packaged (MSIX) app that virtualizes
`%APPDATA%` (e.g. the Claude desktop app's shell) gets its writes redirected to
`%LOCALAPPDATA%\Packages\<app>\LocalCache\Roaming\GMM\`. Once a normal run has also created
the real `%APPDATA%\GMM`, the folder and the database resolve to different places and fsguard
refuses the database as "resolves outside its root" (correctly: it can't tell this apart from a
planted link). Run `gmm` from a normal terminal, or set `GMM_HOME`. Checked again 2026-10-25:
the dev machine itself has no junction there; it is only that redirect. Writes into XXMI's
existing `Mods` folders are not redirected, so installs from such a shell land in the real
folders.
