# 08 — Design system

Phase 4's output: how GMM looks and is organized. The interactive mockup lives on a design
canvas in your claude.ai account (private): every screen in dark and light, built with real
GameBanana names and stats (mod preview images are placeholders, since GMM never re-hosts
them). This doc is the written source of truth the app is built from. Screen-level detail for
the catalogue and mod page fields stays in [07](07-frontend-design.md).

**Status (2026-10-25): signed off, in the app.** The layout and theme are signed off; the
second round added the startup check, the install confirmation (versions and files), mod
settings with versions, and the sticky search and filter bar. Every state was checked in both
themes (screenshots plus an automated contrast and overlap check over 62 states). Since
2026-10-25 the same kind of audit runs on the built app in CI (`apps/desktop/tests/ui-audit.mjs`:
20 states in both themes, WCAG AA contrast, no overlapping text, no sideways scroll).

**In the app** (Phase 4): the tokens below, base styles, scrollbars and the `:where()`
button base are in `apps/desktop/src/styles.css`; Nunito is bundled; the primitives are in
`apps/desktop/src/ui` and shown in every state on the dev-only **Style guide** page (sidebar,
Development, dev builds only). Built screens: the sidebar with Select Game, the Dashboard
(bubbles, Launch, recently installed, setup), the Mods Library and Mod settings (complete since the third checkpoint, 2026-10-25),
Settings with the Storage breakdown, and Documentation. Every screen is built. Since 2026-10-25 the game banners carry real key art, the Dashboard bubbles show
"Played … ago" (or "Not played from GMM yet"), and two screens not on the canvas are built
from its tokens: the first-run preferences card and the setup guide. The Mods Store
(gallery, sticky toolbar, grid with real previews) follows the canvas; the mod page isn't on
the canvas and is built from 07's sketch and these tokens (below). Library cards show the preview stored at install (a quiet
image placeholder for hand-installed and older mods).

## Direction

- **Structure from HoYoLAB**: a dark navy ground, layered rounded panels, pill-shaped tabs and
  filters, big banners that fade into the page, an accent used sparingly for the current
  item.
- **Color, type and mark from the GMM brand** ([brand/](brand/)): the glossy capsule mark,
  Nunito (rounded, heavy headings), the sky-blue gradient, candy-yellow highlights.
- Dark is the default; light uses the same layout and token names.

## Navigation

A fixed sidebar on the left (268 px), top to bottom:

| Area | Contents |
|---|---|
| Brand | Capsule mark at 48 px; beside it a column with "Gacha Mod Manager" on one line (Nunito 900, 18 px, title gradient) tight over a small one-line "Powered by GameBanana" pill (10 px, 1 px gap), deliberately quieter than the name and mark. The text column starts 5 px below the mark's top, so the name-pill split sits just under the capsule's horizontal center line. Springy hover on the mark, off with reduced motion |
| **General** | Dashboard · Downloads (yellow badge with the active count) · Settings |
| **Mods** | **Select Game** (banner dropdown, below) · Mods Library · Mods Store |
| **Development** | Report a Bug (`WIP` tag, not in the first beta) · Documentation |
| Footer | Theme switch (dark / light), version |

**Select Game** is a banner button: the selected game's banner with a bottom fade, a small
"Game" label and the game's name. Clicking it opens a list of banners (All games, then each
supported game, each with its installed-mod count); picking one collapses the list and
updates the button. The choice scopes the Mods Library and Mods Store; **All games** shows
every game's mods unfiltered (cards then carry a small game badge). The Library's header
offers the same choice as large banner cards, so the two always agree.

## Screens

