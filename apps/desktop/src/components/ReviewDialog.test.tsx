// SPDX-License-Identifier: MIT OR Apache-2.0
// An install that needs action: the reasons, the versions to choose (all ticked, each on
// unless it clashes with one picked before it), and going ahead or cancelling.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Review } from "../api";
import { calls } from "../test/fakeBridge";
import * as fx from "../test/fixtures";
import { renderApp } from "../test/render";
import { ReviewDialog, versionTags } from "./ReviewDialog";

describe("ReviewDialog", () => {
  it("shows why it stopped and goes ahead with the versions picked", async () => {
    const onClose = vi.fn();
    renderApp(<ReviewDialog job={fx.reviewJob} review={fx.review} onClose={onClose} />);
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Needs your attention")).toBeTruthy();
    expect(within(dialog).getByText("2 of 2 selected")).toBeTruthy();
    expect(within(dialog).getByText(/won't be installed/)).toBeTruthy();
    expect(within(dialog).getByText("Spirit Blade: tools/setup.exe")).toBeTruthy();
    expect(within(dialog).getByText("On after install")).toBeTruthy();
    expect(within(dialog).getByText("Off · clashes with Void Blade")).toBeTruthy();

    // Without Spirit Blade nothing is blocked any more.
    fireEvent.click(within(dialog).getByRole("checkbox", { name: /Spirit Blade/ }));
    expect(within(dialog).queryByText(/won't be installed/)).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: /Install 1 version/ }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(calls.find((c) => c.cmd === "queue_proceed")?.args).toEqual({
      id: 3,
      variants: { type: "named", names: ["Void Blade"] },
    });
  });

  it("cancels the download", async () => {
    renderApp(<ReviewDialog job={fx.reviewJob} review={fx.review} onClose={() => {}} />);
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel download" }));
    await waitFor(() => expect(calls.some((c) => c.cmd === "queue_cancel" && c.args?.id === 3)).toBe(true));
  });

  it("keeps every version on that doesn't clash with one before it", () => {
    const review: Review = {
      issues: [{ type: "choose_versions", count: 3 }],
      versions: ["Top", "Skirt", "Alt top"].map((name) => ({
        name,
        folder: name,
        files: 1,
        bytes: 1,
        installed: null,
        blocked: [],
        warnings: [],
      })),
      version_conflicts: [{ a: 0, b: 2, hashes: 3 }],
    };
    const tags = versionTags(review, ["Top", "Skirt", "Alt top"], true);
    expect(Object.fromEntries(tags)).toEqual({
      Top: "On after install",
      Skirt: "On after install",
      "Alt top": "Off · clashes with Top",
    });
    // Without "Top", the alternative is on.
    expect(versionTags(review, ["Skirt", "Alt top"], true).get("Alt top")).toBe("On after install");
  });
});
