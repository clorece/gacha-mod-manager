# GameBanana API: terms, limits, and how GMM stays polite

## Do we need permission?

**No.** GMM reads public mod data and downloads public files, which needs no key or account.
Checked on 2026-10-25:

- GameBanana's **Terms of Service** and **Site Rules** say nothing about the API, third-party
  clients or request rates.
- **apiv11**, which GMM uses, is the API GameBanana's own website calls. Public data needs no
  key or login.
- A GameBanana site moderator answered a question about building a mod loader on the API by
  pointing to its field list, with no objection. Many mod managers for other games work the
  same way.
- No published rate limit or API policy was found.

The one thing that would need GameBanana: **1-click "Install with GMM" buttons** on mod pages.
GameBanana adds those integrations itself, so GMM has none for now.

**No outreach.** GMM doesn't contact GameBanana. Every request carries an honest User-Agent
that links this repo, and the project will comply with any request GameBanana makes. The
README credits GameBanana and the mod creators.

## How GMM talks to GameBanana

- Plain HTTPS `GET`s to public endpoints. No key, no login, no cookies.
- User-Agent: `GMM/<version> (+https://github.com/clorece/gacha-mod-manager)`. Never disguised
  as a browser.
- Downloads run one at a time, can resume, and are cached, so each file is fetched once.

## Keeping the load low

| Measure | Detail |
|---|---|
| Request budget | About 2 requests/s per install for the API (background reads share it); images have their own 8/s |
| Response cache | Lists 5 min, mod pages 1 h, categories 24 h; the startup update check accepts answers up to 6 h old |
| No duplicates | The same page or file requested twice at once is fetched once |
| Counters only | Card download counts ask for just the counters, only for cards on screen |
| No polling | GMM only asks when the user does something, plus the startup check |

## When GameBanana says "slow down"

A **429**, a **503 with `Retry-After`** or a **Cloudflare challenge** pauses every GameBanana
request in the app: for the time GameBanana asked, or 30 s doubling each time (up to 15
minutes). GMM never retries in a loop and never tries to pass a challenge. Meanwhile:

- Pages show **saved answers** from the last 7 days instead of an error, with a notice at the
  top of the app.
- A queued install shows **Waiting** and carries on by itself, keeping what it downloaded.
- Server errors and dropped connections are retried 3 times (1, 2, 4 s); downloads resume.

Limits are usually counted per IP address, so one user's limit doesn't affect another's. People
behind a shared address (mobile networks, VPNs, offices) can be limited by other people's
traffic; GMM handles that the same way.

## Limits users will notice

| Limit | What happens |
|---|---|
| apiv11 has no published contract | Fields can change; GMM decodes leniently and shows plain errors instead of crashing |
| Many mods are marked sensitive | With them hidden (the default), the Store loads more pages and shows how many were hidden |
| Archive contents aren't listed | Versions inside a file are only known after downloading it |
| Files can be removed or still awaiting a virus scan | The install is refused with the reason |
| Description images on other sites (imgur and others) | Not shown; only GameBanana's own hosts |
| No login | No subscriptions, likes or private mods |
