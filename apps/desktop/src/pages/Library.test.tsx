// SPDX-License-Identifier: MIT OR Apache-2.0
// The Mods Library against the fake bridge: mod cards (versions as one card, previews, badges),
// the sensitive filter, the conflict prompt, select mode with Delete selected, and the Mod
// settings panel's Versions section.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useOwned } from "../catalog";
import { useAppData } from "../data";
import { calls } from "../test/fakeBridge";
import * as fx from "../test/fixtures";
import { renderApp } from "../test/render";
import { Library } from "./Library";

const noop = () => {};

function Harness({ onAddVersions = noop }: { onAddVersions?: (modId: number, fileId: number | null) => void }) {
  const data = useAppData();
  const owned = useOwned(data, fx.updates);
  return (
    <Library
      data={data}
      scope="zzz"
      onScope={noop}
      updates={fx.updates}
      owned={owned}
      onQueued={noop}
      focus={null}
      onFocusDone={noop}
      onOpenMod={noop}
      onAddVersions={onAddVersions}
    />
  );
}

const card = (name: string) => {
  const title = screen.getByRole("heading", { name, level: 3 });
  const el = title.closest("article");
  if (!el) throw new Error(`no card for ${name}`);
  return el;
};

describe("Mods Library", () => {
  it("shows one card per mod, with versions grouped and sensitive mods hidden", async () => {
    renderApp(<Harness />);
    await screen.findByRole("heading", { name: "Sample Sword Pack", level: 3 });

    // Six rows and two hand-installed folders: the three sword versions share a card and the
    // sensitive one is hidden by default.
    expect(screen.getAllByRole("article")).toHaveLength(5);
    expect(within(card("Sample Sword Pack")).getByText("Red Blade · 3 versions")).toBeTruthy();
    expect(screen.getByText(/1 hidden as sensitive/)).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Swimsuit Recolor" })).toBeNull();

    // The startup check found an update for Casual Outfit; it also clashes with a mod that's on.
    expect(within(card("Casual Outfit")).getByText("Update")).toBeTruthy();
    expect(within(card("Hand Folder")).getByText("Hand-installed")).toBeTruthy();
    expect(within(card("Old Install")).getByRole("img", { name: /no preview image/ })).toBeTruthy();
  });

  it("Show sensitive reveals the hidden mod and saves the preference", async () => {
    renderApp(<Harness />);
    await screen.findByRole("heading", { name: "Sample Sword Pack", level: 3 });
    fireEvent.click(screen.getByRole("switch", { name: "Show sensitive" }));
    expect(await screen.findByRole("heading", { name: "Swimsuit Recolor" })).toBeTruthy();
    expect(within(card("Swimsuit Recolor")).getByText("Sensitive")).toBeTruthy();
    expect(calls.some((c) => c.cmd === "set_preferences")).toBe(true);
  });

  it("asks before switching on a mod that conflicts, and Enable anyway goes ahead", async () => {
    renderApp(<Harness />);
    await screen.findByRole("heading", { name: "Sample Sword Pack", level: 3 });
    fireEvent.click(screen.getByRole("switch", { name: "Show sensitive" }));
    await screen.findByRole("heading", { name: "Swimsuit Recolor" });

    fireEvent.click(screen.getByRole("switch", { name: "Swimsuit Recolor on or off" }));
    expect(await screen.findByText("Swimsuit Recolor conflicts with Casual Outfit [gb-688491].")).toBeTruthy();
    expect(screen.getByText(/Both replace the same 2 things in game/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Enable anyway" }));
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === "set_enabled" && c.args?.anyway === true && c.args?.enabled === true)).toBe(
        true,
      ),
    );
  });

  it("deletes every version of a selected mod after the itemized confirmation", async () => {
    renderApp(<Harness />);
    await screen.findByRole("heading", { name: "Sample Sword Pack", level: 3 });
    fireEvent.click(screen.getByRole("button", { name: "Select" }));
    fireEvent.click(screen.getByRole("button", { name: "Select Sample Sword Pack" }));
    expect(screen.getByText("1 mod selected")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Delete selected" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Deleted for good, not moved to the Recycle Bin/)).toBeTruthy();
    const preview = calls.find((c) => c.cmd === "uninstall_preview");
    expect(preview?.args?.keys).toEqual([
      { type: "managed", id: 10 },
      { type: "managed", id: 11 },
      { type: "managed", id: 12 },
    ]);

    fireEvent.click(within(dialog).getByRole("button", { name: "Delete 3" }));
    await waitFor(() => expect(calls.some((c) => c.cmd === "uninstall")).toBe(true));
  });

  it("Mod settings lists the installed versions and offers more from the cache", async () => {
    const onAddVersions = vi.fn();
    renderApp(<Harness onAddVersions={onAddVersions} />);
    await screen.findByRole("heading", { name: "Sample Sword Pack", level: 3 });
    fireEvent.click(screen.getByRole("button", { name: "Mod settings for Sample Sword Pack" }));

    const panel = await screen.findByRole("dialog");
    for (const name of ["Blue Blade", "Red Blade", "Gold Blade"]) {
      expect(within(panel).getByRole("switch", { name: `${name} on or off` })).toBeTruthy();
    }
    expect(within(panel).getByRole("switch", { name: "Red Blade on or off" }).getAttribute("aria-checked")).toBe(
      "true",
    );

    fireEvent.click(within(panel).getByRole("switch", { name: "Gold Blade on or off" }));
    await waitFor(() => expect(calls.some((c) => c.cmd === "switch_version" && c.args?.id === 12)).toBe(true));

    fireEvent.click(within(panel).getByRole("button", { name: "Add versions" }));
    expect(onAddVersions).toHaveBeenCalledWith(553203, 1314110);
  });
});
