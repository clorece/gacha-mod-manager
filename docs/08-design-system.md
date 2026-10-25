# 08 — Design system

Phase 4's output: how GMM looks and is organized. The interactive mockup lives on a design
canvas in your claude.ai account (private): every screen in dark and light, built with real
GameBanana names and stats (mod preview images are placeholders, since GMM never re-hosts
them). This doc is the written source of truth the app is built from. Screen-level detail for
the catalogue and mod page fields stays in [07](07-frontend-design.md).

**Status (2026-10-25): signed off, in the app.** The layout and theme are signed off; the
second round added the startup check, the install confirmation (versions and files), mod
settings with versions, and the sticky search and filter bar. Every state was checked in both
themes (screenshots plus an automated contrast and overlap check over 62 states).

**In the app** (Phase 4): the tokens below, base styles, scrollbars and the `:where()`
button base are in `apps/desktop/src/styles.css`; Nunito is bundled; the primitives are in
`apps/desktop/src/ui` and shown in every state on the dev-only **Style guide** page (sidebar,
Development, dev builds only). Built screens: the sidebar with Select Game, the Dashboard
(bubbles, Launch, recently installed, setup), the Mods Library (game cards, search, enable /
disable all, switches, conflict notice; category, character and select mode wait for stored
categories) and a first Mod settings panel (switch, details, Uninstall). The rest follow in
Phase 5. Two details differ from the canvas on purpose: "Last played" isn't shown until GMM
records it (Phase 5), and mod cards use placeholder art until previews are wired.

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
  multi-select · Sort · (right) result count with "N hidden as sensitive". It is **sticky**:
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
- Header: title, one line ("Switch one off and XXMI skips it…"), then the game banner cards
  (All games / each game, with "N mods · M on").
- Toolbar: search, Category, Characters on the left; **Enable all**, **Disable all** and
  **Select** on the right. Sticky like the store's, together with the selection bar. Select
  mode adds a selection bar under the toolbar ("N selected", a hint, Enable selected /
  Disable selected / Done) and a checkbox on each card.
- Cards: preview, character badge (mods installed through GMM), a round settings button
  (opens Mod settings), name, "Installed 24 Oct 2026" or "Added by hand", size, On/Off label
  and a switch. A mod with versions shows "2 of 8 versions" on its preview; its switch turns
  the mod's current version on or off.
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
- **Uninstall** (red outline) with a line saying what it does: all versions to the Recycle
  Bin, archive kept in the cache; for hand-installed mods, the itemized confirmation first.

### Settings
Cards: Appearance (Dark / Light / Match Windows), Mods Store (show sensitive mods, blur their
previews), XXMI Launcher (folder, found automatically or chosen), Games (how each was found,
add a folder), Storage (download cache, image cache, each with Clear), About (version,
license, disclaimer, licenses).

### Report a Bug (in development)
The page exists in the sidebar with a `WIP` tag. It shows the mark, an "In development"
chip, one paragraph on what it will do (describe the problem, optionally attach an error or
crash log, see everything before it's sent), and a greyed preview of the form. Not in the
first beta.

### Documentation
A grid of guide cards: Getting started, How mods load, Fixing setup problems, Installing
safely, Conflicts, Privacy. Content comes from these docs, rewritten for players.

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
  order, backfilled with popular recent mods to 5. Needs a small `gmm-catalog` addition.
- **Download counts aren't in listings** (only on mod pages; checked that list endpoints don't
  offer them). Cards show likes and views at once and fill in downloads as each visible mod's
  page loads (cached for an hour); the mockup shows that loading state.
- **RAR is common**: of 15 mod files sampled across both games (2026-10-24/25), 9 were zip,
  4 rar and 2 7z. RAR support landed first in Phase 5 (2026-10-24), so these install like
  any other mod. Details: [09](09-archive-formats.md).
- **Versions come two ways on GameBanana**: as separate files on the mod page (Homewear Belle:
  three zips with descriptions), or as several mod folders inside one archive (Fixed Yae
  Sakura Sword Pack: 8). The first is known from the API; the second only after looking inside
  the archive. GameBanana has a `File/{id}/RawFileList` route and a `_bHasContents` flag on
  files, but the route returned nothing in tests (2026-10-25); worth another look, since it
  could list versions without downloading. Until then GMM fetches the archive into its cache
  before showing the versions.
- **Versions often exclude each other**: the 8 sword versions all replace Anby's weapon, so
  only one can be on. Conflict detection between versions decides this, not names.
- **Characters have icons** on GameBanana (subcategory icons); Bangboo don't (initials).
- **Game banner art** has no source yet: GameBanana's game record has no banner, and game key
  art can't be bundled (trademark, [06](06-licensing-and-provenance.md)). Until decided, the
  banners are the game's colors and name. Options: a banner you provide per game under a
  clear license, a top mod's preview image loaded live, or plain styled banners.
- **"Last played"** isn't tracked; GMM can record it when its Launch button starts a game
  (a new settings row per game).
- **Character for library cards** needs the mod's category stored at install (not stored
  today); hand-installed mods have none.
- **Report a Bug** will need a destination host (GitHub issues or similar), which the network
  allowlist (R2) doesn't allow today; that's part of building it, after the beta.

## Open design decisions

1. ~~Theme and layout sign-off~~ (done 2026-10-25).
2. Game banner source (above).
3. "Powered by GameBanana" in the lockup is descriptive, but ask GameBanana before release
   along with the API questions (roadmap open question 2).
4. ~~Component library~~: built by hand (`src/ui`, 2026-10-25); dialogs use the native
   `<dialog>` element for focus trapping and Escape.
