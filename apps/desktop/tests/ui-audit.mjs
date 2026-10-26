// SPDX-License-Identifier: MIT OR Apache-2.0
// The contrast and overlap audit (docs/05-roadmap.md, Phase 5 tests; first run by hand on the
// design canvas in Phase 4). It serves the frontend with Vite, opens it in Chromium through
// Playwright with GMM's commands answered by the fake bridge (src/test/fakeBridge.ts, the same
// fixtures as the Vitest tests), walks the app's screens in both themes and checks:
//
// - contrast: every piece of text against the background it sits on, composited through
//   semi-transparent layers: 4.5:1, or 3:1 for large text (WCAG 2.2 AA). Text on artwork
//   (previews, banners) is skipped: its readability comes from scrims, not one color.
//   Disabled controls are exempt, as in WCAG.
// - overlap: no two pieces of text (from different elements) cover each other.
// - overflow: the page never scrolls sideways.
//
// Run: `pnpm ui-audit` in apps/desktop. CI installs Chromium (`playwright install
// chromium`); locally GMM_AUDIT_CHANNEL=msedge (or chrome) uses an installed browser instead.
// Exits 1 with a list of findings.

import { chromium } from "playwright";
import { createServer } from "vite";

const PORT = 1431;
const VIEWPORT = { width: 1440, height: 900 };

/** Installs a fake `__TAURI_INTERNALS__` that answers from the fake bridge (served by Vite). */
const fakeTauri = (prefs) => {
  let bridge;
  const load = async () => {
    if (!bridge) {
      bridge = await import("/src/test/fakeBridge.ts");
      bridge.resetBridge(prefs);
    }
    return bridge;
  };
  let next = 1;
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => (await load()).fakeInvoke(cmd, args),
    transformCallback: (callback) => {
      const id = next++;
      window[`_${id}`] = callback;
      return id;
    },
    unregisterCallback: (id) => delete window[`_${id}`],
    convertFileSrc: (path) => path,
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
  };
  try {
    localStorage.setItem("gmm.theme", prefs.theme);
  } catch {
    // The theme comes from the preferences anyway.
  }
};

