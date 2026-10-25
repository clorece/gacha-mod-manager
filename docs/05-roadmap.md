# 05 — Roadmap

Backend first (Phases 0–2, every feature behind the `gmm` CLI), then the app: foundations
(Phase 3), design (Phase 4), screens (Phase 5). Security harnesses land in the same phase as
the code they guard ([04](04-security.md)). When a phase finishes, every doc gets updated (see
`CLAUDE.md`). Finished phases are summarized here; the details live in the other docs, in
`docs/research/phase0-observations.md` (local only) and in git history.

## Done

| Phase | Finished | What it delivered |
|---|---|---|
| 0 — Discovery | 2026-10-23 | Facts from the dev machine: XXMI layout and config keys, ZZZ/WuWa installs, `d3dx.ini` rules (`include_recursive = Mods`, `exclude_recursive = DISABLED*`), the Quick Start command line, GameBanana endpoints and image fields ([02](02-architecture.md), [03](03-game-adapters.md)) |
| 0b — Setup check | 2026-10-23 | `gmm-detect` (Steam, HoYoPlay, Epic, manual), `gmm-xxmi` (allowlisted config read, importer checks), the setup states with fix steps, `gmm detect` / `gmm status` |
| In-game checks | 2026-10-24 | Confirmed in game: `DISABLED ` hides a mod, nested `Mods/GMM/<mod>/` loads, Quick Start works, WuWa's Steam copy runs with Game Launch = Direct |
| 1 — Foundations | 2026-10-23 | Workspace, CI (Windows + Linux), `cargo-deny`; `gmm-fsguard`, `gmm-net`, `gmm-archive` (zip/7z, malicious corpus), `gmm-catalog`, `gmm-adapters`; `gmm catalog`, `gmm fetch` |
| 2 — Backend features | 2026-10-24 | `gmm-db`, `gmm-core` (install pipeline, library, toggle, uninstall to the Recycle Bin, conflicts, storage, launch through XXMI), `gmm-migoto`; the adapter conformance suite and the egress / file-access privacy harnesses. Exit check passed in game with Casual Velina (GameBanana 688491) |
| 3 — App foundations | 2026-10-24 | `apps/desktop`: Tauri 2 shell over `gmm-core` with 22 commands (ids only), generated TypeScript types, the `gmmimg` image protocol, locked-down capability / CSP / navigation guard, a first shell (game rail, state pill, setup card, mod list, Launch with XXMI); frontend CI (typecheck, lint, `pnpm audit`, npm license check). Verified in a release build on the dev machine |

Cleanup pass (2026-10-24, after Phase 3): unused functions and a dependency removed; the NSFW
filter, "which game is this mod for", launch planning and post-install conflict checks moved
into shared `gmm-catalog` / `gmm-core` functions used by both the CLI and the app; Biome pinned
to a release outside pnpm's minimum-age window (the auto-added exemption removed); finished
phases condensed in this file.

Design rounds (2026-10-24/25): see Phase 4 below.

## Phase 4 — Design (mostly done; finishing in the app)

Deciding how the app looks and flows before building the screens, so each gets built once.
The written spec is [08-design-system.md](08-design-system.md); the interactive mockup is a
design canvas in your claude.ai account (23 artboards: every screen and dialog, dark and
light, real GameBanana data with placeholder preview art).

**Your direction (2026-10-24/25):** left sidebar with General (Dashboard, Downloads, Settings),
Mods (Select Game banner dropdown, Mods Library, Mods Store) and Development (Report a Bug as
WIP, Documentation); dashboard game bubbles that enlarge to reveal Launch; downloads grouped by
game with a self-clearing finished state; store with a trending gallery and sticky search and
filters; library with a game selector header, switches, install dates, select mode and enable /
disable all; a startup check screen; an install confirmation where every choice (file,
versions) is made before downloading; per-mod settings with versions; theme after HoYoLAB; the
GMM brand ([brand/](brand/)).

**Done**
- [x] Brand assets in the repo (`docs/brand/`); app and exe icons from the mark; the mark in
      the current shell