### Dashboard
- Header: title, one line on launching, summary chips (mods installed, on, XXMI found).
- **Game bubbles**: one large banner card per game with indicators: mods downloaded (download
  icon + count), last played, setup state (dot + "Ready" or the state). Clicking a bubble
  **enlarges it** (it grows wider and lifts slightly; it doesn't open a page) and reveals the
  **Launch** button (candy yellow, the one yellow button in the app) with a one-line detail
  ("2 of 3 mods on · ZZMI through HoYoPlay"). Clicking again shrinks it. One bubble open at
  a time. A dashed "More games later" bubble holds the place for the next adapter.
- Below: "Recently installed" and "Setup" panels. A game that isn't Ready shows its setup
  card here (states and steps from `gmm-xxmi`) instead of a Launch button.

### Downloads
- Grouped by game (banner chip, name, count), newest first within a group.
- One row per install: thumbnail, mod name, author and file name, state line, progress bar,
  actions. States and their colors:

  | State | Line | Actions |
  |---|---|---|
  | Queued | muted "Queued" | Cancel |
  | Downloading | accent "Downloading · 62% · 16.9 of 27.3 MiB" + bar | Pause, Cancel |
  | Extracting / scanning / installing | accent + indeterminate bar | — |
  | Failed | red reason in plain words | View on GameBanana, Dismiss (Retry when it can help) |
  | Installed | green "Installed · just now", row dims, then removes itself after ~5 s | Show in library |
- "Clear finished" in the header.
- There is no "needs your choice" state: every choice (file, versions, blocked files) is made
  in the install confirmation before a row appears here. A row for a multi-version install
  says how many versions were chosen ("2 of 8 versions chosen") and which one is installing.

### Mods Store
- **Featured gallery** at the top: a large banner (fade from the left for text) with a tag
  ("Trending today"), name, author, character, likes / views / downloads, Download and View
  details, prev/next buttons. Below it a strip of smaller banners; changing the featured mod
  slides the strip left so the current one leads. Source: GameBanana's top submissions for
  the game (see Data, below).
- **Toolbar** (one row, wraps when narrow): search · Category dropdown · Characters
  multi-select · Sort (Recommended, Recently updated, Newest, Most downloaded, Most liked,
  Most viewed) · **Hide installed** and **Show sensitive** toggle chips (pills with a small
  slider switch like the sidebar's theme switch; added 2026-10-25) · (right) result count
  with "N hidden as sensitive". It is **sticky**:
  once the featured gallery scrolls away, the search and filter row locks to the top of the
  page (on the page background, with a soft shadow under it) while the grid keeps scrolling
  beneath it. Open dropdowns stay above the cards.
  - Categories, from GameBanana: ZZZ All Mods / Character Skins / Bangboo Skins /
    Other/Misc / UI; WuWa All Mods / Character Skins / Other/Misc / UI.
  - Characters: multi-select with checkboxes and character icons; nothing ticked = all.
    With ZZZ + Bangboo Skins the list becomes the Bangboo. Disabled (with the reason) for
    All games, Other/Misc and UI.
- **Grid** of mod cards (auto-fill, min 232 px): 16:10 preview, character badge, game badge
  (All games only), "Installed" badge; name (2 lines max), author with avatar, likes, views,
  downloads, and a round Download button (a check when installed). Every Download button
  (cards and the featured banner) opens the install confirmation. On an installed mod the
  button opens its settings, or, for a mod with several versions, the confirmation in
  "Add versions" mode.

### Mods Library
- Header: title, one line ("Switch one off and XXMI skips it…"), **Update all (N)** when the
  startup check found updates (added 2026-10-25), then the game banner cards (All games /
  each game, with "N mods · M on").
- Toolbar: search, Category, Characters and the **Show sensitive** chip on the left;
  **Enable all**, **Disable all** and **Select** on the right. Sticky like the store's,
  together with the selection bar. Select mode adds a selection bar under the toolbar ("N
  selected", a hint, Enable selected / Disable selected / **Delete selected** (red outline,
  opens the uninstall confirmation; added 2026-10-25) / Done) and a checkbox on each card.
- Cards: preview, character badge (mods installed through GMM), a round settings button
  (opens Mod settings), name, "Installed 24 Oct 2026" or "Added by hand", size, On/Off label
  and a switch. A mod with versions shows "2 of 8 versions" on its preview; its switch turns
  the mod's current version on or off.
  As built (2026-10-25): the chip reads "<version that's on> · N versions" (installed ones;
  the archive's total is only known after looking inside), and a mod GameBanana flags shows
  a Sensitive badge and blurs like the Store's; the toolbar also has Profiles and a count
  with "N hidden as sensitive".
- **Conflicts**: switching a mod on that clashes with one that's on shows an amber notice
  ("Phoenix_duplicate conflicts with Phoenix… only one shows") with Keep it off / Enable
  anyway. Enable all applies the same rule.

### Startup check
A full-window screen while GMM starts (dark and light): the mark (gently bobbing), the
lockup, and a card with the heading **Checking for updates**, a status line ("Checking for a
GMM update", "Pulling new GameBanana mods", "Checking your installed mods for updates",
"New software update available", "All set"), a progress bar with what it's working on and a
percentage, and a step list with an icon per step (done, working, waiting, attention):

1. GMM app: compares the version with GMM's GitHub releases (needs the Phase 7 updater;
   until then this step is skipped).
2. One step per game: pulls the newest catalogue pages so the store opens with fresh mods.
3. Your installed mods: compares each GMM-installed mod with its GameBanana page (new files,
   a newer update date) and marks updates in the library.

When an app update exists the status turns amber and a panel offers **Update and restart**
(downloads with progress, signed, then restarts), **Later** and **What's new**; the mod steps
keep running underneath. **Skip for now** is always there, and the app works offline with
cached data. The footer states the only hosts GMM talks to. Cached results keep this fast; a
check that finds nothing should take a second or two.

### Install confirmation
Opens from every Download button; nothing downloads until it's confirmed. One dialog with up
to three parts:

- **Choose a file**, when GameBanana lists several (e.g. Homewear Belle's three zips, each
  with its uploader's description and date): a radio list, the newest clean file first.
- **Choose versions**, when the archive holds several mods (e.g. Fixed Yae Sakura Sword
  Pack, 8 weapon versions): a checkbox list (it scrolls past about 5 rows) with files and
  size per version, "N of M selected" and Select all / Clear. The first version picked gets
  an "On after install" tag. A note explains that versions replacing the same thing can't be
  on together. To know the versions GMM has to look inside the archive: it fetches it into
  the download cache first (the dialog says so) and installs nothing until confirmed. In
  **Add versions** mode (the mod is already installed) the versions you have show as
  locked, dashed rows tagged "Installed".
- **One version**: just the details and the confirmation.

Always shown: the file and size, where it installs ("Zenless Zone Zero › ZZMI\Mods\GMM"),
the safety line (GameBanana's scan plus GMM's own), and **Switch it on after installing**.
The button says what happens ("Install", "Install 2 versions", disabled as "Pick a version"
when nothing is chosen). A file GMM can't install (password-protected or split into several
parts, [09](09-archive-formats.md)) shows a red notice instead, with View on GameBanana;
RAR itself is supported since Phase 5. Blocked files (executables, scripts) are listed
in the same dialog with the option to install without them. Confirming starts the download
and goes to the Downloads page.

### Mod settings
A panel that slides in from the right (480 px) from a library card's settings button:

- Header: the mod's preview, "Mod settings", name, source (GameBanana id, author,
  character) or "Added by hand".