/** Runs in the page: the findings for the current screen. */
function inspect() {
  const findings = [];
  const parse = (c) => {
    const m = c.match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const [r, g, b, a = "1"] = m[1].split(/[ ,/]+/).filter(Boolean);
    return { r: +r, g: +g, b: +b, a: +a };
  };
  const over = (top, bottom) => {
    const a = top.a + bottom.a * (1 - top.a);
    if (a === 0) return { r: 0, g: 0, b: 0, a: 0 };
    const mix = (k) => (top[k] * top.a + bottom[k] * bottom.a * (1 - top.a)) / a;
    return { r: mix("r"), g: mix("g"), b: mix("b"), a };
  };
  const lum = ({ r, g, b }) => {
    const ch = (v) => {
      const s = v / 255;
      return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b);
  };
  const ratio = (x, y) => {
    const [a, b] = [lum(x), lum(y)].sort((p, q) => q - p);
    return (a + 0.05) / (b + 0.05);
  };
  const name = (el) => {
    const cls = typeof el.className === "string" && el.className ? `.${el.className.trim().split(/\s+/).join(".")}` : "";
    return `${el.tagName.toLowerCase()}${cls}`;
  };
  const visible = (el) => {
    for (let e = el; e && e !== document.documentElement; e = e.parentElement) {
      const s = getComputedStyle(e);
      if (s.display === "none" || s.visibility === "hidden" || Number(s.opacity) === 0) return false;
      if (e.hidden) return false;
    }
    return true;
  };
  const isArt = (e) =>
    e.matches(".preview, .banner, .art, .thumb, img, video, canvas") || getComputedStyle(e).backgroundImage !== "none";
  const arts = [...document.querySelectorAll("body *")].filter((e) => isArt(e) && visible(e));
  /** The color behind `el`, or null when artwork is behind it. */
  const background = (el) => {
    const layers = [];
    let base = document.body;
    for (let e = el; e; e = e.parentElement) {
      if (isArt(e)) return null;
      const c = parse(getComputedStyle(e).backgroundColor);
      if (c && c.a > 0) {
        layers.push(c);
        if (c.a >= 1) {
          base = e;
          break;
        }
      }
    }
    // Artwork painted between that background and the text (a sibling image under a title,
    // like the featured banner's) means the text sits on the picture.
    const box = el.getBoundingClientRect();
    const x = box.left + box.width / 2;
    const y = box.top + box.height / 2;
    for (const art of arts) {
      if (art === base || !base.contains(art) || art.contains(el)) continue;
      const r = art.getBoundingClientRect();
      if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return null;
    }
    let bg = { r: 255, g: 255, b: 255, a: 1 };
    for (const layer of layers.reverse()) bg = over(layer, bg);
    return bg;
  };
  /** Text boxes cut to what scroll boxes and line clamps (overflow other than visible) show. */
  const clipped = (el, rects) => {
    let out = rects.map((r) => ({ left: r.left, top: r.top, right: r.right, bottom: r.bottom }));
    for (let e = el; e && e !== document.body; e = e.parentElement) {
      const s = getComputedStyle(e);
      if (s.overflowX === "visible" && s.overflowY === "visible") continue;
      const c = e.getBoundingClientRect();
      out = out.map((r) => ({
        left: Math.max(r.left, c.left),
        top: Math.max(r.top, c.top),
        right: Math.min(r.right, c.right),
        bottom: Math.min(r.bottom, c.bottom),
      }));
    }
    return out
      .filter((r) => r.right - r.left > 1 && r.bottom - r.top > 1)
      .map((r) => ({ ...r, width: r.right - r.left, height: r.bottom - r.top }));
  };

  // With a modal open, only its content is in front.
  const scope = document.querySelector("dialog[open]") ?? document.body;
  const owners = [];
  const walker = document.createTreeWalker(scope, NodeFilter.SHOW_TEXT);
  const seen = new Set();
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const el = node.parentElement;
    if (!el || !node.textContent.trim() || seen.has(el)) continue;
    if (el.closest("script, style, .sr-only, [aria-hidden='true'], svg")) continue;
    if (!visible(el)) continue;
    seen.add(el);
    const range = document.createRange();
    range.selectNodeContents(node);
    const rects = clipped(el, [...range.getClientRects()]);
    if (rects.length === 0) continue;
    owners.push({ el, rects });

    if (el.closest("[disabled], [aria-disabled='true'], .is-disabled")) continue;
    const s = getComputedStyle(el);
    const bg = background(el);
    if (!bg) continue;
    let fg = parse(s.color);
    if (!fg) continue;
    let opacity = 1;
    for (let e = el; e; e = e.parentElement) opacity *= Number(getComputedStyle(e).opacity);
    fg = over({ ...fg, a: fg.a * opacity }, bg);
    const size = Number.parseFloat(s.fontSize);
    const large = size >= 24 || (size >= 18.66 && Number(s.fontWeight) >= 700);
    const need = large ? 3 : 4.5;
    const got = ratio(fg, bg);
    if (got + 0.005 < need) {
      findings.push({
        kind: "contrast",
        where: name(el),
        text: node.textContent.trim().slice(0, 50),
        detail: `${got.toFixed(2)}:1 (needs ${need}:1)`,
      });
    }
  }

  for (let i = 0; i < owners.length; i++) {
    for (let j = i + 1; j < owners.length; j++) {
      const a = owners[i];
      const b = owners[j];
      if (a.el.contains(b.el) || b.el.contains(a.el)) continue;
      for (const p of a.rects) {
        for (const q of b.rects) {
          const w = Math.min(p.right, q.right) - Math.max(p.left, q.left);
          const h = Math.min(p.bottom, q.bottom) - Math.max(p.top, q.top);
          if (w > 2 && h > 2 && w * h > 0.2 * Math.min(p.width * p.height, q.width * q.height)) {
            findings.push({
              kind: "overlap",
              where: `${name(a.el)} / ${name(b.el)}`,
              text: `${a.el.textContent.trim().slice(0, 30)} / ${b.el.textContent.trim().slice(0, 30)}`,
              detail: `${Math.round(w)}×${Math.round(h)} px`,
            });
          }
        }
      }
    }
  }

  const main = document.querySelector("main") ?? document.scrollingElement;
  for (const box of [document.scrollingElement, main]) {
    if (box && box.scrollWidth > box.clientWidth + 1) {
      findings.push({ kind: "overflow", where: name(box), text: "", detail: `${box.scrollWidth} > ${box.clientWidth}` });
    }
  }
  return findings;
}

const nav = (label) => async (page) => {
  await page.getByRole("navigation", { name: "Pages" }).getByRole("button", { name: label }).click();
};
/** Clicks the first visible match (pages stay mounted behind the mod page). Strings match the
 *  whole accessible name; a RegExp matches part of it. */
const click = (role, name) => async (page) => {
  await page
    .getByRole(role, { name, exact: typeof name === "string" })
    .filter({ visible: true })
    .first()
    .click();
};

