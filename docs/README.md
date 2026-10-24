# Gacha Mod Manager (working name: GMM) — Design & Plan

A desktop app, written from scratch, for finding, downloading and organizing GameBanana mods for
gacha games, with **XXMI** as the mod loader. Initial catalogue: **Zenless Zone Zero (ZZZ)** and
**Wuthering Waves (WuWa)**. XXMI also supports Arknights: Endfield, Honkai: Star Rail, Genshin
Impact and Honkai Impact 3rd, so each of those is one adapter away.

Development happens on Windows (primary target). Linux follows later from the same codebase.

## Where we are (2026-10-23)

**Done:** Phase 0 (discovery), Phase 0b (setup check), the parts of Phase 1 that Phase 2
needs, and the first Phase 2 items: the local database, the install pipeline, toggling,
uninstalling and listing mods, conflict detection, and storage accounting. Everything
works from the `gmm` command line; there is no app window yet.

| Crate | What it does |
|---|---|
| `gmm-fsguard` | Every file access: registered read-only/read-write roots, safe relative paths, junction/symlink escape checks, denylist for secrets |
| `gmm-net` | Every network request: GameBanana-only allowlist (every redirect hop), rate limiting, retries, verified resumable downloads |
| `gmm-archive` | Safe zip/7z extraction (two-pass checks, bomb limits) and a policy scan of what came out |
| `gmm-catalog` | GameBanana client: list, search, mod page, files, updates, categories |
| `gmm-adapters` | The ZZZ and WuWa manifests and their validation |
| `gmm-detect` | Finds games: Steam, HoYoPlay, Epic, manual folders |
| `gmm-xxmi` | Reads XXMI's setup (allowlisted config fields only) and works out each game's setup state |
| `gmm-migoto` | Reads 3DMigoto mod `.ini` files: override hashes, what they replace, namespaces, key bindings |
| `gmm-db` | SQLite database (`%APPDATA%\GMM\gmm.db`): settings, manual game folders, installed mods, download cache, GameBanana response cache |
| `gmm-core` | Features built from the crates above: the install pipeline (fetch → extract/scan/classify → stage → rename into `Mods/GMM/`), the library (list with disk state, unmanaged mods, enable/disable, uninstall to the Recycle Bin), conflict detection, storage accounting, app folders |
| `gmm-cli` | `gmm games`, `gmm catalog …`, `gmm fetch`, `gmm install`, `gmm list`, `gmm enable/disable/uninstall`, `gmm conflicts`, `gmm storage`, `gmm detect`, `gmm status`, `gmm config` |

On the dev machine, `gmm status` reports ZZZ `Ready` (HoYoPlay install) and WuWa
`LaunchModeMismatch` (Steam install, XXMI's Game Launch still on Direct).

**Waiting on you:** the in-game checks in [05-roadmap.md](05-roadmap.md) under "Before Phase 2"
(WuWa launch setting, `DISABLED` rename, nested `Mods/GMM/` folder, Quick Start shortcut).

**Next:** `gmm launch`, then the adapter conformance suite and the privacy harnesses. See "Start here" in the roadmap.

## Docs

| Doc | What it covers |
|---|---|
| [02-architecture.md](02-architecture.md) | Stack, components, repo layout, data model, core flows |
| [03-game-adapters.md](03-game-adapters.md) | Game + XXMI detection, setup states, per-game rules, adding a game |
| [04-security.md](04-security.md) | Threat model, hard rules, test harnesses per phase |
| [05-roadmap.md](05-roadmap.md) | Phases (backend, then frontend), milestones, exit criteria |
| [06-licensing-and-provenance.md](06-licensing-and-provenance.md) | Third-party licenses, clean-room rules, our license, release checklist |
| [07-frontend-design.md](07-frontend-design.md) | Screens: catalogue, mod page, library, conflicts, storage, setup |
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