- **Switched on** with the mod's state.
- **Versions** (mods with several): every version in the archive. Installed ones have an
  On/Off switch and a remove button; switching one on switches the others off when they
  replace the same things (from conflict detection between the versions). Versions not
  installed have **Install**, which reuses the cached archive.
- **Details**: version, install date, file (kept in the download cache), size on disk and
  folder count, folder path.
- **Actions**: Open folder, View on GameBanana, Check for updates, Reinstall (from the cache).
  Hand-installed mods get Open folder and Find on GameBanana (matching, Phase 6).
- **More settings**: room for per-mod options as they arrive; Key bindings shows as "Later"
  (Phase 6).
- Built so far (2026-10-25, Phase 6 ahead of the full panel): **Updates** (Update to the
  newer file the startup check found; Roll back to an earlier file from the cache), **Key
  bindings** (each `[Key*]` section: name, keys prettified as "Ctrl + Left", back keys,
  cycle / toggle / hold), and **Actions** as two tiles (Open folder; View on GameBanana, or
  Find on GameBanana for a hand-installed folder, which opens a picker of suggested mods with
  flagged previews hidden). Not designed on the canvas yet, and built from its tokens: the
  Library's **Profiles** panel (save current, apply, copy code, delete, import a code), the
  **local install** confirmation (game, versions, blocked files) opened by a file dropped
  on the window or "Install from a file" on Downloads, and the **GameBanana notice** at the
  top of the page while GMM holds off after a "slow down".