/** Each state: a name and the steps from a freshly opened app. */
const STATES = [
  ["Dashboard", []],
  ["Dashboard, game open", [click("button", /^Zenless Zone Zero: show/)]],
  ["Setup guide", [click("button", "Open setup guide")]],
  ["Downloads", [nav(/^Downloads/)]],
  ["Needs action review", [nav(/^Downloads/), click("button", "Review")]],
  ["Settings", [nav("Settings")]],
  ["Storage, all games", [nav("Settings"), click("button", "Show breakdown")]],
  ["Storage, one game", [nav("Settings"), click("button", "Show breakdown"), click("button", "Zenless Zone Zero")]],
  ["Third-party notices", [nav("Settings"), click("button", "Third-party notices")]],
  ["Mods Library", [nav("Mods Library")]],
  ["Library, sensitive shown", [nav("Mods Library"), click("switch", "Show sensitive")]],
  ["Library, select mode", [nav("Mods Library"), click("button", "Select")]],
  ["Mod settings", [nav("Mods Library"), click("button", "Mod settings for Sample Sword Pack")]],
  ["Conflict prompt", [nav("Mods Library"), click("switch", "Show sensitive"), click("switch", "Swimsuit Recolor on or off")]],
  ["Profiles", [nav("Mods Library"), click("button", /^Zenless Zone Zero \d/), click("button", "Profiles")]],
  ["Mods Store", [nav("Mods Store")]],
  ["Mod page", [nav("Mods Store"), click("button", "Open Sample Sword Pack")]],
  ["Install confirmation, conflict", [nav("Mods Store"), click("button", "Open Street Jacket"), click("button", "Download")]],
  ["Install confirmation, variants", [nav("Mods Store"), click("button", "Add versions of Sample Sword Pack")]],
  ["Documentation", [nav("Documentation")]],
  ["Guide", [nav("Documentation"), click("button", /^Installing safely/)]],
  ["Report a Bug", [nav(/^Report a Bug/)]],
];

async function open(browser, prefs) {
  const page = await browser.newPage({ viewport: VIEWPORT, reducedMotion: "reduce" });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.addInitScript(fakeTauri, prefs);
  await page.goto(`http://localhost:${PORT}/`);
  if (prefs.onboarded) {
    await page.getByRole("button", { name: /Open GMM|Skip for now/ }).click();
    await page.getByRole("heading", { name: "Dashboard", level: 1 }).waitFor();
  }
  await page.evaluate(() => document.fonts.ready);
  return { page, errors };
}

async function settle(page) {
  // Let data arrive and transitions finish (reduced motion keeps them short).
  await page.waitForLoadState("networkidle");
  await page.waitForTimeout(300);
}

const server = await createServer({ server: { port: PORT, strictPort: true }, logLevel: "error" });
await server.listen();
const browser = await chromium.launch(process.env.GMM_AUDIT_CHANNEL ? { channel: process.env.GMM_AUDIT_CHANNEL } : {});
const report = [];
let failed = false;
try {
  // Self-check: with a contrast, an overlap and an overflow planted on the Dashboard, each
  // must be reported, so a broken check can't pass quietly.
  {
    const { page } = await open(browser, { theme: "dark", onboarded: true });
    await page.addStyleTag({
      content: `.page-head p { color: #1c2440 !important; }
        .page-head h1 { margin-bottom: -26px !important; }
        main::after { content: ""; display: block; width: 6000px; height: 1px; }`,
    });
    await settle(page);
    const kinds = new Set((await page.evaluate(inspect)).map((f) => f.kind));
    await page.close();
    for (const kind of ["contrast", "overlap", "overflow"]) {
      if (!kinds.has(kind)) throw new Error(`self-check: a planted ${kind} problem wasn't reported`);
    }
    console.log("ok   self-check (planted contrast, overlap and overflow were found)");
  }
  for (const theme of ["dark", "light"]) {
    const states = [...STATES.map(([n, steps]) => [n, steps, true]), ["First run", [], false]];
    for (const [name, steps, onboarded] of states) {
      const { page, errors } = await open(browser, { theme, onboarded });
      try {
        for (const step of steps) {
          await step(page);
          await settle(page);
        }
        await settle(page);
        const findings = await page.evaluate(inspect);
        for (const e of errors) findings.push({ kind: "script error", where: "", text: "", detail: e });
        report.push({ theme, name, findings });
        if (findings.length > 0) failed = true;
        console.log(`${findings.length === 0 ? "ok  " : "FAIL"} ${theme.padEnd(5)} ${name}`);
        for (const f of findings) console.log(`       ${f.kind}: ${f.where} "${f.text}" ${f.detail}`);
      } catch (e) {
        failed = true;
        console.log(`FAIL ${theme.padEnd(5)} ${name}: couldn't reach the state: ${e.message.split("\n")[0]}`);
      } finally {
        await page.close();
      }
    }
  }
} finally {
  await browser.close();
  await server.close();
}
const total = report.reduce((n, r) => n + r.findings.length, 0);
console.log(`\n${report.length} states checked, ${total} finding(s).`);
process.exit(failed ? 1 : 0);
