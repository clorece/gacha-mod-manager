# Gacha Mod Manager (working name: GMM) — Design & Plan

A desktop app, written from scratch, for finding, downloading and organizing GameBanana mods for
gacha games, with **XXMI** as the mod loader. Initial catalogue: **Zenless Zone Zero (ZZZ)** and
**Wuthering Waves (WuWa)**. XXMI also supports Arknights: Endfield, Honkai: Star Rail, Genshin
Impact and Honkai Impact 3rd, so each of those is one adapter away.

Development happens on Windows (primary target). Linux follows later from the same codebase.

## Where we are (2026-10-25)

**Done:** the backend (Phases 0–2), the app foundations (Phase 3) and the design (Phase 4),
2026-10-23 to 25. Phase 5 (screens) is built and tested (2026-10-25); only its exit check,
which needs a person at the real setup, is left. Phase 6 (power features) started alongside
it on 2026-10-25 (below).
Every feature works from the `gmm` command line: browse GameBanana, install (download, safe
extraction, policy scan, variants), switch mods on and off (keeping their saved in-game
toggles), uninstall for good, conflicts, storage, launch through XXMI. The Phase 2 exit check
passed in game. The app window
(`apps/desktop`, Tauri 2 + React) has a command for every CLI action, generated TypeScript
types, the `gmmimg` image protocol, a locked-down capability and CSP, and the designed shell:
the sidebar (General, Mods with Select Game, Development, theme switch), the Mods Store and
mod page, a first-run
preferences screen, the Dashboard (game banners with key art that open to Launch, last
played, recently installed, setup cards with a setup guide), the Mods Library (previews,
filters, version groups, select mode, Update all) with its Mod settings panel, Settings
with the Storage breakdown, and Documentation (player guides). The visual system from the
design canvas is in the code: tokens for both themes, bundled Nunito, hand-built primitives
(`src/ui`) and a dev-only style guide page. Run it with
`corepack pnpm tauri dev` in `apps/desktop` (or `just app-dev`). A cleanup pass followed
(dead code removed, CLI/app duplicates merged into shared functions).

| Crate | What it does |
|---|---|
| `gmm-fsguard` | Every file access: registered read-only/read-write roots, safe relative paths, junction/symlink escape checks, denylist for secrets |
| `gmm-net` | Every network request: GameBanana-only allowlist (every redirect hop), rate limiting, one shared "slow down" cooldown for all clients, verified resumable downloads |
| `gmm-archive` | Safe zip/7z/rar extraction (two-pass checks, bomb limits) and a policy scan of what came out; starts the RAR helper |
| `gmm-extract` | The RAR helper: a separate, sandboxed program with RARLAB's UnRAR that streams entries to GMM and never writes a file |
| `gmm-catalog` | GameBanana client: list (with a name filter), search, mod page, files, updates, categories, trending, counters; saved answers stand in when GameBanana can't answer |
| `gmm-adapters` | The ZZZ and WuWa manifests and their validation |
| `gmm-detect` | Finds games: Steam, HoYoPlay, Epic, manual folders |
| `gmm-xxmi` | Reads XXMI's setup (allowlisted config fields only) and works out each game's setup state |
| `gmm-migoto` | Reads 3DMigoto mod `.ini` files: override hashes, what they replace, namespaces, key bindings |
| `gmm-db` | SQLite database (`%APPDATA%\GMM\gmm.db`): settings and preferences, manual game folders, installed mods (with categories, sensitive flag and preview address), last played, profiles, saved in-game toggles, download cache, GameBanana response cache |
| `gmm-core` | Features built from the crates above: the install pipeline (fetch → extract/scan/classify → stage → rename into `Mods/GMM/`, in one go or in two steps with an install plan; one download per file at a time), the install queue and stop signal, versions (grouping, switching), the update check, the featured gallery (trending, backfilled), Store pages merged from several lists (characters, games), the library (list with disk state, unmanaged mods, enable/disable keeping saved in-game toggles, open folder, uninstall for good with the cached archives), update and roll back, profiles, key bindings, adopting hand-installed mods, installing from a local archive, conflict detection, storage accounting, launching through XXMI (with exe confirmation, recording "last played"), app folders, the image cache behind `gmmimg` (with Clear) |
| `gmm-desktop` (`apps/desktop/src-tauri`) | The app: Tauri shell, a command per CLI action plus the install confirmation, queue, versions, startup check, featured mods, card stats and the confirmed openers (ids only), the queue worker, the `gmmimg` protocol, the description address rewrite, navigation guard; the React UI is in `apps/desktop/src` |
| `gmm-cli` | `gmm games`, `gmm catalog …`, `gmm fetch`, `gmm install`, `gmm install-file`, `gmm list`, `gmm enable/disable/uninstall`, `gmm update`, `gmm rollback`, `gmm keys`, `gmm adopt`, `gmm profile …`, `gmm conflicts`, `gmm storage` (`--clear-downloads`, `--clear-images`), `gmm launch`, `gmm detect`, `gmm status`, `gmm config` |

