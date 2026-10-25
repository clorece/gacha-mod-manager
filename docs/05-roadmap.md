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
| 4 — Design | 2026-10-25 | The design canvas (23 artboards, dark and light; signed off) written up in [08](08-design-system.md), file formats in [09](09-archive-formats.md); in the app: the tokens and base styles for both themes in `styles.css`, Nunito bundled (OFL-1.1 exception, `THIRD-PARTY-NOTICES.md`), hand-built primitives in `src/ui` (button, icon button, switch, checkbox, radio, segmented control, dropdown, chip / badge, notice, dialog, side panel, toast, progress bar, spinner, icons), a dev-only style guide page, and the shell restyled to the sidebar (Select Game, Dashboard with bubbles and Launch, Mods Library with switches, conflicts and a first Mod settings panel, the launch dialog) |
| 3 — App foundations | 2026-10-24 | `apps/desktop`: Tauri 2 shell over `gmm-core` with 22 commands (ids only), generated TypeScript types, the `gmmimg` image protocol, locked-down capability / CSP / navigation guard, a first shell (game rail, state pill, setup card, mod list, Launch with XXMI); frontend CI (typecheck, lint, `pnpm audit`, npm license check). Verified in a release build on the dev machine |

Cleanup pass (2026-10-24, after Phase 3): unused functions and a dependency removed; the NSFW
filter, "which game is this mod for", launch planning and post-install conflict checks moved
into shared `gmm-catalog` / `gmm-core` functions used by both the CLI and the app; Biome pinned
to a release outside pnpm's minimum-age window (the auto-added exemption removed); finished
phases condensed in this file.

Phase 4 decisions: the primitives are built by hand (no component library); the theme choice
is kept per window in `localStorage` until the Settings screen stores it. Still open, waiting
on you: the game banner source (08); the banners are the game's colors until then.

## Phase 5 — Screens (in progress)

Every screen sits on commands that exist or are small additions. Order: what unblocks
installs first, then the screens in the order users meet them. Started 2026-10-24. Done so
far: RAR, the whole backend list (2026-10-25), and the first three screens (startup check,
Downloads, install confirmation), each checked in a real `tauri dev` window against the
real XXMI setup. Next: the Dashboard and setup guide polish, then the Mods Store (which
gives the install confirmation its Download buttons).

**Carried over from Phase 4**
- [ ] Game banner source (your decision, 08): swap the styled banners for it once chosen
- [ ] Theme choice into `gmm-db` settings with the Settings screen (Dark / Light / Match
      Windows; `src/theme.ts` already handles all three)

