// SPDX-License-Identifier: MIT OR Apache-2.0
// GameBanana HTML (mod descriptions, licenses, update notes) shown safely (docs/security.md,
// T3). Rust already replaced every address: images are `gmmimg` URLs, links are
// `#gmm-link:<key>` (src-tauri/src/html.rs). Here DOMPurify sanitizes with a strict
// allowlist (no styles, forms, frames, scripts or event handlers), anything that still names
// an outside address is removed, and a click on a link asks Rust to open it, which shows the
// real address in a native dialog first. The CSP stops whatever might slip past both.

import DOMPurify from "dompurify";
import { type MouseEvent, useMemo } from "react";
import { api, asCmdError } from "../api";
import { useToast } from "../ui";

const LINK_PREFIX = "#gmm-link:";
const IMAGE = /^(?:http:\/\/gmmimg\.localhost|gmmimg:\/\/localhost)\/[0-9a-f]{64}$/;
const LINK = /^#gmm-link:[0-9a-f]{32}$/;

// Its own instance, so the hooks below apply to GameBanana HTML only.
const purify = DOMPurify(window);
purify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    const href = node.getAttribute("href");
    if (href === null || !LINK.test(href)) node.removeAttribute("href");
  } else if (node.tagName === "IMG") {
    const src = node.getAttribute("src");
    if (src === null || !IMAGE.test(src)) {
      node.remove();
      return;
    }
    node.setAttribute("loading", "lazy");
    node.setAttribute("decoding", "async");
  }
});

const CONFIG = {
  ALLOWED_TAGS: (
    "p br hr b strong i em u s strike del sub sup small h1 h2 h3 h4 h5 h6 ul ol li blockquote " +
    "code pre span div a img table thead tbody tr th td"
  ).split(" "),
  ALLOWED_ATTR: ["href", "src", "alt", "title", "width", "height", "colspan", "rowspan"],
  // DOMPurify judges every other attribute's value as an address; these hold numbers, so
  // without this they were always dropped (found by the XSS corpus test).
  ADD_URI_SAFE_ATTR: ["width", "height", "colspan", "rowspan"],
  ALLOW_DATA_ATTR: false,
  ALLOW_ARIA_ATTR: false,
  // Only the two shapes Rust produces; the hook above checks them exactly.
  ALLOWED_URI_REGEXP: /^(?:#gmm-link:|http:\/\/gmmimg\.localhost\/|gmmimg:\/\/localhost\/)/,
};

/** Sanitized HTML for display. Exported for tests. */
export function sanitize(html: string): string {
  return purify.sanitize(html, CONFIG);
}

export function RichText({ html, className }: { html: string; className?: string }) {
  const toast = useToast();
  const clean = useMemo(() => sanitize(html), [html]);

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    const link = (e.target as Element).closest("a");
    if (!link) return;
    // The webview never navigates; a known link opens through Rust after a confirmation.
    e.preventDefault();
    const href = link.getAttribute("href") ?? "";
    if (!href.startsWith(LINK_PREFIX)) return;
    api.openLink(href.slice(LINK_PREFIX.length)).catch((err) => toast("bad", asCmdError(err).message));
  };

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents lint/a11y/noStaticElementInteractions: delegates clicks on the links inside, which are focusable themselves
    <div
      className={className ? `rich-text ${className}` : "rich-text"}
      onClick={onClick}
      // biome-ignore lint/security/noDangerouslySetInnerHtml: sanitized by DOMPurify above (T3), addresses already rewritten in Rust
      dangerouslySetInnerHTML={{ __html: clean }}
    />
  );
}