On the dev machine, WuWa is `Ready` (Steam install; its Steam copy runs with XXMI's Game
Launch on Direct). ZZZ (HoYoPlay install) was `Ready` until XXMI's "Game Folder" for it
changed to `D:\HoYoverse\ZenlessZoneZero Game` while the game is at `C:\HoYoverse\...`
(seen 2026-10-25 evening); GMM reports it as "game folder in XXMI is wrong" with the fix.

**Design** (Phase 4): the canvas (23 artboards, every screen dark and light, signed off
2026-10-25) is written up in [08-design-system.md](08-design-system.md); file formats and the
RAR plan in [09-archive-formats.md](09-archive-formats.md).

**Waiting on you:** a look at the third review checkpoint in the app (the Library, Mod
settings, Settings with Storage, Documentation) and an edit of the six player guides in
[guides/](guides/); ZZZ's "Game Folder" in XXMI (above); the Phase 5 exit check by hand
(switching mods and versions from the app, a real install from the Store seen in game, a
real file drop and the "Install from a file" dialog, the browser openers' native
confirmations; listed in the roadmap). Decided 2026-10-25: the banners are official key art (bundled
with a notice), GMM keeps saved in-game toggles across every switch (R3 amended), deletes are
permanent (R8 amended), the name stays "Gacha Mod Manager", the setup guide is text only, and
the rest of Phase 5 follows the decisions listed in the roadmap.

**Phase 5** (started 2026-10-24): **RAR works.** GMM extracts RAR4 and RAR5 through
`gmm-extract`, a separate sandboxed helper with RARLAB's UnRAR 7.23 that streams entries
back while GMM checks and writes every file (libarchive was tried first and failed one of
five real mods; details in [09](09-archive-formats.md)). All five sampled RAR mods extract,
including the 113 MiB Yanagi pack. The backend for the install flow is in place too: the
two-step install (`prepare_install` returns the plan the confirmation dialog shows),
versions as one mod (grouping, installing more from the cache, `switch_version`), the
install queue with pause, resume and cancel, and the startup check (newest catalogue pages,
update check of installed mods).

On 2026-10-25 the rest of the backend list landed: trending for the Store's featured
gallery, download counts for cards, categories stored with each mod, "last played", the
confirmed openers (GameBanana page, XXMI releases, description links, mod folder), clearing
the HTTP cache, and description HTML with every address replaced in Rust and sanitized by
DOMPurify. Three screens followed: the **startup check**, **Downloads** (the queue with
pause, resume and cancel) and the **install confirmation** (file choice, versions, blocked
files). The app was smoke-tested in a real `tauri dev` window: `prepare_install`, the
queue, `startup_check`, a RAR install into the real WWMI Mods folder, an 8-version pack
(first version on, the clashing one off), open folder, uninstall. Two bugs found that way
are fixed: progress events flooded the IPC channel (now at most every 200 ms), and two
fetches of the same file at once collided on its partial file (now one download per file).
The first Linux CI run (2026-10-25) built the RAR helper with g++ and passed its 19 RAR
cases.

