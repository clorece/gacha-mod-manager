// SPDX-License-Identifier: MIT OR Apache-2.0
// The bundled-Markdown renderer: the shapes the guides use, guide links that stay in the app,
// outside addresses that stay text, and markup in the source that never becomes markup.

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Markdown, parseBlocks, section } from "./markdown";

describe("Markdown", () => {
  it("renders headings, paragraphs, lists, code and inline styles", () => {
    const { container } = render(
      <Markdown
        source={
          "# Title\n\nIntro with **bold**, *italic*, _under_ and `code`.\n\n## Steps\n\n1. One\n2. Two\n   continued\n\n- a\n- b\n\n```\nraw <b>text</b>\n```\n"
        }
        skipTitle
      />,
    );
    expect(container.querySelector("h1")).toBeNull();
    expect(screen.getByRole("heading", { name: "Steps" }).tagName).toBe("H2");
    expect(container.querySelector("p b")?.textContent).toBe("bold");
    expect(container.querySelectorAll("em")).toHaveLength(2);
    expect(container.querySelector("p code")?.textContent).toBe("code");
    expect(container.querySelector("ol")?.textContent).toBe("OneTwo continued");
    expect(container.querySelectorAll("ul li")).toHaveLength(2);
    expect(container.querySelector("pre")?.textContent).toBe("raw <b>text</b>");
  });

  it("opens another guide in the app and leaves outside addresses as text", () => {
    const onGuide = vi.fn();
    const { container } = render(
      <Markdown
        source="See [Conflicts](conflicts.md) and [GitHub](https://github.com/x) or [a doc](../05-roadmap.md)."
        onGuide={onGuide}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Conflicts" }));
    expect(onGuide).toHaveBeenCalledWith("conflicts");
    expect(container.querySelector("a")).toBeNull();
    expect(screen.getByText("GitHub").getAttribute("title")).toBe("https://github.com/x");
    expect(screen.getByText("a doc").getAttribute("title")).toBeNull();
  });

  it("never turns markup in the source into elements", () => {
    const { container } = render(
      <Markdown source={'<img src=x onerror="alert(1)"> <script>alert(1)</script> [x](javascript:alert(1))'} />,
    );
    expect(container.querySelector("img, script, a")).toBeNull();
    expect(container.textContent).toContain("<script>");
  });

  it("finds a ## section of a file", () => {
    const src = "# R\n\n## Credits\n\nThanks.\n\n- one\n\n## License\n\nMIT";
    expect(section(src, "Credits")).toBe("Thanks.\n\n- one");
    expect(section(src, "Missing")).toBe("");
    expect(parseBlocks(section(src, "License"))).toEqual([{ kind: "para", text: "MIT" }]);
  });
});
