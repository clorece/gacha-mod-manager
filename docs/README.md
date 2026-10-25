# Gacha Mod Manager (working name: GMM) — Design & Plan

A desktop app, written from scratch, for finding, downloading and organizing GameBanana mods for
gacha games, with **XXMI** as the mod loader. Initial catalogue: **Zenless Zone Zero (ZZZ)** and
**Wuthering Waves (WuWa)**. XXMI also supports Arknights: Endfield, Honkai: Star Rail, Genshin
Impact and Honkai Impact 3rd, so each of those is one adapter away.

Development happens on Windows (primary target). Linux follows later from the same codebase.

## Where we are (2026-10-25)

**Done:** the backend (Phases 0–2), the app foundations (Phase 3) and the design (Phase 4),
2026-10-23 to 25. Phase 5 is under way (below).
Every feature works from the `gmm` command line: browse GameBanana, install (download, safe
extraction, policy scan, variants), switch mods on and off, uninstall to the Recycle Bin,
conflicts, storage, launch through XXMI. The Phase 2 exit check passed in game. The app window
(`apps/desktop`, Tauri 2 + React) has a command for every CLI action, generated TypeScript
types, the `gmmimg` image protocol, a locked-down capability and CSP, and the designed shell:
the sidebar (General, Mods with Select Game, Development, theme switch), the Dashboard (game
bubbles that open to Launch, recently installed, setup cards) and the Mods Library (switches,
conflict prompt, a first Mod settings panel with Uninstall). The visual system from the
design canvas is in the code: tokens for both themes, bundled Nunito, hand-built primitives
(`src/ui`) and a dev-only style guide page. Run it with
`corepack pnpm tauri dev` in `apps/desktop` (or `just app-dev`). A cleanup pass followed
(dead code removed, CLI/app duplicates merged into shared functions).

| Crate | What it does |
|---|---|
| `gmm-fsguard` | Every file access: registered read-only/read-write roots, safe relative paths, junction/symlink escape checks, denylist for secrets |
| `gmm-net` | Every network request: GameBanana-only allowlist (every redirect hop), rate limiting, retries, verified resumable downloads |
| `gmm-archive` | Safe zip/7z/rar extraction (two-pass checks, bomb limits) and a policy scan of what came out; starts the RAR helper |
| `gmm-extract` | The RAR helper: a separate, sandboxed program with RARLAB's UnRAR that streams entries to GMM and never writes a file |
| `gmm-catalog` | GameBanana client: list, search, mod page, files, updates, categories |
| `gmm-adapters` | The ZZZ and WuWa manifests and their validation |
| `gmm-detect` | Finds games: Steam, HoYoPlay, Epic, manual folders |
| `gmm-xxmi` | Reads XXMI's setup (allowlisted config fields only) and works out each game's setup state |
| `gmm-migoto` | Reads 3DMigoto mod `.ini` files: override hashes, what they replace, namespaces, key bindings |
| `gmm-db` | SQLite database (`%APPDATA%\GMM\gmm.db`): settings, manual game folders, installed mods, download cache, GameBanana response cache |
| `gmm-core` | Features built from the crates above: the install pipeline (fetch → extract/scan/classify → stage → rename into `Mods/GMM/`, in one go or in two steps with an install plan), the install queue and stop signal, versions (grouping, switching), the update check, the library (list with disk state, unmanaged mods, enable/disable, uninstall to the Recycle Bin), conflict detection, storage accounting, launching through XXMI (with exe confirmation), app folders, the image cache behind `gmmimg` |
| `gmm-desktop` (`apps/desktop/src-tauri`) | The app: Tauri shell, a command per CLI action plus the install confirmation, queue, versions and startup check (ids only), the queue worker, the `gmmimg` protocol, navigation guard; the React UI is in `apps/desktop/src` |
| `gmm-cli` | `gmm games`, `gmm catalog …`, `gmm fetch`, `gmm install`, `gmm list`, `gmm enable/disable/uninstall`, `gmm conflicts`, `gmm storage`, `gmm launch`, `gmm detect`, `gmm status`, `gmm config` |

On the dev machine, `gmm status` and the app report ZZZ `Ready` (HoYoPlay install) and WuWa
`Ready` (Steam install; its Steam copy runs with XXMI's Game Launch on Direct).

**Design** (Phase 4): the canvas (23 artboards, every screen dark and light, signed off
2026-10-25) is written up in [08-design-system.md](08-design-system.md); file formats and the
RAR plan in [09-archive-formats.md](09-archive-formats.md).

**Waiting on you:** the game banner source (see 08); screenshots of each XXMI setup step, for
the setup guide; the product name, if it changes.

**Phase 5** (started 2026-10-24): **RAR works.** GMM extracts RAR4 and RAR5 through
`gmm-extract`, a separate sandboxed helper with RARLAB's UnRAR 7.23 that streams entries
back while GMM checks and writes every file (libarchive was tried first and failed one of
five real mods; details in [09](09-archive-formats.md)). All five sampled RAR mods extract,
including the 113 MiB Yanagi pack. The backend for the install flow is in place too: the
two-step install (`prepare_install` returns the plan the confirmation dialog shows),
versions as one mod (grouping, installing more from the cache, `switch_version`), the
install queue with pause, resume and cancel, and the startup check (newest catalogue pages,
update check of installed mods).

**Next:** the rest of the Phase 5 backend list (trending, download counts, stored
categories, open-in-browser / open-folder commands, description sanitizing), then the
screens, starting with the startup check, Downloads and the install confirmation. See the
roadmap.

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
   folders. Game folders are read-only to GMM. The webview can't name a path. Deletes go to the
   Recycle Bin.
6. **Backend first, then frontend.** Every feature works from the `gmm` CLI before it gets a
   screen, so the UI phase is purely presentation.