**GameBanana API** (2026-10-25, [10](10-gamebanana-api.md)): no permission or key is needed
for what GMM does (checked: the Terms of Service, Site Rules and a site moderator's answer
to the same question); only 1-click buttons on mod pages would need GameBanana to add GMM.
No outreach is planned: GameBanana can find GMM through its User-Agent and the repo, and the
project complies with any request; the root README now has a Credits section with a contact
line for credit changes or removal.
GMM now treats a "slow down" (429, 503 with `Retry-After`, a Cloudflare challenge) as a
pause for all of its GameBanana traffic, shows saved data and a notice meanwhile, and the
install queue waits and carries on. Background reads share the API budget, identical
requests are merged, and the startup update check accepts answers up to 6 h old.
Description images from GameBanana's own file hosts are shown now.

**Phase 6** (started 2026-10-25): update and roll back (folder names and on/off states
kept, previous files from the cache), profiles with share codes, key bindings, adopting
hand-installed mods, and installing from a local archive (drop on the window or a file
dialog). Each works from the `gmm` command line and in the app (Mod settings panel,
Profiles panel, Downloads); update, roll back, key bindings, profiles and adopt suggestions
were tried in the real window against the real XXMI setup (and cleaned up afterwards).

**First review checkpoint** (2026-10-25): the game banners use official key art (cropped
2:1, one file per game for every banner size); a first-run screen asks for the theme and
whether to show and blur sensitive mods, stored in the database (`preferences`); the
Dashboard shows "Played … ago"; setup cards open a four-step, text-only **setup guide**.
Behind it, two rule changes the project owner decided: **saved in-game toggles survive
every switch** (GMM copies a mod's `d3dx_user.ini` lines before it goes off and writes back
the missing ones after it comes on; that one file only, backed up, never while the game
runs; R3 amended), and **deletes are permanent** (uninstall, cache clearing, replaced
folders; the last version's cached archives go too; R8 amended, `trash` dropped). Checked in
a real `tauri dev` window; the privacy harness covers the new write.

**Second review checkpoint** (2026-10-25): the **Mods Store** (featured gallery, sticky
search and filters with characters and all six sorts, Hide installed and Show sensitive
chips, a virtualized grid that pages on scroll, download counts filling in, state badges)
and GMM's own **mod page** (slideshow and lightbox, header, license, Description / Files /
Updates / Credits, per-file Install, a link out to GameBanana), opened by clicking a card;
Back keeps the Store as it was. Behind it: GameBanana's list name filter (search now
combines with category and sort) and `gmm_core::browse`, which merges one list per ticked
character or per game. Every Download button now opens the install confirmation. Checked in
a real `tauri dev` window.

**Third review checkpoint** (2026-10-25): every Phase 5 screen is built. The **Mods
Library** shows each mod's stored preview, groups the versions of one file into one card,
filters by category and character, hides sensitive mods with the shared chip, badges updates
and conflicts, and has **Update all** and a select mode (Enable / Disable / Delete
selected). **Mod settings** gained a Versions section (switch which version is on, remove
one, add more from the cached archive). **Settings** covers Appearance, the Mods Store
switches, the XXMI folder, games and their folders, Storage (three caches with Clear, plus a
breakdown page with a per-mod bar and a sortable, multi-select table) and About (credits,
disclaimer and third-party notices from the repo files). **Documentation** shows six
player guides from [guides/](guides/), bundled at build time. Behind it: migration 0005
(sensitive flag and preview address per mod, refreshed by updates), clearing the image
cache (`clear_image_cache`, `gmm storage --clear-images`). Checked in a real `tauri dev`
window (a three-version install switched off, Add versions, select-mode delete, Settings,
Storage, Documentation in both themes).

