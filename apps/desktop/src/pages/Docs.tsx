// SPDX-License-Identifier: MIT OR Apache-2.0
// Documentation: a grid of guide cards, each opening the
// guide in the app. The guides are the Markdown files in docs/guides/, bundled at build time
// (`?raw`), so editing one updates the next build and installed copies fetch nothing.

import { useEffect, useRef, useState } from "react";
import conflicts from "../../../../docs/guides/conflicts.md?raw";
import fixingSetup from "../../../../docs/guides/fixing-setup.md?raw";
import gettingStarted from "../../../../docs/guides/getting-started.md?raw";
import howModsLoad from "../../../../docs/guides/how-mods-load.md?raw";
import installingSafely from "../../../../docs/guides/installing-safely.md?raw";
import privacy from "../../../../docs/guides/privacy.md?raw";
import { Markdown, parseBlocks } from "../markdown";
import { Button, Icon, type IconName } from "../ui";

type Guide = { slug: string; icon: IconName; source: string };

/** In the order docs/guides/README.md lists them. */
const GUIDES: Guide[] = [
  { slug: "getting-started", icon: "play", source: gettingStarted },
  { slug: "how-mods-load", icon: "layers", source: howModsLoad },
  { slug: "fixing-setup", icon: "sliders", source: fixingSetup },
  { slug: "installing-safely", icon: "shield", source: installingSafely },
  { slug: "conflicts", icon: "warning", source: conflicts },
  { slug: "privacy", icon: "eye", source: privacy },
];

/** A guide's `# Title` and its first paragraph (the card's summary). */
function head(source: string): { title: string; summary: string } {
  const blocks = parseBlocks(source);
  const title = blocks.find((b) => b.kind === "heading")?.text ?? "Guide";
  const para = blocks.find((b) => b.kind === "para");
  return { title, summary: para?.kind === "para" ? para.text.replace(/\*\*/g, "") : "" };
}

export function Docs() {
  const [open, setOpen] = useState<string | null>(null);
  const top = useRef<HTMLDivElement>(null);
  const guide = GUIDES.find((g) => g.slug === open);

  // A new guide starts at its top.
  useEffect(() => {
    if (open) top.current?.scrollIntoView({ block: "start" });
  }, [open]);

  if (guide) {
    const { title } = head(guide.source);
    const at = GUIDES.indexOf(guide);
    const next = GUIDES[at + 1];
    return (
      <div ref={top} className="guide-page">
        <button type="button" className="back-link" onClick={() => setOpen(null)}>
          ← Documentation
        </button>
        <article className="card panel guide-article">
          <div className="guide-title">
            <span className="guide-icon" aria-hidden="true">
              <Icon name={guide.icon} size={22} stroke={2.4} />
            </span>
            <h1>{title}</h1>
          </div>
          <Markdown source={guide.source} onGuide={setOpen} skipTitle />
        </article>
        {next && (
          <div className="guide-next">
            <Button onClick={() => setOpen(next.slug)}>Next: {head(next.source).title} →</Button>
          </div>
        )}
      </div>
    );
  }

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Documentation</h1>
          <p>How GMM works, and what to do when something doesn't. Everything here is on this computer.</p>
        </div>
      </div>
      <div className="guide-grid">
        {GUIDES.map((g) => {
          const { title, summary } = head(g.source);
          return (
            <button key={g.slug} type="button" className="card guide-card" onClick={() => setOpen(g.slug)}>
              <span className="guide-icon" aria-hidden="true">
                <Icon name={g.icon} size={22} stroke={2.4} />
              </span>
              <b>{title}</b>
              <span className="sub">{summary}</span>
            </button>
          );
        })}
      </div>
    </>
  );
}
