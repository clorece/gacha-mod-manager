# 10 — GameBanana API: terms, limits, and how GMM stays polite

How GMM uses GameBanana, what GameBanana allows, what can go wrong for a user in daily use,
and what GMM does about it. Checked 2026-10-25. Endpoints and field shapes are in
[02](02-architecture.md#catalog-flow-gamebanana-apiv11).

## Do we need permission?

**No permission or key is needed for what GMM does** (reading public mod data, downloading
public files). What was checked, 2026-10-25:

- **Terms of Service** (gamebanana.com/wikis/334, linked from the site footer, last changed
  2023-05-25): about content ownership, liability, paid services, age and jurisdiction.
  Nothing about the API, automated access, bots, scraping, third-party clients or request
  rates. It can change without notice, so check it again before each release.
- **Site Rules** (gamebanana.com/rules): about submissions and conduct (spam, harassment,
  AI content, NSFW). Nothing about API use.
- **apiv11**, which GMM uses, is the API GameBanana's own website calls. It takes no key or
  login for public data. The older documented API (api.gamebanana.com) has app
  registration (App ID and password) for authenticated actions and "apps" shown on member
  profiles; GMM uses neither.
- **Community answers**: a GameBanana **Site Moderator** answered a 2026 question asking
  exactly this ("create a modloader... is this allowed by ToS?", question 101991) by pointing
  to the API's field list, with no objection or conditions. Many mod managers for other games
  already read the API and download files the same way.
- **Not found**: any published rate limit, quota or API policy. The footer's "gbAPI" link (a
  Postman workspace) is dead.

What *does* need GameBanana:

- **1-click install buttons** on mod pages ("Install with GMM"). GameBanana lists a mod's
  manager integrations in `_aModManagerIntegrations`; it is empty for every ZZZ and WuWa mod
  checked (729726, 553203, 688491, 731970). A staff post from 2018 describes them integrating
  mod managers that start from a custom URL scheme. So GMM can only get those buttons if
  GameBanana adds it; deep links (deferred from Phase 6 to "Later" on 2026-10-26) wait on that.
- **Using their name** in the lockup ("Powered by GameBanana") is descriptive; it stays
  unless they ask otherwise.

**Decision (2026-10-25): no outreach.** GMM is an open-source interface for easier downloads
and management, and needs no permission, so the project lets GameBanana find it rather than
contacting them. They can: every request carries GMM's User-Agent, which links the public
repo. If GameBanana asks for contact, changes or removal, the project complies. The README's
Credits section names GameBanana, the mod creators, XXMI and the other sources, and invites
any owner of credited work to ask for credit changes or removal through GitHub. The cost of
not asking: no 1-click buttons unless GameBanana adds GMM on its own.

## How GMM talks to GameBanana

- **No API key, no account, no login, no cookies.** Plain HTTPS `GET`s to public endpoints
  through `gmm-net`, which refuses every host but `gamebanana.com`, `files.gamebanana.com`,
  `filecacheNN.gamebanana.com` and `images.gamebanana.com` (R2). Nothing secret is stored or
  sent, so there is no key to leak.
- **An honest User-Agent**: `GMM/<version> (+https://github.com/clorece/gacha-mod-manager)`.
  If GMM ever causes trouble, GameBanana can see whose it is and get in touch instead of
  guessing. Never disguise it as a browser.
- **Three clients, one state**: the API client, a background client (card counters) and the
  image client share one connection pool and one slow-down state (`Client::sibling`), since
  they all reach GameBanana from the same IP address.
- **Downloads**: one at a time (the install queue), resumable (range requests), each file
  fetched once and kept in the download cache for reinstalls and other versions. A second
  fetch of the same file waits for the first (no duplicate downloads).

## Who gets limited: per IP, mostly