**Phase 5 tests** (2026-10-25, all in CI): Vitest component tests on a fake bridge with
synthetic fixtures typed against the Rust types (73 tests: the Library, the install
confirmation, the storage table, the slideshow, the Markdown renderer, a mod page
snapshot); the XSS corpus through `RichText.sanitize` (it found image and table size
attributes always stripped, now fixed); the IPC fuzz harness (`ipc_fuzz.rs`: about 1,500
hostile calls through Tauri's mock runtime with the real capability, a fake XXMI and a
network that refuses everything; nothing outside GMM's folders may change); and the
contrast and overlap audit (`tests/ui-audit.mjs`, Playwright, 20 states in both themes, no
findings).

**Next:** the Phase 5 exit check (by hand), then Phase 6's Report a Bug (GitHub issue link
plus a local log), profiles on the Dashboard, and deep links if GameBanana adds GMM. See
the roadmap.

## Docs

| Doc | What it covers |
|---|---|
| [02-architecture.md](02-architecture.md) | Stack, components, repo layout, data model, core flows |
| [03-game-adapters.md](03-game-adapters.md) | Game + XXMI detection, setup states, per-game rules, adding a game |
| [04-security.md](04-security.md) | Threat model, hard rules, test harnesses per phase |
| [05-roadmap.md](05-roadmap.md) | Phases (backend, then frontend), milestones, exit criteria |
| [06-licensing-and-provenance.md](06-licensing-and-provenance.md) | Third-party licenses, clean-room rules, our license, release checklist |
| [07-frontend-design.md](07-frontend-design.md) | Screens: what each one shows (store, mod page, library, conflicts, storage, setup) |
| [08-design-system.md](08-design-system.md) | Navigation, screen layouts, tokens (dark and light), type, motion, data constraints on the design |
| [09-archive-formats.md](09-archive-formats.md) | Which archive formats GMM installs from, error messages, and the RAR plan |
| [10-gamebanana-api.md](10-gamebanana-api.md) | GameBanana's terms (no permission needed), rate limits per IP, how GMM backs off and keeps its load low, API limits users meet |
| [guides/](guides/) | The player guides the app's Documentation page shows (bundled at build time) |
| [brand/](brand/) | The GMM mark and lockups, and how to use them |
| [../crates/gmm-adapters/games/](../crates/gmm-adapters/games/) | The game manifests for ZZZ and WuWa |
| `research/` (local only, git-ignored) | Prior-art notes and Phase 0 observations |

## The decisions that shape everything

1. **XXMI loads mods; GMM manages them.** XXMI injects its 3DMigoto-based DLL when the game
   starts and reads mods from `<XXMI>/<IMPORTER>/Mods/`. Nothing is written into the game files
   that would make mods load on their own, so **the game must be started through XXMI** every
   session. GMM is the catalogue, download and organization layer. It puts mods in the right
   `Mods/` folder, toggles and checks them, and offers a "Launch with XXMI" button that calls
   XXMI's own command line. GMM never injects anything, never patches game files, and never
   bundles XXMI.
2. **Detect, then guide.** GMM detects each game (Steam, HoYoPlay, Epic, manual; Kuro launcher
   later) and XXMI's setup state per game: XXMI missing, importer missing or damaged, game folder
   not set or wrong, launch setting that doesn't fit the install, or ready. Each state comes with
   step-by-step instructions using XXMI's own button names.
3. **Games are adapters: data first, code only where needed.** A TOML manifest per game
   (GameBanana ids, XXMI importer, detection sources, classifier rules), compiled into the signed
   app.
4. **No backend server.** The client talks to GameBanana's public `apiv11` directly, with a local
   cache. No accounts, no telemetry.
5. **Least privilege in code.** GMM writes only to its own app folders and to XXMI `Mods/`
   folders, plus one file per importer, `d3dx_user.ini` (saved in-game toggles, R3). Game
   folders are read-only to GMM. The webview can't name a path. Deletes happen only after an
   itemized confirmation, and are permanent (R8).
6. **Backend first, then frontend.** Every feature works from the `gmm` CLI before it gets a
   screen, so the UI phase is purely presentation.
