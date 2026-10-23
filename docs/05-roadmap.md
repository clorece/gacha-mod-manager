# 05 — Roadmap

Backend and frontend are separate phases. Phases 1–2 build every feature behind the `gmm` CLI.
Phase 3 builds the UI on top of it. Security harnesses are added in the same phase as the code
they guard ([04](04-security.md)).

## Phase 0 — Discovery on the dev machine (no app code)

Partly done 2026-10-22; see [research/phase0-observations.md](research/phase0-observations.md).

- [x] Locate XXMI root, config keys, ZZMI folder layout, `d3dx.ini` (`include_recursive = Mods`,
      `exclude_recursive = DISABLED*`)
- [x] Locate ZZZ and WuWa Steam installs (appids, ZZZ's nested `games/ZenlessZoneZero Game`)
- [ ] Launch ZZZ once through XXMI; record what changes (`game_folder`, `d3dx_user.ini`, game settings file)
- [ ] Install WWMI for WuWa in XXMI; record the new `WWMI/` folder and config changes
- [ ] Write down XXMI GUI labels for the setup guide; take our own screenshots
- [ ] Confirm the XXMI exe path and that `--nogui --xxmi ZZMI` / `WWMI` launches the game with
      mods
- [ ] Test "start XXMI first, then the game from Steam": do mods load?
- [ ] Test that a nested `Mods/GMM/<mod>/` loads, that a `DISABLED …` folder is skipped, and
      whether a `.staging` folder is ignored
- [ ] Install one mod by hand per game and confirm it renders
- [ ] Confirm GameBanana list/page/sort params, image fields, updates endpoint; check GB rules
      for API clients

Exit: every **[verify]** in the docs is resolved, or explicitly deferred.

## Phase 1 — Backend foundations

- [x] Cargo workspace, pinned toolchain, rustfmt/clippy config, `justfile`
- [x] CI on Windows + Linux: fmt, clippy `-D warnings`, tests; `cargo-deny` (licenses, bans,
      advisories, sources)
- [ ] pnpm + Tauri app shell; `pnpm audit` and npm license checks in CI
- [x] `gmm-fsguard` (ScopedPath, RelPath, roots, denylist, junction/symlink escape checks) +
      `disallowed_methods` lints (verified to fire outside fsguard)
- [ ] `gmm-net` (allowlist, redirect checks, rate limit, resumable downloads)
- [x] `gmm-archive`: hardened zip + 7z extraction, policy scan, malicious archive corpus,
      randomized corruption tests
- [ ] RAR support (C `unrar`, in a sandboxed subprocess)
- [ ] cargo-fuzz targets (archive extraction, `RelPath`) as a nightly CI job
- [ ] `gmm-catalog`: GameBanana client, SQLite HTTP cache, recorded-response fixtures
- [ ] `gmm-adapters`: trait, registry, manifest schema + validation, conformance-suite scaffold
- [ ] `gmm catalog list|show --game zzz` in the CLI

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
