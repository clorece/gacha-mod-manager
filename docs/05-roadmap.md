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
      fix: set "Game Launch" to "Steam". Superseded 2026-10-24: WuWa's Steam copy works with
      Direct too, so it's now `Ready` either way (`direct_launch = true` in its manifest)

## Before Phase 2 — in-game checks ✅ done 2026-10-24 (except screenshots)

The install pipeline relies on these. Confirmed by you in game:

- [x] Rename `ZZMI\Mods\Phoenix` to `DISABLED Phoenix`: gone in game
- [x] Move it to `ZZMI\Mods\GMM\Phoenix\`: still loads (nested folder). Left there; `gmm list`
      shows it as `GMM/Phoenix`, not installed by GMM
- [x] The desktop **ZZMI Quick Start** shortcut (`--nogui --xxmi ZZMI`) starts ZZZ with mods
- [x] One WuWa mod shows in game. Observed: XXMI started the **Steam** copy of WuWa twice with
      "Game Launch" still on **Direct**, and mods worked. So "Steam copies need Game Launch =
      Steam" (learned from ZZZ's Steam copy, which wraps HoYoPlay) doesn't hold for WuWa, and
      `gmm status` wrongly reports WuWa as `LaunchModeMismatch`
- Dropped: the empty `Mods\GMM\.staging\` check. GMM stages under `DISABLED .staging`, which
  XXMI skips by its own `exclude_recursive = DISABLED*` rule
- [ ] Screenshots of each XXMI step for the setup guide

## Phase 1 — Backend foundations

- [x] Cargo workspace, pinned toolchain, rustfmt/clippy config, `justfile`
- [x] CI on Windows + Linux: fmt, clippy `-D warnings`, tests; `cargo-deny` (licenses, bans,
      advisories, sources)
- [ ] pnpm + Tauri app shell; `pnpm audit` and npm license checks in CI (moved to Phase 3,
      "Start here")
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
- [x] SQLite-backed HTTP cache (done with `gmm-db` in Phase 2)
- [x] `gmm-adapters`: manifest schema (unknown fields rejected), validation, registry with
      lookups by id / GameBanana id / importer / Steam appid; CLI `gmm games`
- [x] CLI: `gmm catalog list|search|show|categories`, `gmm fetch` (download → extract → scan)

Exit: the CLI browses both catalogues and prints a full mod page. Archive harness green. ✅ met
(the open items above don't block Phase 2).

## Phase 2 — Backend features ✅ done 2026-10-24

Already done in Phase 0b: game detection, XXMI detection, setup states, `gmm detect`,
`gmm status`, per-state fixtures.

**Start here** (suggested order):
- [x] `gmm-db` (2026-10-23): SQLite (sqlx 0.9, bundled SQLite, no macros, no extension
      loading) with migrations in `crates/gmm-db/migrations/`; tables `settings` (XXMI folder,
      confirmed XXMI exe), `manual_game_folders`, `installed_mods`, `download_cache`,
      `http_cache`. The catalogue's HTTP cache now lives there (`HttpCache` became async).
      CLI: `gmm config show|add-folder|remove-folder|set-xxmi|clear-xxmi`; `detect`/`status`
      use the saved folders and XXMI folder; `gmm fetch` records archives in `download_cache`.
      Verified on the dev machine: the cache survives between runs (459 ms → 18 ms for a
      list page)
- [x] Install pipeline (2026-10-23): new crate `gmm-core` (`fetch_archive` → `analyze` →
      `install_analyzed`, plus `install` and `install_gate`). `gmm install <mod-id> [--file]
      [--game] [--variant N|name ...] [--all-variants] [--skip-blocked]` and `gmm list`;
      `gmm fetch` now reuses the same steps and prints kind and variants. Stages under
      `Mods/GMM/DISABLED .staging/`, writes `gmm.json` and an `installed_mods` row, never
      overwrites. `gmm status` now counts mods inside `Mods/GMM/` one by one. The catalogue
      model gained `game_id` (`_aGame`). 12 pipeline tests (variants, blocked files, hostile
      names, collisions, end to end against a mock GameBanana). Verified with a real mod
      (Belle High Waisted, 534637: 47 files, 85 MiB) installed into a throwaway XXMI folder.
      Not yet done: installing into your real `ZZMI\Mods` and checking it in game (that's the
      Phase 2 exit check, with `gmm launch`). Known gap: `installed_mods` doesn't record which
      XXMI root a mod went into; `gmm list` reconciles with disk in the next item
- [x] Enable/disable, uninstall, unmanaged listing (2026-10-23): `gmm-core::library`
      (`library`, `set_enabled`, `uninstall` with a `Trash` trait; `RecycleBin` in the app).
      The disk is the truth for on/off: hand renames update the DB; a vanished folder is
      reported `missing` and uninstall then only removes the record; both names present is
      `ambiguous` and refused. Unmanaged = folders and loose `.ini` files in `Mods/`, plus
      unknown folders inside `Mods/GMM/`; listed only, never touched. `Guard::move_to_trash`
      (new `trash` dependency, in fsguard). CLI: `gmm list` (per game, with disk state and
      unmanaged mods), `gmm enable|disable|uninstall <id>...`. Verified in a throwaway XXMI
      folder (install, disable, enable, uninstall to the real Recycle Bin) and as a read-only
      `gmm list` of the real setup (Phoenix shown as unmanaged). An interrupted install left
      only a staging leftover, no mod and no record. Known: SQLite may reuse the id of the
      most recently deleted mod
- [x] Conflicts (2026-10-23): new crate `gmm-migoto` (tolerant parser: override hashes,
      `match_first_index`, geometry/texture replacement, namespace, key bindings; property
      tests) and `gmm-core::conflicts` (`conflicts`, `conflicts_for`). Library mods ignored by
      namespace via the manifest. CLI: `gmm conflicts`, `gmm enable` refuses a conflicting mod
      unless `--anyway`, `gmm install` warns. Verified: the real Belle mod and an unmanaged copy
      of it conflict on 20 hashes; the real setup (Phoenix alone) has none. Deferred: the
      "same character" warning tier (needs the category stored at install) and caching the index
- [x] Storage accounting (2026-10-23): `gmm-core::storage` (`game_storage`, `cache_storage`,
      `clear_downloads`), `Guard::available_space` (new `fs4` dependency). CLI: `gmm storage
      [--game] [--clear-downloads]`. Verified on the real setup: Phoenix 123.4 MiB, 369.9 GiB
      free, cache 22.7 MiB of downloads
- [x] `gmm launch <game> [--yes]` (2026-10-23): `gmm-xxmi::launch` (the only process launch),
      `gmm-core::{plan_launch, confirm_exe, start}`. Requires `Ready`; asks once to confirm the
      exact exe path (stored, asked again if it changes). Verified on the dev machine up to the
      confirmation (WuWa refused as not Ready; ZZZ asks for confirmation). First real run
      (2026-10-24) failed with Windows error 740: XXMI's exe requires administrator rights. Fixed
      by starting it through the shell (`ShellExecuteExW`, UAC prompt), the one `unsafe` call
      in GMM, guarded by a CI check
- [x] Adapter conformance suite (2026-10-23): `crates/gmm-core/tests/conformance.rs` builds a
      fake machine from each built-in manifest alone (game folder from the markers, XXMI with the
      importer, a hand-installed mod, secret canaries in the config) and runs detect → `Ready` →
      install → toggle (XXMI's mod counts follow) → conflicts → storage → uninstall. It also
      asserts that nothing outside `Mods/` changed (size and mtime of every other file and
      folder) and that no canary reached GMM's database, cache or the mod folder. Passes for ZZZ
      and WuWa
- [x] CLI: `gmm install`, `gmm list`, `gmm enable|disable`, `gmm uninstall`, `gmm conflicts`,
      `gmm storage`, `gmm launch`
- [x] Egress + file-access harnesses (2026-10-23): `crates/gmm-cli/tests/privacy.rs` runs the real
      binary behind a refusing, logging proxy with a canary-filled fake profile; on Linux CI
      every `gmm` process also runs under `strace` (no non-loopback connections, no canary
      opened, no writes outside `GMM_HOME`/`Mods/`). `GMM_HOME` relocates GMM's folders.
      `just audit-privacy`. Deferred: Windows syscall tracing (Process Monitor) and a
      firewall-based bypass check

Exit: from the CLI alone, install a ZZZ mod, see it in game via `gmm launch`, toggle it, see a
conflict reported, and see it in storage. Privacy harnesses green.

**Exit check (done with you, 2026-10-24):**
- [x] `gmm install 688491` (Casual Velina) into your real `ZZMI\Mods\GMM\` (2026-10-24; GMM
      id `#1`)