- **Uninstall** (red outline) with a line saying what it does: deleted for good together
  with GMM's downloaded copy (R8, amended 2026-10-25). The confirmation lists each folder and
  its size and says it won't be in the Recycle Bin; hand-installed folders add a red notice
  (GameBanana can't give them back). The same dialog serves the Library's Delete selected.

### First run (built 2026-10-25, not on the canvas)
After the first startup check, the startup screen's layout (mark, lockup, card) with "Before
you start": Theme (segmented Dark / Light / Match Windows), Show mods marked as sensitive and
Blur their previews until hovered (switches with On / Off labels; the blur switch is
disabled while sensitive mods are hidden), a hint that these change later from the Store and
Library filters and in Settings, and a full-width primary Continue. Nothing else.

### Setup guide (built 2026-10-25, not on the canvas)
A dialog from a setup card: a row of four step tiles (number, or a green tick when done; the
current one ringed in `--accent`), the step's title, a short paragraph, a numbered list with
XXMI's labels in bold, and step actions as default buttons. Footer: Check again on the left,
Back and Next / Done on the right. Text only.

### Mod page (built 2026-10-25, not on the canvas)
A pill **Back** link ("← Mods Store"); the slideshow (28 px radius, 16:9 up to 520 px tall,
the image contained on a near-black ground, round art arrow buttons at the sides, an
"n / m" chip, a strip of 112 px thumbnails with the current one ringed in `--accent`); then
the header: an overline with category › character, the 30 px title, a 28 px avatar line
(author, version, dates), counters with icons, and on the right the primary Download (or the
install state badge with Add versions / Mod settings) beside a link-style "View on
GameBanana ↗". License chips use the state badges (✓ ok, ? warn, ✗ bad) with "Read the
license" opening a dialog. Tabs are a pill group (the selected tab in `--btn`); the panel is
a card. Files are bordered rows (mono name, version and "Scanned clean" badges, size · date ·
downloads, the description, a small Install button); archived files fold away. The lightbox
is a full-window `<dialog>` on a 92% dark backdrop.

### Store details added while building (2026-10-25)
The toggle chips are pills with a 30 × 18 px track (`--switch-off`, `--btn` when on).
Sensitive previews blur (18 px) until the card, banner or thumbnail is hovered or focused.
The current strip banner's yellow ring is drawn on a layer above the image. Skeleton cards
shimmer while a page loads; motion is off under reduced motion.

### Settings
Cards: Appearance (Dark / Light / Match Windows), Mods Store (show sensitive mods, blur their
previews), XXMI Launcher (folder, found automatically or chosen), Games (how each was found,
add a folder), Storage (download cache, image cache, each with Clear), About (version,
license, disclaimer, licenses).

Built 2026-10-25 (not on the canvas in detail, from its tokens): cards in one column up to
920 px, each a stack of rows (label and muted line on the left, the control on the right);
Storage lists three caches (downloaded archives, preview images, GameBanana answers) and
"Show breakdown"; About shows the README's Disclaimer and Credits and opens the
third-party notices in a dialog. The **Storage breakdown** is its own page with a pill Back
link: a segmented game choice (All games, each game), a 18 px bar of 2 px-gapped segments
(the eight biggest mods plus a grey "Other", a legend with sizes under it, native tooltips),
then the selection bar and a sortable table. The bar colors are `--viz-1..8`, the dataviz
reference categorical palette per theme (validated against `--card`: dark passes every
check; light has three slots under 3:1, so the legend and table always show).

### Report a Bug (in development)
The page exists in the sidebar with a `WIP` tag. It shows the mark, an "In development"
chip, one paragraph on what it will do (describe the problem, optionally attach an error or
crash log, see everything before it's sent), and a greyed preview of the form. Not in the
first beta.

### Documentation
A grid of guide cards: Getting started, How mods load, Fixing setup problems, Installing
safely, Conflicts, Privacy. Content comes from these docs, rewritten for players.
Built 2026-10-25: cards with a round tinted icon, the guide's title and its first line;
a guide opens as a card up to 820 px (icon and 26 px title, then the Markdown) with
"← Documentation" above and "Next: …" below. The text lives in `docs/guides/`.

## Tokens

All colors are CSS custom properties on the app root; `.t-dark` and `.t-light` set them.

| Token | Dark | Light | Use |
|---|---|---|---|
| `--ground` | `#0B1020` | `#EEF2F8` | Window background |
| `--side` | `#0F1528` | `#FFFFFF` | Sidebar |
| `--card` | `#161D35` | `#FFFFFF` | Panels, cards, inputs |
| `--raised` | `#1E2744` | `#F4F7FC` | Menus, secondary buttons |
| `--line` | `#283152` | `#DCE3EF` | Borders |
| `--hover` | `#232D4F` | `#E8EEF8` | Hover fill |
| `--text` | `#EEF3FF` | `#1B2A4A` | Primary text |
| `--text2` | `#B4C0DD` | `#3F4F73` | Secondary text |
| `--muted` | `#8F9BBC` | `#5A6884` | Captions, counts (≥ 4.5:1 on `--card`) |
| `--accent` | `#7DB2FF` | `#335EEA` | Links, current item text, progress text, focus ring (≥ 4.5:1 on every surface, including the pill highlight) |
| `--btn` | `#335EEA` | `#335EEA` | Primary buttons, active nav, ticked checkboxes (white text, 5.3:1); hover `#2A52D6` |
| `--title-a` → `--title-b` | `#B5F5FF` → `#5B9BFF` | `#6FD3FF` → `#3A6BFF` | Brand title gradient only |
| `--yellow` | `#FFB020` | `#FFB020` | Launch button, badges, featured tag, current-banner ring (dark text on it) |
| `--ok` / `--warn` / `--bad` | `#43D99A` / `#FFC24D` / `#FF8590` | `#0B7A4E` / `#8A5600` / `#C2364A` | States (text-safe on their theme's cards); dots on dark chips always use `#43D99A` |
| `--switch-off` | `#66749F` | `#7D8AA4` | Switch track when off (≥ 3:1 against the card) |
| `--pill` / `--pill-ink` | `rgba(127,219,255,.12)` / `#B9D2F5` | `#EEF4FF` / `#3A5A9A` | "Powered by" pill |
| `--scroll` / `--scroll-hover` | `rgba(127,219,255,.22)` / `.42` | `rgba(58,107,255,.24)` / `.46` | Scrollbar thumb (accent `--btn` while dragging) |

- **Type**: Nunito 600 (body), 700 (buttons), 800 (labels), 900 (headings). Sizes: page
  title 30, section 17–18, card title 15, body 14, caption 12–13, overline 11 (uppercase,
  tracked +0.14em). Nunito ships with the app (bundled font file; the webview loads nothing
  from the internet).
- **Radius**: pill 999 px for buttons, inputs, chips and tabs; 20 px cards; 28 px large
  banners and dialogs; 14 px nav items; 18 px game banners in the sidebar.
- **Spacing**: 4 px steps; page padding 28 / 36 px; grid gap 18 px; panel padding 20 px.
- **Scrollbars**: themed everywhere in the app: 12 px, transparent track, a pill thumb inset
  3 px, brighter on hover, solid `--btn` while dragging, no arrow buttons. Each theme also sets
  `color-scheme` so native controls (search clear button, date pickers) follow it.
- **Elevation**: one shadow token (`--shadow`), used on hover, menus and open bubbles.
- **Motion**: 200–450 ms, springy overshoot only on the logo and the dashboard bubble; all
  transitions and animations off under `prefers-reduced-motion`.
- **Icons**: 2 px stroke, round caps, 14–22 px, drawn to one style (Lucide-like; if a library
  is adopted it must pass the npm license policy).
- **Accessibility**: real buttons and inputs, 44 px minimum targets, visible focus ring
  (3 px `--accent`), text contrast AA (4.5:1, 3:1 for large text) and 3:1 for controls in
  both themes, state never by color alone (label plus color).
- **Rules from the contrast and overlap review** (three passes, every screen and state in
  both themes, 2026-10-24; checked by measuring each text's contrast against its real
  background and each pair of text boxes for overlap, plus screenshots):
  - Text over images, banners and art always sits on a dark scrim (`rgba(8,12,26,.6–.9)`
    fades or chips) and is white; it never inherits the page's text color.
  - Never dim with `opacity` on anything that contains text (inactive gallery items,
    finished downloads, previews): darken the artwork or change the background instead.
  - Disabled controls keep readable text: `--raised` background, `--muted` text, a dashed
    border for disabled dropdowns.
  - Every button style sets its own text color; the base button rule has the lowest
    priority (`:where()`).
  - Badges on a card share one row (left group: selection box and character; right group:
    game and "Installed") and truncate with an ellipsis instead of stacking.
  - Watermark text on banners stays at 5–7% opacity, small, and away from titles.
  - Titles on banners clamp to one line (sidebar) or two (gallery, cards) with an ellipsis.

## What the data allows (found while designing)

- **NSFW is the common case on GameBanana.** With sensitive mods hidden (the default), the
  "most liked" page kept 3 of 20 mods, and this week's trending lists for both games were
  almost entirely flagged. The store must page further to fill a screen, show the hidden
  count, and the featured gallery must filter and backfill (below).
- **Trending exists**: `apiv11/Game/{id}/TopSubs` returns the top 3 per period (today,
  week, month, 3 / 6 months, year, all time). Featured = its safe-for-work entries in that
  order, backfilled with popular recent mods to 5 (built 2026-10-25: `gmm_core::featured`,
  backfill = the most liked of the newest 50; command `catalog_featured`). On 2026-10-25,
  18 of ZZZ's 21 entries were flagged sensitive.
- **Download counts aren't in listings** (only on mod pages; checked that list endpoints don't
  offer them). Cards show likes and views at once and fill in downloads as each visible mod's
  numbers arrive (cached for an hour); the mockup shows that loading state. GMM asks for just
  the counters (`Mod/{id}?_csvProperties=...`, a fraction of the mod page) on a slow
  background client (`catalog_stats`).
- **RAR is common**: of 15 mod files sampled across both games (2026-10-24/25), 9 were zip,
  4 rar and 2 7z. RAR support landed first in Phase 5 (2026-10-24), so these install like
  any other mod. Details: [09](09-archive-formats.md).
- **Versions come two ways on GameBanana**: as separate files on the mod page (Homewear Belle:
  three zips with descriptions), or as several mod folders inside one archive (Fixed Yae
  Sakura Sword Pack: 8). The first is known from the API; the second only after looking inside
  the archive. GameBanana has a `File/{id}/RawFileList` route and a `_bHasContents` flag on
  files, but the route returns an empty body (200, `Content-Length: 0`), even for files
  flagged `_bHasContents` and with browser-like headers (checked twice, 2026-10-25). So GMM
  fetches the archive into its cache before showing the versions, which also means the
  confirmation fetches single-version files too.
- **Versions often exclude each other**: the 8 sword versions all replace Anby's weapon, so
  only one can be on. Conflict detection between versions decides this, not names.
- **Characters have icons** on GameBanana (subcategory icons); Bangboo don't (initials).
- **Game banner art** (decided 2026-10-25): official key art per game, provided by the
  project owner, bundled under a notice that it isn't covered by GMM's license and goes on
  request ([06](06-licensing-and-provenance.md)). `apps/desktop/src/assets/banners/`,
  1800 × 900 JPEG (about 300–400 KiB each; the source art is 2000 × 1125, and an open bubble
  on a 2× screen needs about this much), cropped 2:1 with the ZZZ logo cut off. Every banner
  size uses the same file with `background-size: cover` around a focal point per game (ZZZ
  `45% 35%`, WuWa `50% 40%`), so the characters stay in frame from the 34 px Downloads chip
  to the open bubble. Text keeps its dark scrim (`.fade`); the watermark is off on art. A
  game without art keeps its styled banner (colors and name).
- **"Last played"** is recorded when GMM's Launch button starts a game (`game_activity`,
  2026-10-25); launches from elsewhere aren't seen.
- **Character for library cards** comes from the mod's categories, stored at install since
  2026-10-25 (root category and subcategory; WuWa's root is "Skins", ZZZ's "Character
  Skins"); earlier and hand-installed mods have none.
- **Description images are often off GameBanana's image host**: across ten mods (2026-10-25)
  they were as often on `filecacheNN.gamebanana.com` / `files.gamebanana.com` or imgur-like
  hosts as on `images.gamebanana.com`. GameBanana's own hosts (all three) are shown; the mod
  page needs a quiet placeholder for the rest (imgur and others, never loaded: R2).
- **Report a Bug** (decided 2026-10-25): a pre-filled GitHub issue opened in the browser, with
  a local log the user reviews and attaches; GMM itself doesn't connect to GitHub, so the
  network allowlist (R2) stays as it is.

## Open design decisions

1. ~~Theme and layout sign-off~~ (done 2026-10-25).
2. ~~Game banner source~~: official key art, bundled with a notice (2026-10-25, above).
3. "Powered by GameBanana" in the lockup is descriptive and stays; it changes if GameBanana
   asks (no outreach is planned, roadmap open question 2).
4. ~~Component library~~: built by hand (`src/ui`, 2026-10-25); dialogs use the native
   `<dialog>` element for focus trapping and Escape.
