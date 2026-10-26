// SPDX-License-Identifier: MIT OR Apache-2.0
// A small Markdown renderer for text bundled into the app at build time (the Documentation
// guides, the About section's credits and notices). It builds React elements, never HTML, so
// nothing in the text can become markup. It knows what these files use: headings,
// paragraphs, lists, fenced code, bold, italics, inline code and links. Links to another
// guide (`other-guide.md`) open it in the app; every other address is shown as plain text,
// since the webview never opens an outside address on its own (R5).

import { Fragment, type ReactNode } from "react";

type Block =
  | { kind: "heading"; level: number; text: string }
  | { kind: "para"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "code"; text: string };

export function parseBlocks(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.trim() === "" || /^<!--.*-->$/.test(line.trim())) {
      i += 1;
      continue;
    }
    if (line.startsWith("```")) {
      const body: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i].startsWith("```")) body.push(lines[i++]);
      blocks.push({ kind: "code", text: body.join("\n") });
      i += 1;
      continue;
    }
    const heading = /^(#{1,4})\s+(.*)$/.exec(line);
    if (heading) {
      blocks.push({ kind: "heading", level: heading[1].length, text: heading[2].trim() });
      i += 1;
      continue;
    }
    const bullet = /^(\s*)([-*]|\d+\.)\s+(.*)$/;
    const first = bullet.exec(line);
    if (first) {
      const ordered = /\d/.test(first[2]);
      const items: string[] = [];
      while (i < lines.length && lines[i].trim() !== "") {
        const m = bullet.exec(lines[i]);
        if (m) items.push(m[3]);
        else if (items.length > 0) items[items.length - 1] += ` ${lines[i].trim()}`;
        i += 1;
      }
      blocks.push({ kind: "list", ordered, items });
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && lines[i].trim() !== "" && !/^(#{1,4}\s|```|\s*([-*]|\d+\.)\s)/.test(lines[i])) {
      para.push(lines[i].trim());
      i += 1;
    }
    blocks.push({ kind: "para", text: para.join(" ") });
  }
  return blocks;
}

/** `**bold**`, `*italic*` / `_italic_`, `` `code` `` and `[text](target)`. */
function inline(text: string, onGuide?: (slug: string) => void): ReactNode[] {
  const out: ReactNode[] = [];
  const pattern = /(\*\*([^*]+)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)|\*([^*\s][^*]*)\*|\b_([^_]+)_\b)/g;
  let last = 0;
  let key = 0;
  for (const m of text.matchAll(pattern)) {
    const at = m.index ?? 0;
    if (at > last) out.push(text.slice(last, at));
    const k = key++;
    if (m[2] !== undefined) out.push(<b key={k}>{inline(m[2], onGuide)}</b>);
    else if (m[3] !== undefined) out.push(<code key={k}>{m[3]}</code>);
    else if (m[4] !== undefined) {
      const guide = /^([a-z0-9-]+)\.md$/.exec(m[5]);
      out.push(
        guide && onGuide ? (
          <button key={k} type="button" className="md-link" onClick={() => onGuide(guide[1])}>
            {m[4]}
          </button>
        ) : (
          <span key={k} className="md-ref" title={/^https?:/.test(m[5]) ? m[5] : undefined}>
            {m[4]}
          </span>
        ),
      );
    } else out.push(<em key={k}>{m[6] ?? m[7]}</em>);
    last = at + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export function Markdown({
  source,
  onGuide,
  skipTitle = false,
}: {
  source: string;
  /** Opens another guide, for links like `fixing-setup.md`. */
  onGuide?: (slug: string) => void;
  /** Leave out a leading `# Title` (the page shows it already). */
  skipTitle?: boolean;
}) {
  let blocks = parseBlocks(source);
  if (skipTitle && blocks[0]?.kind === "heading" && blocks[0].level === 1) blocks = blocks.slice(1);
  return (
    <div className="md">
      {blocks.map((b, i) => {
        const k = i;
        switch (b.kind) {
          case "heading": {
            const Tag = (["h2", "h2", "h3", "h4"] as const)[b.level - 1];
            return <Tag key={k}>{inline(b.text, onGuide)}</Tag>;
          }
          case "para":
            return <p key={k}>{inline(b.text, onGuide)}</p>;
          case "code":
            return <pre key={k}>{b.text}</pre>;
          case "list": {
            const Tag = b.ordered ? "ol" : "ul";
            return (
              <Tag key={k}>
                {b.items.map((item, j) => (
                  <li key={`${k}-${j.toString()}`}>{inline(item, onGuide)}</li>
                ))}
              </Tag>
            );
          }
          default:
            return <Fragment key={k} />;
        }
      })}
    </div>
  );
}

/** The `## name` section of a Markdown file, without its heading (up to the next `## `). */
export function section(source: string, name: string): string {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const start = lines.findIndex((l) => l.trim() === `## ${name}`);
  if (start < 0) return "";
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((l) => l.startsWith("## "));
  return (end < 0 ? rest : rest.slice(0, end)).join("\n").trim();
}
