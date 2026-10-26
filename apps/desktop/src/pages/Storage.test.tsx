// SPDX-License-Identifier: MIT OR Apache-2.0
// The storage breakdown: the All games overview, the per-game table (names from the library,
// sorting, selecting the switched-off mods) and the delete confirmation it opens.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useAppData } from "../data";
import { calls } from "../test/fakeBridge";
import { renderApp } from "../test/render";
import { StorageBreakdown } from "./Storage";

function Harness() {
  const data = useAppData();
  return data.loading ? null : <StorageBreakdown data={data} onBack={() => {}} />;
}

const rowNames = () =>
  screen
    .getAllByRole("row")
    .slice(1)
    .map((r) => within(r).getAllByRole("cell")[1].querySelector("b")?.textContent);

describe("Storage breakdown", () => {
  it("shows one bar per game and the cache, with a game's problem in place of its bar", async () => {
    renderApp(<Harness />);
    expect(await screen.findByText("GMM's cache")).toBeTruthy();
    expect(screen.getByRole("img", { name: /Zenless Zone Zero: 274\.7 MiB in its Mods folder/ })).toBeTruthy();
    expect(screen.getByText("XXMI's WWMI folder is missing.")).toBeTruthy();
  });

  it("sorts the table and selects the switched-off mods for deleting", async () => {
    renderApp(<Harness />);
    await screen.findByText("GMM's cache");
    fireEvent.click(screen.getByRole("button", { name: "Zenless Zone Zero" }));
    await screen.findByRole("table");

    // Biggest first by default; versions carry their name.
    expect(rowNames()).toEqual([
      "Casual Outfit",
      "Hand Folder",
      "Swimsuit Recolor",
      "Sample Sword Pack · Red Blade",
      "Sample Sword Pack · Blue Blade",
    ]);
    fireEvent.click(screen.getByRole("button", { name: /^Mod/ }));
    expect(rowNames()[0]).toBe("Casual Outfit");
    expect(screen.getByRole("columnheader", { name: /Mod/ }).getAttribute("aria-sort")).toBe("ascending");

    fireEvent.click(screen.getByRole("button", { name: "Select switched off" }));
    expect(screen.getByText(/2 mods · /)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Delete…" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Delete 2 mods?")).toBeTruthy();
    await waitFor(() => {
      const keys = calls.find((c) => c.cmd === "uninstall_preview")?.args?.keys as { id: number }[];
      expect(keys.map((k) => k.id).sort()).toEqual([10, 2]);
    });
  });
});
