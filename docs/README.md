# Gacha Mod Manager (working name: GMM) — Design & Plan

A desktop app, written from scratch, for finding, downloading and organizing GameBanana mods for
gacha games, with **XXMI** as the mod loader. Initial catalogue: **Zenless Zone Zero (ZZZ)** and
**Wuthering Waves (WuWa)**. XXMI also supports Arknights: Endfield, Honkai: Star Rail, Genshin
Impact and Honkai Impact 3rd, so each of those is one adapter away.

Development happens on Windows (primary target). Linux follows later from the same codebase.

| Doc | What it covers |
|---|---|
| [02-architecture.md](02-architecture.md) | Stack, components, repo layout, data model, core flows |
| [03-game-adapters.md](03-game-adapters.md) | Game + XXMI detection, setup states, per-game rules, adding a game |
| [04-security.md](04-security.md) | Threat model, hard rules, test harnesses per phase |
| [05-roadmap.md](05-roadmap.md) | Phases (backend, then frontend), milestones, exit criteria |
| [06-licensing-and-provenance.md](06-licensing-and-provenance.md) | Third-party licenses, clean-room rules, our license, release checklist |
| [07-frontend-design.md](07-frontend-design.md) | Screens: catalogue, mod page, library, conflicts, storage, setup |
| [examples/](examples/) | Draft adapter manifests for ZZZ and WuWa |
| [research/](research/) | Prior-art notes and Phase 0 observations. Internal only: keep out of the public repo |

## The decisions that shape everything

1. **XXMI loads mods; GMM manages them.** XXMI injects its 3DMigoto-based DLL when the game
   starts and reads mods from `<XXMI>/<IMPORTER>/Mods/`. Nothing is written into the game files
   that would make mods load on their own, so **the game must be started through XXMI** every
   session. GMM is the catalogue, download and organization layer. It puts mods in the right
   `Mods/` folder, toggles and checks them, and offers a "Launch with XXMI" button that calls
   XXMI's own command line. GMM never injects anything, never patches game files, and never
   bundles XXMI.
2. **Detect, then guide.** GMM detects each game (Steam, HoYoPlay, Kuro launcher, Epic, manual)
   and XXMI's setup state per game: XXMI missing, importer not installed, game path not set, or
   ready. For each state it shows step-by-step instructions and a link to XXMI's releases.
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