**Backend additions**
- [x] **RAR support** (2026-10-24): the `gmm-extract` helper process, sandboxed (job object
      and self-lockdown on Windows, resource caps on Linux), two-pass checks in GMM, which
      writes every file itself; R7 amended, CI checks the launch places and `unsafe` files;
      19 RAR cases in the malicious-archive corpus. libarchive failed one of five real RAR
      mods, so the helper uses UnRAR 7.23 (vendored; license exception); all five extract
      ([09](09-archive-formats.md#rar-plan-decision-record))
- [x] Install in two steps (2026-10-24): `gmm_core::prepare` + `install_plan` (file, other
      files, versions with blocked files and warnings, installed versions, conflicts between
      versions, target folder) behind the `prepare_install` command; `install_prepared`, and
      `InstallOptions::start_disabled` for "Switch it on after installing"
- [x] Versions as one mod (2026-10-24): the library groups folders by GameBanana mod and
      file (`ModRow.group`); `prepare_install` marks the versions already installed, and
      installing one more reuses the cached archive; removing one is `uninstall`;
      `switch_version` turns one on and the clashing ones off
- [x] Install queue (2026-10-24): `gmm_core::InstallQueue` (per-item state, one install at
      a time) and `StopSignal` (the cancel hook, checked between steps and during the
      download); the app runs one worker; commands `queue_install`, `queue_jobs`,
      `queue_watch` (a channel with the list), `queue_pause` / `queue_resume` (the partial
      download is kept and resumed), `queue_cancel`, `queue_dismiss`, `queue_clear_finished`
- [x] Startup check (2026-10-24): `startup_check` streams its steps: the GMM app step
      (skipped until the Phase 7 updater), the newest catalogue page per game
      (`refresh_catalog`), and the update check of GMM-installed mods (`check_updates`: a
      newer clean file than the installed one; failures become notes)
- [x] Trending (2026-10-25): `CatalogSource::top_subs` over `apiv11/Game/{id}/TopSubs`;
      `gmm_core::featured` keeps the safe-for-work entries in GameBanana's order, reads each
      mod page (views, downloads, character, content rating) and backfills with the most liked
      of the newest 50 to five; command `catalog_featured`
- [x] Download counts for cards (2026-10-25): `CatalogSource::mod_stats`
      (`Mod/{id}?_csvProperties=...`, just the counters, cached an hour); command
      `catalog_stats` streams them for up to 60 cards on its own slow client (2/s)
- [x] Categories and "last played" (2026-10-25): migration 0002 stores the root category and
      subcategory (character) with each installed mod (`ModRow` has them);
      `game_activity` records when GMM's Launch started a game (`GameInfo.last_played`)
- [x] Commands (2026-10-25): `open_mod_page`, `open_xxmi_releases`, `open_link` (a native
      dialog shows the address and asks first; ids or link keys only), `open_mod_folder`
      (folders only, through `Guard::os_path_of_dir`), `clear_http_cache`; the openers live in
      `gmm-xxmi/src/launch.rs` (R7)
- [x] Description HTML (2026-10-25): Rust replaces every address (images → `gmmimg` keys on
      GameBanana's image host, links → `#gmm-link:<key>`, `srcset` dropped; `html.rs`), then
      `RichText` sanitizes with DOMPurify 3.4.17 (strict allowlist) and opens links through
      `open_link`
- [x] `File/{id}/RawFileList` looked at again (2026-10-25): still an empty body (200,
      `Content-Length: 0`), even for files GameBanana flags `_bHasContents`; GMM keeps
      fetching the archive before showing versions

**Screens** (in order)
- [x] Startup check screen (2026-10-25): `pages/Startup.tsx` on `startup_check`; opens the
      app on its own when done, Skip for now; failures shown, the app opens anyway
- [x] App shell: sidebar (Select Game, theme switch, active-download count); Downloads page on
      the install queue (2026-10-25): grouped by game, progress with Pause / Resume / Cancel,
      failed rows with View on GameBanana, installed rows clear after 5 s
- [ ] Dashboard: game bubbles with Launch, setup cards for games that aren't Ready, recently
      installed
- [ ] Setup guide: the failing row highlighted per `SetupState`, a stepper with your
      screenshots
- [ ] Mods Store: featured gallery, sticky toolbar, virtualized grid, filters (category,
      character, sort), state badges (installed, disabled, update available, conflict)
- [x] Install confirmation (2026-10-25): `components/InstallDialog.tsx` from `InstallPlan`:
      one version, choose a file, choose versions (installed ones locked), the clash note,
      blocked files, "no mod found" and unsupported-file notices; the first version picked
      stays on and clashing ones are switched off after install (queue worker). Opened from
      the dev style guide until the Store's Download buttons exist
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
- [ ] XSS corpus through the description renderer (T3): the Rust rewrite has unit tests
      (`html.rs`); the DOMPurify side (`RichText.sanitize`) still needs the Vitest corpus
- [ ] IPC fuzz harness: hostile arguments to every command (R5; so far checked by hand)
- [ ] The contrast and overlap audit from Phase 4, run against the real app in CI

Exit: everything the CLI does is reachable in the app for ZZZ and WuWa (RAR included), with
the designed loading, empty and error states; install (all confirmation paths), versions,
uninstall, storage and settings exercised in the app against the real setup.

## Phase 6 — Power features (in progress)

Started 2026-10-25 at your request, alongside the Phase 5 screens still to build (the Store,
mod page, Settings...). Each feature is built backend first (`gmm-core`, a `gmm` command,
an app command) and shown where a screen already exists; the rest of its UI arrives with its
Phase 5 screen.

- [x] **Update and roll back** (2026-10-25): `gmm_core::change_file` moves every installed
      folder of a GameBanana mod to another of its files, keeping each folder's name (so
      3DMigoto's saved toggles survive) and on/off state; versions are matched by name; a
      failed swap is undone; the previous folders go to the Recycle Bin and their archive
      stays cached, so a roll back downloads nothing (`rollback_choices`). `gmm update`
      (no id: list updates), `gmm rollback`; app: `queue_change_file` (through the queue,
      "Updated" rows), `rollback_files`, Update / Roll back in the Mod settings panel
- [x] **Profiles** (2026-10-25): named sets of switched-on mods per game (migration 0003);
      apply switches exactly those on and the rest off, GMM's and hand-installed mods alike;
      share codes are versioned JSON with GameBanana ids (hand-installed mods left out),
      validated as untrusted input (size, counts, game, name). `gmm profile ...`; app:
      `profiles`, `profile_save` / `apply` / `rename` / `delete` / `export` / `import`,
      the Profiles panel in the Library
- [ ] Profiles: snapshot / restore `d3dx_user.ini` (3DMigoto's saved in-game toggles). Waits
      on your decision: the file is outside `Mods/`, so writing it needs an R3 amendment
- [x] **Key bindings** (2026-10-25): `gmm_core::mod_keybindings` reads every `.ini` of a mod
      (`DISABLED*` skipped, links not followed, conflict-check limits). `gmm keys`; app:
      `mod_keybindings`, Key bindings in the Mod settings panel (key names prettified)
- [x] **Adopt hand-installed mods** (2026-10-25): suggestions from the folder's own
      `gmm.json`, a GameBanana id in its name, then a search; the user picks; the folder
      keeps its name (moved into `Mods/GMM/` only if it was elsewhere) and gets a
      `gmm.json` and a row. `gmm adopt`; app: `adopt_candidates`, `adopt`, Find on
      GameBanana in the Mod settings panel (flagged previews hidden)
- [x] **Install from a local archive** (2026-10-25): `gmm_core::prepare_local` copies the
      archive into the cache (its folder registered read-only for that one read), then the
      same extraction, scan and classification as a download; `install_local`. `gmm
      install-file`; app: a file dropped on the window reaches Rust (R5), or "Install from a
      file" on Downloads opens a native dialog; the local install confirmation (game,
      versions, blocked files)
- [ ] Report a Bug (describe, attach a log the user reviews; needs a destination host, R2)
- [ ] 1-click install deep links (`gmm://`): needs GameBanana to add GMM as a mod-manager
      integration for ZZZ/WuWa (none listed today, [10](10-gamebanana-api.md)); then T6
- [ ] UI still to come with Phase 5 screens: version groups and switching in the Library,
      Update badges on cards, profiles in the Dashboard

## Phase 7 — Release

- [ ] Name chosen and cleared; logo and app icon
- [ ] Windows installer (NSIS/MSI via Tauri), code signing, signed auto-updates from GitHub
      Releases (adds `github.com` to the allowlist, R2)
- [ ] Hardening: `cargo-fuzz` targets as a nightly job; Windows equivalents of the strace checks
      (Process Monitor, a firewall rule for the bypass check); reproducible-build check;
      release feature assertion; `SECURITY.md`
- [ ] Licensing checklist in [06](06-licensing-and-provenance.md) (third-party notices in About)
- [ ] Courtesy contact with GameBanana before the public beta (not a permission: none is
      needed, [10](10-gamebanana-api.md)): say what GMM requests and its User-Agent, ask about
      a 1-click integration for ZZZ/WuWa and the "Powered by GameBanana" wording
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
- The install queue and the startup check's update list live in memory: a restart empties
  the Downloads page (a paused download's partial file stays in the cache and resumes when
  the same file is installed again) and the next startup checks again.
- A cancelled download's partial file stays in `cache/downloads` until that file is
  installed again or the download cache is cleared.
- The RAR helper's Linux lockdown has no seccomp filter yet, and on Windows it runs at Low
  integrity rather than in an AppContainer (it could still read the user's files or open a
  connection if UnRAR were exploited). Phase 7 hardening.
- RAR "file copy" entries (identical files stored once, `rar -oi`) are refused, not copied.
- The install confirmation fetches the archive before showing it (versions are only known
  from inside), so the Downloads page mostly shows the install step; download progress
  appears there only for files that weren't fetched first.
- Description images hosted outside `images.gamebanana.com` aren't shown: third-party
  hosts (imgur and others) never will be (R2), and GameBanana's own file hosts
  (`filecacheNN.gamebanana.com`, `files.gamebanana.com`, common in descriptions) wait on
  your decision.
- The browser openers (`open_mod_page`, `open_link`, `open_xxmi_releases`) need a click
  in their native dialog, so they haven't been driven end to end yet; their address checks
  have unit tests. The same goes for a real file drop and the "Install from a file" dialog:
  the local install pipeline is tested in `gmm-core`, not yet through the window.
- GameBanana publishes no rate limit; GMM's budget (2 API requests/s per install) is a
  guess, backed by the shared cooldown ([10](10-gamebanana-api.md)). The cooldown and the
  "slow down" notice have only been exercised against a mock server.
- Applying a profile renames the mods it switches (`DISABLED `), so 3DMigoto forgets their
  saved in-game toggles until `d3dx_user.ini` snapshots exist (above).
- Updating an adopted mod replaces the files that were there by hand with GameBanana's.

## Open questions

1. **Name.** The license is decided (MIT OR Apache-2.0, see [06](06-licensing-and-provenance.md)).
   The product name still needs to be chosen and cleared before publishing.
2. **GameBanana relationship.** No permission needed ([10](10-gamebanana-api.md)); a courtesy
   contact before the public beta, mainly for 1-click.
3. **Copyright holder** for the license files (currently "The Gacha Mod Manager Contributors").
4. **`d3dx_user.ini` in profiles**: an R3 amendment letting GMM write that one file in each
   importer folder. Your call.