Limits like this are almost always counted per IP address (GameBanana sits behind Cloudflare,
whose rate limits and bot checks key on the visitor's IP). So the user's guess is right in the
main: a thousand GMM users downloading at once are a thousand separate budgets, and one user's
limit doesn't touch another's. Three caveats:

1. **Shared addresses share a budget.** Carrier-grade NAT (common on mobile and some ISPs),
   university or office networks and VPN exit nodes put many people behind one IP. A GMM user
   there can be limited because of other people's traffic, and nothing on GMM's side prevents
   it. GMM handles it like any other limit (below).
2. **The User-Agent is shared by every GMM install.** If GameBanana ever judged GMM's total
   traffic abusive, it could block that User-Agent and stop every user at once. This is the
   real "many users" risk, and why per-install load must stay low and predictable (prevention,
   below), and why the User-Agent stays honest and links the repo, so they can reach the
   project instead of blocking it.
3. **Bot checks aren't about rate.** Cloudflare can ask any non-browser client for a browser
   check (a challenge page) during an attack or a configuration change. GMM can't pass one and
   must not try to; it backs off and serves saved data.

## What can go wrong in daily use, and what GMM does

Mitigations come first (they work whatever the cause), then prevention (making it less
likely), then the comparison.

### Mitigation: when GameBanana says "slow down"

| Situation | What GMM does |
|---|---|
| **HTTP 429**, a **503 with `Retry-After`**, or a **Cloudflare challenge** (`cf-mitigated: challenge`) | Counted as a strike. Every GameBanana request in the app pauses (API, background, images, downloads): the server's `Retry-After` if it sent one, else 30 s doubling with each strike in a row (5 s to 15 min). Never retried in a loop. A success clears the strikes (`gmm-net/src/cooldown.rs`) |
| A request during the pause | Fails at once with how long is left, without contacting GameBanana; a pause under 3 s is waited out instead |
| Pages while paused, offline or on 5xx | **Saved answers stand in**: any response saved in the last 7 days is shown instead of an error (stale-if-error, `GameBanana::fetch`). A 404 or a bad request is never answered from saved data |
| The user | A notice at the top of the app: "GameBanana asked GMM to slow down... tries again in N s", or "Showing saved GameBanana data" (`network_status`, a local check every 5 s) |
| An install in the queue | Shows "Waiting · GameBanana asked GMM to slow down · carries on at 14:32" and continues by itself, keeping the part already downloaded; Pause and Cancel still work. Fails only after four waits in a row |
| The startup check | Steps that can't reach GameBanana are shown as failed; the app opens with saved data |
| 5xx without `Retry-After` (an outage) | Retried 3 times with 1, 2, 4 s backoff, then saved data or a plain error |

### Prevention: keeping GMM's load low

| Measure | Where |
|---|---|
| One API budget of 2 requests/s (bursts of 5) per install. Background reads (card download counts) spend the same budget, at most 1/s of it, so they never add to the API load | `NetConfig`, `Client::sibling(.., share_budget: true)` |
| Images on their own budget (8/s, bursts of 16): they come from GameBanana's image host and CDN, not the API, and are kept on disk (512 MiB) so each is fetched once | `ImageCache` |
| Response cache in SQLite: lists 5 min, mod pages and updates 1 h, categories 24 h, file lists 2 min before a download | `gmm-db` `http_cache` |
| The same page asked twice at once (two parts of the UI, React's double effects in dev) is fetched once | `GameBanana::fetch` per-URL lock |
| The same file downloaded twice at once is downloaded once | `fetch_archive` per-file lock |
| The startup update check accepts a file list up to 6 h old, so starting GMM several times a day doesn't ask about every installed mod each time | `check_updates`, `mod_files_within` |
| Only the counters are asked for card download counts (`_csvProperties`), at most 60 cards per call, never the whole mod page | `catalog_stats` |
| Downloads one at a time; nothing downloads twice (cache) | install queue, download cache |
| The Store pages on scroll (30 cards a page) and reads one list per request; ticking several characters (at most 6) or "All games" costs one request per list per page, and its search uses the list's name filter instead of a second endpoint. Download counts are asked only for cards on screen, once each | `gmm_core::browse`, `catalog_stats`, `pages/Store.tsx` |
| No polling: GMM asks GameBanana only when the user does something, plus the startup check | by design |

### Comparison: why mitigation leads

- A limit can come from other people on a shared IP, a Cloudflare rule change or an attack,
  none of which any client-side budget can prevent. Mitigation (backing off, saved data, the
  queue waiting) is the only thing that helps then, and it also covers GMM's own mistakes.
- Prevention by budget is a guess: GameBanana publishes no limit. A community project quotes
  60 requests a minute, unconfirmed. GMM's 2/s budget is only reached in bursts (opening the
  store, scrolling quickly); ordinary use stays far below it, and the heavy cases are bounded
  (60 cards per counter request, the 6 h update-check cache). Lowering the budget further would
  make every screen slower for everyone to guard against a limit nobody has hit; the cooldown
  catches it if they do.
- Prevention that would be stronger but isn't worth it: a shared GMM proxy or mirror (a
  server to run, a privacy and trust cost, against decision 4 "no backend"), or pre-fetching
  less (already minimal). Probing GameBanana to find the real limit would risk a block for the
  person doing it, and isn't done.
- If a real limit is ever observed, the budget in `NetConfig::production` is one number to
  change, and the cooldown already stops the damage.

## Other API limits users will meet

| Limit | Effect | Handling |
|---|---|---|
| No published API contract; apiv11 is the website's own API | Field names or types can change without notice | Lenient decoding (missing or odd fields fall back to defaults); ignored `live_*` tests to spot drift; plain errors instead of crashes |
| Mostly sensitive content | With sensitive mods hidden (default), a page of 20 can keep 3 | The store asks for more pages and shows "N hidden"; the featured gallery filters and backfills |
| Page sizes | Lists at most 50 per page; search fixed at 15 | The Store asks for 30 a page and pages on scroll; its search is the list's name filter (any page size, with category and sort), so the fixed-15 search endpoint is only used by `gmm catalog search` |
| Download counts aren't in listings | Cards fill them in as they arrive | `catalog_stats` (counters only, background budget) |
| Archive contents can't be listed (`RawFileList` is empty) | Versions are only known after fetching | The confirmation fetches into the cache first; the install reuses it |
| Files can be trashed, withheld, made obsolete or still awaiting a virus scan | Install refused with the reason | Checked at every install (`prepare`) |
| MD5 is often missing | Weaker check of the download | Exact size from GameBanana, then GMM's own SHA-256 and scan |
| Description images on other hosts | Not shown (imgur and others never: R2) | GameBanana's own image and file hosts are shown |
| No login | No subscriptions, likes, private or unlisted mods | Out of scope; nothing user-specific is ever sent |
| Mirrors can be slow | Long downloads | Resumable downloads, pause and resume, one at a time |
| Outages and challenges | Pages can't load fresh | Saved data and the notice (above) |
