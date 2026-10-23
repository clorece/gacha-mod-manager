# 05 — Roadmap

Backend and frontend are separate phases. Phases 1–2 build every feature behind the `gmm` CLI.
Phase 3 builds the UI on top of it. Security harnesses are added in the same phase as the code
they guard ([04](04-security.md)).

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
- Deferred to Phase 0b (needs the game running): mod visible in game, `DISABLED` hides it,
  nested `Mods/GMM/<mod>/` loads, `.staging` ignored, Quick Start command works, screenshots
- Deferred to pre-release: GameBanana's rules for API clients (no public terms found; ask them)

## Phase 0b — Setup check (next session)

Goal: `gmm detect` and `gmm status` tell the user, per game, exactly what's missing before
mods can work, and how to fix it. Read-only: nothing is written anywhere except GMM's own
folders. Uses the adapter registry from Phase 1.

**Build**
- [ ] `gmm-detect`: Steam (`SteamPath` registry value → `libraryfolders.vdf` → `appmanifest_<appid>.acf`
      → `common/<installdir>/<steam_subpath>`), Epic manifests, manual folder; validate against
      each adapter's folder markers; dedupe. Only the exact files allowlisted in 04-security
      R3 are read. HoYoPlay/Kuro launcher sources come later (no test machine has them yet)
- [ ] VDF/ACF parser (small, fuzzed; Steam's text format)
- [ ] `gmm-xxmi`: find the root (default path, uninstall key `InstallLocation`, user-picked),
      validate it (config file + `Resources/Bin/XXMI Launcher.exe` + version resource), read the
      config through a struct that declares only the allowed fields
- [ ] Per-game state machine: `XxmiNotFound`, `ImporterMissing`, `ImporterBroken`,
      `GamePathNotSet`, `PathMismatch`, `LaunchModeMismatch`, `Ready`, each with its fix
      steps (wording from 03-game-adapters.md)
- [ ] Importer details: XXMI DLL version (`d3d11.dll`), importer version (`main.ini`), mod
      count, `Mods/` size
- [ ] CLI: `gmm detect` (installs found, and from where) and `gmm status [--game]` (state +
      next step), both with `--json`
- [ ] Fixtures for every state; secret-canary test for the XXMI config (`github_token` etc.
      never appear in output, logs or JSON)

**Verify on the dev machine (needs you at the game)**
- [ ] Set **Game Launch** to **Steam** for ZZZ and WuWa; `gmm status` then reports `Ready`
- [ ] The Phoenix mod shows in game (its toggle menu opens with H)
- [ ] Rename it to `DISABLED Phoenix`: gone in game
- [ ] Move it to `Mods/GMM/Phoenix/`: still loads (nested folder)
- [ ] An empty `Mods/GMM/.staging/` folder causes no errors
- [ ] Desktop **ZZMI Quick Start** shortcut (`--nogui --xxmi ZZMI`) launches with mods
- [ ] WuWa: first launch through XXMI with one mod
- [ ] Our own screenshots of each XXMI step in the setup guide

Exit: `gmm status` is correct for both games on the dev machine before and after the Game
Launch change, and every fixture state has a test.

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
- [ ] cargo-fuzz targets (archive extraction, `RelPath`) as a nightly CI job
- [x] `gmm-catalog`: `CatalogSource` trait + GameBanana client (list, search, mod page, files,
      updates, categories), in-memory cache, synthetic fixtures, live API smoke test
- [ ] SQLite-backed HTTP cache (with the database crate)
- [x] `gmm-adapters`: manifest schema (unknown fields rejected), validation, registry with
      lookups by id / GameBanana id / importer / Steam appid; CLI `gmm games`
- [ ] Adapter conformance suite (runs against fixtures once detection exists, Phase 0b)
- [x] CLI: `gmm catalog list|search|show|categories`, `gmm fetch` (download → extract → scan)

Exit: the CLI browses both catalogues and prints a full mod page. Archive harness green.

## Phase 2 — Backend features

- [ ] `gmm-detect`: Steam (registry → libraryfolders → appmanifest → sub-path), Epic, Kuro,
      HoYoPlay, manual; dedupe + validation
- [ ] `gmm-xxmi`: root detection, safe config read (allowlisted fields), per-importer state
      machine, version info
- [ ] Install pipeline: download → verify → extract → classify → variants → scan → stage →
      `Mods/GMM/`
- [ ] Enable/disable (DISABLED rename), uninstall (Recycle Bin), unmanaged-mod listing
- [ ] `gmm-migoto` ini parser + conflict index + severity rules + library-mod ignore list
- [ ] Storage accounting (per mod, per game, cache, drive free space)
- [ ] Launch with XXMI (allowlisted exe + args)
- [ ] CLI: `gmm detect`, `gmm status`, `gmm install`, `gmm enable|disable`, `gmm conflicts`,
      `gmm storage`, `gmm launch`
- [ ] Fixtures for each XXMI state (incl. your machine's ZZZ=`GamePathNotSet`,
      WuWa=`ImporterMissing`)
- [ ] Egress + file-access harnesses in CI

Exit: from the CLI alone, install a ZZZ mod, see it in game via `gmm launch`, toggle it, see a
conflict reported, and see it in storage. Privacy harnesses green.

## Phase 3 — Frontend

Spec: [07-frontend-design.md](07-frontend-design.md).

- [ ] Design tokens, app shell, game rail, XXMI state pill, downloads tray
- [ ] Setup/onboarding cards + setup guide stepper
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
then Star Rail (SRMI), Genshin (GIMI), HI3 (HIMI). Each one goes through the adapter checklist
in [03](03-game-adapters.md).

## Open questions

1. **Name.** The license is decided (MIT OR Apache-2.0, see [06](06-licensing-and-provenance.md)).
   The product name still needs to be chosen and cleared before publishing.
2. **GameBanana relationship.** Contact them before release about API use and 1-click.
3. **Copyright holder** for the license files (currently "The Gacha Mod Manager Contributors").
