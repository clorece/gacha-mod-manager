# 07 — Frontend design

The app is built in Phases 3–5 (foundations, design, screens), after the backend works from
the CLI. Every screen reads from typed Tauri commands (TS types generated from the Rust types with `ts-rs` into
`apps/desktop/src/bindings.ts`). The UI never builds a path or a URL to the outside world.

Navigation, layout, tokens and the look of each screen are in
[08-design-system.md](08-design-system.md) (Phase 4). This doc keeps the screen contents and
the data each one shows.

## App shell

A left sidebar with three groups (General: Dashboard, Downloads, Settings; Mods: Select
Game, Mods Library, Mods Store; Development: Report a Bug, Documentation) and the page on the
right. Details in [08](08-design-system.md#navigation).

- **Launch** lives on the Dashboard's game bubbles, enabled only in `Ready`, with the "mods
  only load through XXMI" line beside it. First launch confirms the exe path (R7). Since
  2026-10-26 the open bubble of a Ready game can apply one of its profiles before launching
  (the same apply as the Library's Profiles panel, which keeps saving and sharing).
- **Downloads** is a page (grouped by game, with a finished state that clears itself), not a
  tray; the sidebar's count bubble counts installs queued, running or waiting. Since
  2026-10-26 every install downloads and is checked there in the background, and one whose
  check found something waits as "Needs action" (versions to choose, unsafe or outdated
  files) until the user reviews it; the choices that need no download are made in the
  install confirmation first.
- **Startup check**: a screen while GMM starts (app update, new mods per game, updates for
  installed mods), skippable. See [08](08-design-system.md#startup-check).
- Keyboard navigable, visible focus rings, all images with alt text (the mod name).

## Setup & onboarding

Shown on the Dashboard in place of a game's Launch button whenever it isn't `Ready`, and on
first run. One card per game, filled from
`gmm_xxmi::GameStatus` (the same data `gmm status --json` prints today):

```
┌ Wuthering Waves ───────────────────────────────────────────────┐
│ Game     ✓ Found (Steam) C:\…\steamapps\common\Wuthering Waves │
│ XXMI     ✓ Found  %APPDATA%\XXMI Launcher (v2.4.9)             │
│ WWMI     ✓ Installed (WWMI 1.10, XXMI DLL 1.2.2.0)             │
│ Launch   ⚠ XXMI's "Game Launch" is "Direct"; Steam copies      │
│            need "Steam"                                        │
│                                                                │
│ Next step: in XXMI, select Wuthering Waves → settings →        │
│ "General" → "Game Launch" → "Steam".                           │
│ [Open setup guide]  [Re-check]                                 │
└────────────────────────────────────────────────────────────────┘
```

Each `SetupState` maps to one highlighted row (XXMI, importer, game folder, launch) and its
`steps` become the "Next step" text (the CLI's own "run `gmm status` again" line is left out;
the card has **Check again**).

**Setup guide** (built 2026-10-25, `components/SetupGuide.tsx`, opened from the card's
**Open setup guide**): a dialog with four steps in XXMI, **text only** (the project owner's
decision; no screenshots), using XXMI's own labels:

1. **Install XXMI Launcher**: the official releases page (opened in the browser after the
   native confirmation), the `.msi` or the portable `.zip`, start it once; "It's somewhere
   else…" picks its folder.
2. **Select the game**: tick it under "Select Games To Mod:".
3. **Install the mod loader**: press "Install" (the importer, e.g. WWMI).
4. **Follow XXMI's prompts**: XXMI checks the game settings mods need and asks for changes
   itself (for example HD instead of UHD for a Steam copy, or where the game is), then
   "Start" once.

The setup state picks the opening step (XXMI not found → 1, importer missing → 2, importer
damaged → 3, game folder or launch setting → 4, with what GMM sees shown as a notice). Done
steps are ticked; **Check again** re-runs detection and moves on.

**First run** (built 2026-10-25, `pages/Welcome.tsx`): once, after the first startup check,
a "Before you start" card with only the theme (Dark / Light / Match Windows), **Show mods
marked as sensitive** (off) and **Blur their previews until hovered** (on; only while
sensitive mods are shown), a line saying they can be changed from the Store and Library
filters and in Settings, and Continue. Whether the featured gallery shows sensitive mods
follows that choice.

## Mods Store (catalogue)

Layout, featured gallery and filters: [08](08-design-system.md#mods-store). **Built
2026-10-25** (`pages/Store.tsx`; what differs from the sketch below is noted in the list).
Contents:

```
[Search…………]  Category ▾  Character ▾  Sort: Recommended ▾  (○ Hide installed) (○ Show sensitive)
┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐
│  banner    │ │  banner    │ │  banner    │ │  banner    │
│  (16:9)    │ │            │ │  [Installed]│ │ [Update ↑] │
├────────────┤ ├────────────┤ ├────────────┤ ├────────────┤
│ Mod name   │ │ …          │ │ …          │ │ …          │
│ ◉ author   │ │            │ │            │ │            │
│ Ellen · ♥1.2k · ⬇8.4k · 3d │ │            │ │            │
└────────────┘ └────────────┘ └────────────┘ └────────────┘
```

- **Banner** = the first GameBanana preview image (`_aPreviewMedia._aImages[0]`). The same image
  is the card thumbnail and the mod page hero. Card size uses the ~530px variant, the hero uses
  the original (`_sBaseUrl` + `_sFile530` / `_sFile`, verified; exposed as
  `gmm_catalog::Image::url_for_width`). Cropped with `object-fit: cover`. A placeholder shows
  when a mod has no image.
- Card fields (built): name, submitter avatar + name, character (or the root category, e.g.
  "UI") as a badge, likes, views, downloads (filled in for the cards on screen), the game
  under All games, *Sensitive* when shown, and one state badge: *Update* (the startup check
  found a newer file), *Conflict*, *Installed*, or *Off* (installed, every version off). The
  *Conflict* badge is only for installed mods that clash with a mod that's on (a mod that
  isn't installed can't be checked before its archive is fetched). No relative "updated"
  date on cards (the sort covers it).
- **Clicking a card** (anywhere but its Download button) opens GMM's own mod page.
- **Sort** (decided 2026-10-25): Recommended (GameBanana's default, the default here),
  Recently updated, Newest, Most downloaded, Most liked, Most viewed.
- **Toggle chips** (decided 2026-10-25): **Hide installed** and **Show sensitive**, each a
  pill with a small slider switch like the sidebar's theme switch. Show sensitive is the same
  preference as on the first-run screen and in Settings.
- **NSFW**: `_sInitialVisibility = warn/hide` or content ratings → hidden by default. If the user
  shows them, they are blurred until hovered (the "blur" preference), in the grid and in the
  featured gallery, which then includes them too.
- Infinite scroll with a virtualized grid (TanStack Virtual; rows of cards, columns from the
  width), 30 cards a page, skeleton rows while loading; when sensitive or installed mods are
  left out and a page comes back thin, it keeps reading (up to 12 pages) until the screen is
  full.
- Filters map to GB categories: Category lists the game's root categories (from its
  manifest; disabled under All games), Characters the chosen skins category's subcategories
  with their icons (up to 6 at once; "Bangboo" for ZZZ's Bangboo Skins), and the search is
  GameBanana's name filter, so it combines with both and with the sort.
- The featured gallery is hidden while a search is typed, so results start at the top.

## Mod page

```
┌──────────────────────────────────────────────────────────────────────────┐
│                       HERO BANNER (image 1, large)                        │
│   ◀                                                                ▶      │
│   ▢ ▢ ▣ ▢ ▢ ▢   ← thumbnail strip (all preview images), click = jump      │
├──────────────────────────────────────────────────────────────────────────┤
│ Ellen - Maid Outfit                          [Install ▾]  [♡ on GB ↗]     │
│ by ◉ Author · Character Skins › Ellen · v1.2                               │
│ Added 12 Mar 2026 · Updated 3 Oct 2026 · ♥ 1.2k · ⬇ 8.4k · 👁 40k           │
│ License: ✓ install  ? redistribute  ✗ commercial                          │
├───────────── Description │ Files (3) │ Updates │ Credits ─────────────────┤
│ (sanitized HTML from GameBanana)                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Built 2026-10-25** (`pages/ModPage.tsx`, `components/Slideshow.tsx`): the slideshow sits
above the header rather than behind it, the install button is a plain Download (the file
choice happens in the install confirmation, or per file on the Files tab), and an
installed mod shows its state with Add versions (several versions) and Mod settings.
**A full page of its own** (decided 2026-10-25), built from this sketch and the existing
tokens and pieces (not designed on the canvas first; reviewed from screenshots of the real
app). It opens from a click anywhere on a Store card, the featured banner's View details,
and the Library's mod settings; Back returns to the Store with its scroll position and
filters. A small "View on GameBanana" link opens the mod's GameBanana page in the browser
(after the native confirmation).

**Slideshow**: built by hand (decided 2026-10-25, no carousel library). Arrow buttons,
keyboard ←/→, a thumbnail strip, and a click opening a full-screen lightbox (a `<dialog>`).
Images load lazily through the `gmmimg://` cache. With one image, there are no controls.
With none, a placeholder. Sensitive previews follow the blur preference; description images
on hosts GMM doesn't load show a quiet placeholder.

**Field mapping** (from `Mod/{id}/ProfilePage` + `DownloadPage`):

| UI | GameBanana field |
|---|---|
| Title, version | `_sName`, `_sVersion` |
| Author | `_aSubmitter._sName`, `_sAvatarUrl`, `_sProfileUrl` |
| Credits tab | `_aCredits[]._sGroupName` → `_aAuthors[]._sName/_sRole` |
| Category › character | `_aSuperCategory._sName` › `_aCategory._sName` |
| Dates | `_tsDateAdded`, `_tsDateUpdated`, `_tsDateModified` |
| Stats | `_nLikeCount`, `_nDownloadCount`, `_nViewCount` |
| Description | `_sText` (sanitized) |
| License chips | `_aLicenseChecklist.yes/ask/no`, full text `_sLicense` (sanitized) in a dialog |
| Images | `_aPreviewMedia._aImages[]` |
| Files tab | `_aFiles[]`: `_sFile`, `_sVersion`, `_nFilesize`, `_tsDateAdded`, `_nDownloadCount`, `_sDescription`, `_sAvResult`; older files in a collapsible "Archived files" group (`_aArchivedFiles[]`) |
| Obsolete / withheld | `_bIsObsolete`, `_bIsWithheld`, `_bIsTrashed` → banner, install disabled |
| Updates tab | `Mod/{id}/Updates` (verified): title, version, date, changelog entries (`text`, `cat`), file ids |

**Choosing what to install** starts in the install confirmation, which every Download and
Install button opens ([08](08-design-system.md#install-confirmation)); nothing is downloaded
or installed before it's confirmed (since 2026-10-26). GameBanana can't list an archive's
contents, so what needs the archive is decided after the background check, in Downloads:
1. **Variants** (GameBanana level, in the confirmation): when the mod has several files, a
   checklist with each file's description, size and date; the newest clean file is ticked,
   and several can be ticked (each is its own install, switched on). Files with an AV result
   other than `clean` can't be chosen. The Files tab on the mod page shows the same list,
   archived files included.
2. **Conflict** (in the confirmation): GMM-installed mods on for the same character: switch
   them off, install this one switched off, or keep both on.
3. **Versions** (archive level, in Downloads): when the archive holds several full mods, the
   install waits as "Needs action" after GMM has downloaded and checked it; Review shows a
   checklist of them (folder name, files, size), all ticked. Every picked version is
   switched on except one that replaces the same things as one picked before it. Blocked
   files and an outdated (archived) file are shown there too, with Cancel download.
4. Already installed → the button opens the mod's settings (or "Add versions" for a mod with
   several), with Update / Reinstall / Uninstall there.

## Mods Library (installed mods)

Layout: [08](08-design-system.md#mods-library). Built 2026-10-25 (`pages/Library.tsx`, with
the Mod settings panel); the Conflicts filter and the per-mod "conflicts with" panel below
are not built (cards carry a Conflict badge instead). Contents and actions:

- Every installed mod for the selected game (or all games): GMM's mods and hand-installed
  ones ("Added by hand"), each with an on/off switch (the `DISABLED` rename).
- Bulk: Enable all, Disable all, and Select mode, whose bar has **Enable selected**, **Disable
  selected** and **Delete selected** (decided 2026-10-25). Enabling and disabling need no
  confirmation; deleting opens the itemized uninstall confirmation (`UninstallDialog`), then
  deletes the folders for good (R8).
- **Update all (N)** in the header (decided 2026-10-25) queues every update the startup check
  found, one at a time through the install queue.
- **Show sensitive** chip in the filters: the same preference as the Store's; it hides
  installed mods GameBanana flags (their rating is stored at install).
- **Mod settings** (the settings button on each card, [08](08-design-system.md#mod-settings)):
  on/off, versions (switch which one is on, install more from the cached archive, remove
  one), details (version, file, size, folder), actions (open folder, view on GameBanana,
  check for updates, reinstall), per-mod options as they arrive (key bindings, Phase 6),
  uninstall (built: the itemized confirmation says the delete is permanent and that GMM's
  downloaded copy goes too; hand-installed folders get a red warning that GameBanana can't
  give them back; R8).
  Adopt a hand-installed mod (match it on GameBanana; Phase 6, built). Update / roll back
  and key bindings are built too (2026-10-25).
- **Profiles** (Phase 6, built): per game, from the Library toolbar. Save which mods are on,
  apply a profile (exactly those on, the rest off), share it as a code (GameBanana ids; the
  other side sees which mods it lacks).
- The search and filter row stays at the top while the grid scrolls (also in the store).
- A reminder while the game isn't `Ready`: "These mods are installed but XXMI isn't set up
  for this game yet."

## Conflict dialog

Opens when enabling or installing a mod that conflicts with enabled mods:

```
┌ Conflict ───────────────────────────────────────────────────────┐
│ "Ellen - Swimsuit" replaces the same model parts as:             │
│   ☑ Ellen - Maid Outfit   (body, hair: 3 shared hashes)          │
│                                                                  │
│ [Disable selected and enable]   [Enable anyway]   [Cancel]        │
└──────────────────────────────────────────────────────────────────┘
```

Warnings (same character, no shared hashes) show only as badges and never block. The library has
a **Conflicts** filter, plus a per-mod "conflicts with" panel.

## Storage

No page of its own: Settings › Storage shows the cache sizes with Clear buttons and opens
this breakdown. Like Steam's storage manager, but per game and per mod instead of per drive:

```
Zenless Zone Zero ▾                        Drive C: 112 GB free of 1 TB
████████████████░░░░░░░░░░░░░░░░░░░░░░░░  Mods 3.4 GB · Cache 1.1 GB
[Ellen ▓▓▓▓][Miyabi ▓▓▓][Jane ▓▓][Other ▓]   ← segment bar, top 8 + "other"

☐ │ Mod                          │ Character │ Size     │ State    │ Last used
☑ │ Ellen - Maid Outfit           │ Ellen     │ 142 MB   │ Enabled  │ 3 Oct
☐ │ Miyabi - Casual               │ Miyabi    │  88 MB   │ Disabled │ 1 Sep
                                                    [Disable] [Delete…] [Clear cache]
```

- Game picker plus an "All games" overview (one bar per game).
- Sortable columns. Checkbox multi-select, with select all / select disabled.
- **Delete…** confirms with the count and total size, then deletes for good (R8).
  **Clear cache** removes only the cached archives.
- Decided 2026-10-25 for Phase 5: the full breakdown (bar, table, multi-select disable and
  delete) **without** the "Last used" column (GMM doesn't record it) and without the drive
  line; both can come later. Built 2026-10-25 (`pages/Storage.tsx`): the bar shows the
  eight biggest mods plus "Other" (by mod, not by character); the per-mod "Clear cache"
  button isn't built (the download cache is cleared as a whole in Settings).

## Settings

Sections per [08](08-design-system.md#settings): Appearance, Mods Store (sensitive mods), XXMI
Launcher, Games, Storage, About. Built 2026-10-25 (`pages/Settings.tsx`). Later: a download cache size cap, language (i18n-ready from
the start). An About page with the
third-party notices and the "not affiliated" disclaimer. Appearance and the sensitive-mods
switches read and write the stored preferences (built 2026-10-25: `preferences` /
`set_preferences`, `src/prefs.tsx`).

## Documentation page

Decided 2026-10-25: the guides are Markdown files in the repo (readable on GitHub too),
turned into the app's pages at build time, so editing a guide updates the app in its next
build (installed copies don't fetch anything). Drafted short and friendly, linking to the
repo docs for detail; the project owner edits them. Built 2026-10-25: six guides in
[guides/](guides/), shown by `pages/Docs.tsx`.

## Report a Bug

Decided 2026-10-25 (Phase 6): a pre-filled GitHub issue opened in the browser through the
confirmed opener (`github.com/clorece/gacha-mod-manager/issues/new?...`, the one link added);
the user reviews GMM's local error log in the app and attaches it themselves. GMM itself
never connects to GitHub (R2 unchanged), and no account or token is involved. GMM keeps a
small local log for this (errors only, rotated, no paths that identify the user).

Built 2026-10-26 (`pages/ReportBug.tsx`): three cards. **Your report**: a title (required,
120 characters) and "What happened?", with "Open on GitHub…" (native confirmation, then the
browser). **What GitHub will get**: a live preview of the issue as Rust writes it
(`bug_report_draft`, debounced), with a warning when the description was shortened to fit
the link. **Error log**: the newest 50 entries, newest first (local time, source, message),
with Open log folder and Clear log. The webview only sends what was typed; Rust adds the
setup lines and builds the address.

## Frontend quality bar

- Each screen has loading, empty and error states designed, not just the happy path.
- Component tests (Vitest + Testing Library) for the mod card, mod page header, slideshow,
  variant picker, conflict dialog and storage table.
- The description renderer has an XSS test corpus.
- Snapshot of the mod page against recorded GameBanana fixtures (no live network in tests).
- Built 2026-10-25: all three, in CI (`pnpm test`). The mod card is tested through the
  Library, the conflict dialog is the Library's conflict notice, and the fixtures are
  synthetic data in GameBanana's shape (`src/test/fixtures.ts`), typed against the Rust
  types. A contrast and overlap audit of every screen in both themes runs in CI too
  (`pnpm ui-audit`).