- [x] `gmm launch zzz`, exe confirmed once, game starts with mods (after the UAC fix)
- [x] `gmm disable 1` / `gmm enable 1` rename Casual Velina's folder with and without
      `DISABLED ` (the prefix you confirmed hides a mod in game). The first try used the
      GameBanana id (`688491`); GMM now accepts `#1` and says which GMM id a GameBanana id
      means. `gmm disable Phoenix_duplicate` switched the hand-made copy off the same way
- [x] WuWa's Steam copy was wrongly flagged `LaunchModeMismatch` (it runs fine with XXMI's Game
      Launch on Direct). Fixed 2026-10-24 with a per-game manifest flag,
      `[detect.steam] direct_launch` (WuWa `true`, ZZZ `false`); `gmm status` now says WuWa is
      `Ready`
- [x] `gmm conflicts` reports a conflict: `GMM/Phoenix` and a hand-made copy
      `GMM/Phoenix_duplicate`, 8 shared hashes. `gmm enable` refusing is covered by tests
- [x] `gmm storage` lists Casual Velina (131.0 MiB) and the hand-installed mods
- [x] Hand-installed mods: `gmm enable|disable|uninstall` take their name or path too
      (2026-10-24; uninstall lists them and asks first)
- [x] Privacy harnesses green in CI (egress + file access, strace on Linux)

## Phase 3 — Frontend

Spec: [07-frontend-design.md](07-frontend-design.md). Every screen sits on a `gmm-core` call the
CLI already makes, so this phase adds presentation, not features.

**Start here** (suggested order):
- [ ] App shell in `apps/desktop/`: Tauri 2 (its Rust side depends on `gmm-core`), Vite + React
      + TypeScript + Tailwind, pnpm (this machine has Node 24 and npm; pnpm via corepack). The
      main window's capability lists only GMM's own commands (no `fs`, `shell`, `http`
      plugins); CSP from [04](04-security.md); devtools only behind a `dev` feature (R6).
      CI: `pnpm install --frozen-lockfile`, typecheck, lint, `pnpm audit`, npm license check;
      the Phase 1 item below moves here
- [ ] Command layer: one Tauri command per CLI action, taking ids, never paths (R5): games,
      status, catalog list/search/show/categories, install (progress as events, variant and
      blocked-file prompts as structured errors: `CoreError::ChooseVariant` / `Blocked`),
      library, enable/disable (with `conflicts_for`), uninstall, conflicts, storage, launch
      (plan → confirm → start), config. Folder pickers are opened from Rust and the result is
      validated there. TS types generated from the Rust types (`ts-rs` or `specta`)
- [ ] `gmmimg://` protocol: images fetched through `gmm-net` into `cache/images/` with a size
      cap; the webview never touches the network
- [ ] Then the screens, in this order:
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