- [x] Navigation and every screen designed, dark and light, on the canvas; layout and theme
      signed off (2026-10-25)
- [x] Visual system: tokens for both themes, type, radius, spacing, motion, scrollbars,
      accessibility rules ([08](08-design-system.md#tokens))
- [x] Three contrast and overlap passes, then a fourth with the new surfaces: 62 states in
      both themes, checked by screenshots and an automated audit (contrast of every text
      against its real background, overlap of every pair of text lines). Rules recorded in 08
- [x] Second round (2026-10-25): sticky search / filter bar, startup check screen,
      install confirmation (one version, choose a file, choose versions with a scrolling
      list, add versions, RAR notice), mod settings with versions, "needs your choice"
      removed from Downloads
- [x] Data checks that shape the design: NSFW share, trending endpoint, no download counts in
      listings, RAR share (4 of 15 sampled), versions as files vs. folders, character icons,
      game art ([08](08-design-system.md#what-the-data-allows-found-while-designing))
- [x] File formats documented with a RAR decision record ([09](09-archive-formats.md))
- [x] [07](07-frontend-design.md) updated to the new navigation and flows

**Left** (finishing the phase in the app)
- [ ] Decide the game banner source and the component approach (open decisions in 08)
- [ ] Bundle Nunito (OFL-1.1: a font-only exception in the npm license check, a notice)
- [ ] Port the tokens and base styles from the canvas into `apps/desktop/src/styles.css`
      (both themes, scrollbars, `:where()` button base)
- [ ] Primitives: button, icon button, switch, checkbox, radio, dropdown, chip / badge,
      dialog, side panel, toast, progress bar
- [ ] Living style guide: a dev-only page showing the tokens and every primitive state
- [ ] Restyle the current shell to the sidebar layout (Dashboard with bubbles and Launch,
      Library, the launch dialog), as the proof

Exit: the tokens, font and primitives are in the codebase, the style guide page shows them,
and the shell uses the new navigation.

## Phase 5 — Screens

Every screen sits on commands that exist or are small additions. Order: what unblocks
installs first, then the screens in the order users meet them.

**Backend additions**
- [ ] **RAR support** first (about a quarter of mods): `gmm-extract` helper with libarchive,
      sandboxed, two-pass checks in the parent, R7 amended; fall back to UnRAR if libarchive
      fails real archives ([09](09-archive-formats.md#rar-plan-decision-record))
- [ ] Install in two steps for the confirmation dialog: `prepare_install` (fetch into the
      cache and analyze: files, versions, blocked files, conflicts between versions) and
      `install` (the chosen file, versions, skip-blocked, switch on or not). `gmm-core`
      already has `fetch_archive` + `analyze` + `install_analyzed`
- [ ] Versions as one mod: group installed folders by GameBanana mod and file, list the
      archive's other versions, install one more from the cached archive, remove one, switch
      which one is on (exclusive when they conflict)
- [ ] An install queue with per-item state for the Downloads page; cancel hook in
      `gmm-core::install`; pause / resume via `gmm-net`'s resumable downloads
- [ ] Startup check: newest catalogue pages per game (cached), update check for installed
      mods (`_tsDateUpdated`, new file ids; moved here from Phase 6), and the app-update
      step once the Phase 7 updater exists
- [ ] Trending: `CatalogSource::top_subs` over `apiv11/Game/{id}/TopSubs` (safe-for-work
      entries first, backfilled)
- [ ] Download counts for cards: the mod pages of visible cards in the background
      (rate-limited, cached)
- [ ] Store each mod's category and character at install; record "last played" on launch
- [ ] Commands: open a GameBanana page or the XXMI releases page in the system browser (after
      a confirmation, ids only), open a mod's folder (R7 allows folders only), clear the HTTP
      cache (`gmm_db::clear_http_cache` exists)
- [ ] Rewrite and sanitize description HTML (DOMPurify; images through `gmmimg`)
- [ ] Look again at GameBanana's `File/{id}/RawFileList` (could list versions without
      downloading)

**Screens** (in order)
- [ ] Startup check screen
- [ ] App shell: sidebar (Select Game, theme switch); Downloads page on the install queue
- [ ] Dashboard: game bubbles with Launch, setup cards for games that aren't Ready, recently
      installed
- [ ] Setup guide: the failing row highlighted per `SetupState`, a stepper with your
      screenshots
- [ ] Mods Store: featured gallery, sticky toolbar, virtualized grid, filters (category,
      character, sort), state badges (installed, disabled, update available, conflict)
- [ ] Install confirmation: one version, choose a file, choose versions, add versions,
      blocked files, RAR / unsupported notices
- [ ] Mod page: hero + slideshow + lightbox, header / stats / license, Description, Files
      (archived files), Credits, Updates
- [ ] Mods Library: grid, sticky toolbar, switches, filters, select mode, enable / disable
      all, hand-installed mods, conflict notice
- [ ] Mod settings panel: versions, details, actions, uninstall with the itemized
      confirmation for hand-installed mods
- [ ] Settings (with Storage: segment bar, table, multi-select disable / delete, clear caches)
      and About (notices, disclaimer, XXMI credit)
- [ ] Documentation page (guides rewritten for players); Report a Bug as the WIP page

**Tests**
- [ ] Vitest component tests and fixture-based snapshots
- [ ] XSS corpus through the description renderer (T3)
- [ ] IPC fuzz harness: hostile arguments to every command (R5; so far checked by hand)
- [ ] The contrast and overlap audit from Phase 4, run against the real app in CI

Exit: everything the CLI does is reachable in the app for ZZZ and WuWa (RAR included), with
the designed loading, empty and error states; install (all confirmation paths), versions,
uninstall, storage and settings exercised in the app against the real setup.

## Phase 6 — Power features

- [ ] One-click mod update with rollback from the download cache (the update check itself
      lands in Phase 5's startup check)
- [ ] Profiles (named sets of enabled mods; snapshot / restore `d3dx_user.ini`), shareable as a
      JSON code
- [ ] Keybind viewer (from `[Key*]` sections, already parsed by `gmm-migoto`)
- [ ] Adopt unmanaged mods (match against GameBanana)
- [ ] Drag-and-drop local archive install (same scan pipeline)
- [ ] Report a Bug (describe, attach a log the user reviews; needs a destination host, R2)
- [ ] 1-click install deep links (`gmm://`) if GameBanana allows (T6)

## Phase 7 — Release

- [ ] Name chosen and cleared; logo and app icon
- [ ] Windows installer (NSIS/MSI via Tauri), code signing, signed auto-updates from GitHub
      Releases (adds `github.com` to the allowlist, R2)
- [ ] Hardening: `cargo-fuzz` targets as a nightly job; Windows equivalents of the strace checks
      (Process Monitor, a firewall rule for the bypass check); reproducible-build check;
      release feature assertion; `SECURITY.md`
- [ ] Licensing checklist in [06](06-licensing-and-provenance.md) (third-party notices in About)
- [ ] GameBanana contacted about API use and 1-click
- [ ] Public beta (Windows), then Linux (AppImage / Flatpak) from the same codebase

## Phase 8+ — More games

XXMI importers already exist: **Arknights: Endfield (EFMI)** first (it's in your Steam library),
then Star Rail (SRMI), Genshin (GIMI), HI3 (HIMI). HoYoPlay detection already sees Genshin,
Star Rail and HI3 install paths. Each game goes through the adapter checklist in
[03](03-game-adapters.md). Later detection sources: the Kuro launcher (native WuWa), and a real
Epic install to confirm the Epic reader (fixture-tested only).

## Known gaps

- `installed_mods` doesn't record which XXMI root a mod went into; the library reconciles
  with the disk instead.
- SQLite may reuse the id of the most recently deleted mod.
- The conflict index is rebuilt on every call (fine at today's sizes; cache it if it gets slow).

## Open questions

1. **Name.** The license is decided (MIT OR Apache-2.0, see [06](06-licensing-and-provenance.md)).
   The product name still needs to be chosen and cleared before publishing.
2. **GameBanana relationship.** Contact them before release about API use and 1-click.
3. **Copyright holder** for the license files (currently "The Gacha Mod Manager Contributors").
