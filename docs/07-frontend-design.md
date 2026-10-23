# 07 — Frontend design

The UI is built in Phase 3, after the backend works from the CLI. Every screen reads from typed
Tauri commands (TS types generated from Rust with `specta` or `ts-rs`). The UI never builds a
path or a URL to the outside world.

## App shell

```
┌───────────┬──────────────────────────────────────────────────────────────┐
│ [ZZZ]     │  Top bar: game name · XXMI state pill · search · [Launch with XXMI] │
│ [WuWa]    ├──────────────────────────────────────────────────────────────┤
│ ───────   │                                                              │
│ Browse    │                         page content                         │
│ Library   │                                                              │
│ Storage   │                                                              │
│ ───────   │                                                              │
│ Settings  │                                       downloads tray ▲ (2)   │
└───────────┴──────────────────────────────────────────────────────────────┘
```

- **Game rail** (left): one icon per supported game. Games that aren't detected appear dimmed,
  with a "Set up" badge.
- **XXMI state pill** in the top bar: green *Ready*, amber *Needs setup*, red *XXMI not found*.
  Clicking it opens the setup guide.
- **Launch with XXMI** is enabled only in `Ready`. A tooltip carries the "mods only load through
  XXMI" notice.
- **Downloads tray**: progress, pause/resume, cancel, errors with a retry button.
- Theme: dark by default, light available. Tailwind tokens + shadcn/ui. Keyboard navigable.
  Visible focus rings. All images have alt text (the mod name).

## Setup & onboarding

Shown on first run and whenever a game isn't `Ready`. One card per game:

```
┌ Zenless Zone Zero ─────────────────────────────────────────────┐
│ Game     ✓ Found (Steam) C:\…\Zenless Zone Zero\games\ZenlessZoneZero Game │
│ XXMI     ✓ Found  %APPDATA%\XXMI Launcher (v2.4.9)             │
│ ZZMI     ✓ Installed (ZZMI 1.50, XXMI DLL 1.2.2)               │
│ Game path in XXMI   ⚠ Not set yet                              │
│                                                                │
│ Next step: open XXMI, select ZZZ, check the game folder        │
│ (copy: [C:\…\ZenlessZoneZero Game ⧉]), then launch once.        │
│ [Open setup guide]  [Re-check]                                 │
└────────────────────────────────────────────────────────────────┘
```

The guide is a stepper with the text from [03](03-game-adapters.md), our own screenshots, and the
XXMI releases link (opened in the system browser after a confirmation).

## Catalogue (Browse)

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
  the largest available **[verify GB image field names: `_sBaseUrl` + `_sFile530` / `_sFile`]**.
  Cropped with `object-fit: cover`. A placeholder shows when a mod has no image.
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
| Updates tab | Mod updates endpoint **[verify]** |

**Choosing what to install:**
1. **File** (GameBanana level): the Files tab lists each file with version, size, date and scan
   status. The newest non-archived file is pre-selected in the **Install ▾** split button. Files
   with an AV result other than `clean` can't be installed.
2. **Variant** (archive level): after download, if the archive holds several full mods, a picker
   shows each variant's folder name and its preview image if the archive has one. Multi-select is
   allowed, and the default is the first variant.
3. Already installed → the button becomes **Installed ✓ / Update / Reinstall / Uninstall**.

## Library (downloaded mods)

```
[ Enabled 12 | Disabled 4 | Conflicts 1 | Updates 2 | Unmanaged 3 ]   Group by: Character ▾
┌──────┬───────────────────────────┬──────────┬────────┬──────────┬───────┐
│ [on] │ ▣ Ellen - Maid Outfit      │ Ellen    │ v1.2   │ 142 MB   │  ⋯    │
│ [off]│ ▣ Miyabi - Casual          │ Miyabi   │ v2.0   │  88 MB   │  ⋯    │
│ [on]⚠│ ▣ Ellen - Swimsuit         │ Ellen    │ v1.0   │ 120 MB   │  ⋯    │
└──────┴───────────────────────────┴──────────┴────────┴──────────┴───────┘
```

- List view (default) and grid view, using the same banner as the catalogue.
- Toggle = enable/disable (the `DISABLED` rename). The ⋯ menu has: open mod page, open folder in
  Explorer, reinstall, change variant, uninstall (Recycle Bin).
- **Unmanaged** tab: folders in `Mods/` that GMM didn't install. Actions: adopt (search GB for a
  match), open folder. No delete without confirmation.
- A reminder banner while the game isn't `Ready`: "These mods are installed but XXMI isn't set up
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

Like Steam's storage manager, but per game and per mod instead of per drive:

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

Detected folders (games, XXMI root) with a re-scan button, plus manual overrides. Download cache
size cap. NSFW visibility. Theme. Language (i18n-ready from the start). An About page with the
third-party notices and the "not affiliated" disclaimer.

## Frontend quality bar

- Each screen has loading, empty and error states designed, not just the happy path.
- Component tests (Vitest + Testing Library) for the mod card, mod page header, slideshow,
  variant picker, conflict dialog and storage table.
- The description renderer has an XSS test corpus.
- Snapshot of the mod page against recorded GameBanana fixtures (no live network in tests).
