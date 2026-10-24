# 05 — Roadmap

Backend and frontend are separate phases. Phases 1–2 build every feature behind the `gmm` CLI.
Phase 3 builds the UI on top of it. Security harnesses are added in the same phase as the code
they guard ([04](04-security.md)). When a phase finishes, every doc gets updated (see
`CLAUDE.md`).

## Phase 0 — Discovery on the dev machine (no app code) ✅ done 2026-10-23

Details: `docs/research/phase0-observations.md` (local only).

- [x] Locate XXMI root, config keys, ZZMI folder layout, `d3dx.ini` (`include_recursive = Mods`,
      `exclude_recursive = DISABLED*`)
- [x] Locate ZZZ and WuWa Steam installs (appids, ZZZ's nested `games/ZenlessZoneZero Game`)
- [x] Launch ZZZ once through XXMI: `game_folder` recorded, hook succeeded, 3DMigoto parsed a
      hand-installed mod (`d3dx_user.ini` keys). The Steam copy then exits ("launch from Steam")
      because XXMI's **Game Launch** setting was Direct
- [x] Install WWMI for WuWa in XXMI (`WWMI/` folder, WWMI 1.1.0)
- [x] XXMI exe path, version resource, install record (uninstall key), Quick Start shortcut
      command (`--nogui --xxmi <IMP>`); exe is not code-signed
- [x] XXMI GUI labels for the setup guide, from its locale files
- [x] Confirm GameBanana list/page/sort params, image fields, updates endpoint, download
      redirect chain
- [x] A hand-installed mod shows in game (ZZZ via HoYoPlay; confirmed by the user 2026-10-23)
- Moved to "Before Phase 2" below: `DISABLED` hides a mod, nested `Mods/GMM/<mod>/` loads,
  `.staging` ignored, Quick Start command works, screenshots
- Deferred to pre-release: GameBanana's rules for API clients (no public terms found; ask them)

## Phase 0b — Setup check ✅ done 2026-10-23

`gmm detect` and `gmm status` tell the user, per game, what's missing before mods can work and
how to fix it. Read-only.

**Built**
- [x] `gmm-detect`: Steam (`SteamPath` registry value → `libraryfolders.vdf` →
      `appmanifest_<appid>.acf` → `common/<installdir>/<subpath>`), **HoYoPlay**
      (`gamedata.dat`, `installPath` values only), Epic manifests (fixture-tested only; no
      machine with Epic yet), manual folders (`--folder game=PATH`); every candidate validated
      against the game's folder markers; deduped
- [x] VDF/ACF parser (bounded depth and size, property-tested for panics)
- [x] `gmm-xxmi`: finds the root (explicit `--xxmi`, default `%APPDATA%\XXMI Launcher`,
      uninstall key `InstallLocation`), validates it (config file + launcher exe with `MZ`
      header), reads the config through a struct that declares only the allowed fields
- [x] Setup states with fix steps: `XxmiNotFound`, `ImporterMissing`, `ImporterBroken`,
      `GamePathNotSet`, `PathMismatch`, `LaunchModeMismatch`, `Ready`
- [x] Importer details: XXMI DLL version (`d3d11.dll` version resource), importer version (from
      the core ini that `d3dx.ini` includes: ZZMI `main.ini`, WWMI `WuWa-Model-Importer.ini`),
      enabled/disabled mod counts, `Mods/` size
- [x] CLI: `gmm detect` and `gmm status [--game] [--xxmi] [--folder]`, both with `--json`
- [x] Fixtures for every state; secret-canary tests (XXMI's `github_token`, proxy password,
      signatures never appear in output or `Debug`)

**Verified on the dev machine**
- [x] ZZZ (HoYoPlay install on D:, Game Launch = Direct) → `Ready`, with ZZMI 1.50,
      XXMI DLL 1.2.2.0, 1 mod
- [x] WuWa (Steam, Game Launch = Direct, never started through XXMI) → `LaunchModeMismatch`,
      fix: set "Game Launch" to "Steam"
- [ ] WuWa after you switch Game Launch to Steam → `Ready` (needs you)

## Before Phase 2 — in-game checks (needs you at the game)

The install pipeline relies on these. Run `gmm status` afterwards; it shows mod counts.

- [ ] WuWa: set "Game Launch" to "Steam", start it once through XXMI (first start applies the
      settings WWMI needs), confirm `gmm status` says `Ready`
- [ ] Rename `ZZMI\Mods\Phoenix` to `DISABLED Phoenix`: gone in game
- [ ] Move it to `ZZMI\Mods\GMM\Phoenix\`: still loads (nested folder)
- [ ] An empty `ZZMI\Mods\GMM\.staging\` folder causes no errors
- [ ] The desktop **ZZMI Quick Start** shortcut (`--nogui --xxmi ZZMI`) starts ZZZ with mods
- [ ] One WuWa mod shows in game
- [ ] Screenshots of each XXMI step for the setup guide

## Phase 1 — Backend foundations

- [x] Cargo workspace, pinned toolchain, rustfmt/clippy config, `justfile`
- [x] CI on Windows + Linux: fmt, clippy `-D warnings`, tests; `cargo-deny` (licenses, bans,
      advisories, sources)
- [ ] pnpm + Tauri app shell; `pnpm audit` and npm license checks in CI
- [x] `gmm-fsguard` (ScopedPath, RelPath, roots, denylist, junction/symlink escape checks) +
      `disallowed_methods` lints (verified to fire outside fsguard)
- [x] `gmm-net`: host allowlist (incl. every redirect hop), rate limiting, retries/backoff,
      size caps, resumable verified downloads (SHA-256 + MD5)
- [x] `gmm-archive`: hardened zip + 7z extraction, policy scan, malicious archive corpus,
      randomized corruption tests
- [ ] RAR support (C `unrar`, in a sandboxed subprocess)
- [ ] cargo-fuzz targets (archive extraction, `RelPath`, VDF) as a nightly CI job
- [x] `gmm-catalog`: `CatalogSource` trait + GameBanana client (list, search, mod page, files,
      updates, categories), in-memory cache, synthetic fixtures, live API smoke test
- [ ] SQLite-backed HTTP cache (with the database crate)
- [x] `gmm-adapters`: manifest schema (unknown fields rejected), validation, registry with
      lookups by id / GameBanana id / importer / Steam appid; CLI `gmm games`
- [x] CLI: `gmm catalog list|search|show|categories`, `gmm fetch` (download → extract → scan)

Exit: the CLI browses both catalogues and prints a full mod page. Archive harness green. ✅ met
(the open items above don't block Phase 2).

## Phase 2 — Backend features (next)

Already done in Phase 0b: game detection, XXMI detection, setup states, `gmm detect`,
`gmm status`, per-state fixtures.

**Start here** (suggested order):
- [ ] `gmm-db`: SQLite (sqlx) with migrations; tables from 02-architecture.md (installed mods,
      download cache, settings incl. manual game folders and the confirmed XXMI exe path); move
      the catalogue's HTTP cache onto it
- [ ] Install pipeline (`gmm install <mod-id> [--file] [--variant]`): download → verify →
      extract → classify (migoto ini markers from the manifest) → variants → scan → stage under
      `Mods/GMM/.staging/` → rename into `Mods/GMM/<name> [gb-<id>]/` → `gmm.json` sidecar.
      Requires `Ready` (or warns). New write root: `<XXMI>/<IMP>/Mods/` only
- [ ] Enable/disable (`DISABLED ` prefix rename), uninstall (Recycle Bin via `trash`),
      unmanaged-mod listing
- [ ] `gmm-migoto` ini parser + conflict index + severity rules + library-mod ignore list
- [ ] Storage accounting (per mod, per game, cache, drive free space)
- [ ] `gmm launch`: run the confirmed XXMI exe with `--nogui --xxmi <IMP>` (first launch asks
      the user to confirm the exe path; R7)
- [ ] Adapter conformance suite: one fixture install per game, run through detect → status →
      install → toggle → uninstall
- [ ] CLI: `gmm install`, `gmm list`, `gmm enable|disable`, `gmm uninstall`, `gmm conflicts`,
      `gmm storage`, `gmm launch`
- [ ] Egress + file-access harnesses in CI

Exit: from the CLI alone, install a ZZZ mod, see it in game via `gmm launch`, toggle it, see a
conflict reported, and see it in storage. Privacy harnesses green.

## Phase 3 — Frontend

Spec: [07-frontend-design.md](07-frontend-design.md).

- [ ] Design tokens, app shell, game rail, XXMI state pill, downloads tray
- [ ] Setup/onboarding cards + setup guide stepper (driven by `gmm-xxmi`'s states and steps)
- [ ] Catalogue: virtualized grid, filters (category, character, sort, NSFW), banner cards,
      state badges
- [ ] Mod page: hero banner + slideshow + lightbox, header/stats/license, Description (sanitized),
      Files (file picker, archived files), Credits, Updates
- [ ] Variant picker dialog
- [ ] Library: list/grid, toggles, filters, unmanaged tab, actions menu
- [ ] Conflict dialog + conflict filter
- [ ] Storage screen: segment bar, table, multi-select disable/delete/clear cache
- [ ] Settings + About (notices, disclaimer)
- [ ] Component tests, XSS corpus, fixture-based snapshots

Exit: everything from Phase 2 is reachable in the UI for ZZZ and WuWa, with designed loading,
empty and error states.

## Phase 4 — Power features

- [ ] Update checker (`_tsDateUpdated`, new file ids), one-click update with rollback from the
      cache
- [ ] Profiles (named sets of enabled mods; snapshot/restore `d3dx_user.ini`), share as a JSON
      code
- [ ] Keybind viewer (from `[Key*]` sections)
- [ ] Adopt unmanaged mods (match against GB)
- [ ] Drag-and-drop local archive install (same scan pipeline)
- [ ] 1-click install deep links (`gmm://`) if GameBanana allows

## Phase 5 — Release

- [ ] Windows installer (NSIS/MSI via Tauri), code signing, signed auto-updates from GitHub
      Releases
- [ ] Reproducible-build check, release feature assertion, `SECURITY.md`, docs site
- [ ] Public beta (Windows), then Linux (AppImage/Flatpak) from the same codebase

## Phase 6+ — More games

XXMI importers already exist: **Arknights: Endfield (EFMI)** first (it's in your Steam library),
then Star Rail (SRMI), Genshin (GIMI), HI3 (HIMI). HoYoPlay detection already sees Genshin,
Star Rail and HI3 install paths. Each game goes through the adapter checklist in
[03](03-game-adapters.md).

Later detection sources: Kuro launcher (native WuWa) once a machine with it is available.

## Open questions

1. **Name.** The license is decided (MIT OR Apache-2.0, see [06](06-licensing-and-provenance.md)).
   The product name still needs to be chosen and cleared before publishing.
2. **GameBanana relationship.** Contact them before release about API use and 1-click.
3. **Copyright holder** for the license files (currently "The Gacha Mod Manager Contributors").
