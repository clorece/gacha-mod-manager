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
  only load through XXMI" line beside it. First launch confirms the exe path (R7).
- **Downloads** is a page (grouped by game, with a finished state that clears itself), not a
  tray; the sidebar badge counts active installs. Choices never wait there: they're made in
  the install confirmation first.
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
`steps` become the "Next step" text. The guide is a stepper with the text from
[03](03-game-adapters.md), our own screenshots, and the XXMI releases link (opened in the
system browser after a confirmation).

## Mods Store (catalogue)

Layout, featured gallery and filters: [08](08-design-system.md#mods-store). Contents:

```
[Search…………]  Category ▾  Character ▾  Sort: Recently updated ▾  ☐ Show NSFW  ☐ Hide installed
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
- Card fields: name, submitter avatar + name, character (subcategory), likes, downloads, relative
  "updated" date, state badge (*Installed*, *Disabled*, *Update available*, *Conflict*).
- **NSFW**: `_sInitialVisibility = warn/hide` or content ratings → hidden by default. If the user
  shows them, they are blurred until hovered or clicked.
- Infinite scroll with a virtualized grid (TanStack Virtual), cached pages, and skeleton cards
  while loading.
- Filters map to GB categories. The character list comes from the game's subcategories.

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

**Slideshow**: an Embla carousel. Arrow buttons, keyboard ←/→, swipe or trackpad, a thumbnail
strip, and a click opening a full-screen lightbox. Images load lazily through the `gmmimg://`
cache. With one image, there are no controls. With none, a placeholder.

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

**Choosing what to install** happens in the install confirmation, which every Download and
Install button opens ([08](08-design-system.md#install-confirmation)); nothing is installed
before it's confirmed. To show versions it fetches the file into the download cache first
(GameBanana can't list an archive's contents), so the confirmed install starts from the cache:
1. **File** (GameBanana level): when the mod has several files, a radio list with each file's
   description, size and date; the newest clean file is pre-selected. Files with an AV result
   other than `clean` can't be chosen. The Files tab on the mod page shows the same list,
   archived files included.
2. **Versions** (archive level): when the archive holds several full mods, a scrolling
   checklist of them (folder name, files, size), after GMM has fetched the archive into its
   cache to look inside. Multi-select is allowed; the first one picked is switched on, and
   versions that replace the same things can't be on together.
3. Already installed → the button opens the mod's settings (or "Add versions" for a mod with
   several), with Update / Reinstall / Uninstall there.

## Mods Library (installed mods)

Layout: [08](08-design-system.md#mods-library). Contents and actions:

- Every installed mod for the selected game (or all games): GMM's mods and hand-installed
  ones ("Added by hand"), each with an on/off switch (the `DISABLED` rename).
- Bulk: Enable all, Disable all, and Select mode for enabling or disabling a chosen set.
- **Mod settings** (the settings button on each card, [08](08-design-system.md#mod-settings)):
  on/off, versions (switch which one is on, install more from the cached archive, remove
  one), details (version, file, size, folder), actions (open folder, view on GameBanana,
  check for updates, reinstall), per-mod options as they arrive (key bindings, Phase 6),
  uninstall (Recycle Bin; hand-installed mods only after the itemized confirmation, R8).
  Adopt a hand-installed mod (match it on GameBanana) comes in Phase 6.
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
- **Delete…** confirms with the count and total size, then moves to the Recycle Bin.
  **Clear cache** removes only the cached archives.
- "Last used" = last time it was enabled while XXMI launched from GMM. Unknown when launched
  elsewhere.

## Settings

Sections per [08](08-design-system.md#settings): Appearance, Mods Store (sensitive mods), XXMI
Launcher, Games, Storage, About. Later: a download cache size cap, language (i18n-ready from
the start). An About page with the
third-party notices and the "not affiliated" disclaimer.

## Frontend quality bar

- Each screen has loading, empty and error states designed, not just the happy path.
- Component tests (Vitest + Testing Library) for the mod card, mod page header, slideshow,
  variant picker, conflict dialog and storage table.
- The description renderer has an XSS test corpus.
- Snapshot of the mod page against recorded GameBanana fixtures (no live network in tests).
