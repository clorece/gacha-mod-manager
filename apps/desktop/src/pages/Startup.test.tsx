// SPDX-License-Identifier: MIT OR Apache-2.0
// The startup screen: with nothing new it opens the app by itself; a newer GMM that can install
// itself starts updating at once, with the bar following it, and nothing opens meanwhile.

import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { calls, scenario } from "../test/fakeBridge";
import { renderApp } from "../test/render";
import { Startup } from "./Startup";

describe("Startup", () => {
  it("opens the app by itself when GMM is up to date", async () => {
    const onDone = vi.fn();
    renderApp(<Startup games={[]} onDone={onDone} />);
    await waitFor(() => expect(onDone).toHaveBeenCalled(), { timeout: 3000 });
    expect(calls.some((c) => c.cmd === "app_update_install")).toBe(false);
  });

  it("updates GMM by itself when a newer one is out", async () => {
    scenario.appUpdateAtStartup = true;
    const onDone = vi.fn();
    renderApp(<Startup games={[]} onDone={onDone} />);
    expect(await screen.findByRole("heading", { name: "Updating GMM" })).toBeTruthy();
    expect(await screen.findByText("Installing; GMM closes and reopens by itself")).toBeTruthy();
    expect(screen.getByRole("progressbar", { name: "Update" }).getAttribute("aria-valuenow")).toBe("100");
    expect(calls.find((c) => c.cmd === "app_update_install")?.args).toMatchObject({ version: "0.2.0-beta" });
    expect(screen.queryByRole("button", { name: /Skip for now|Open GMM|Later/ })).toBeNull();
    expect(onDone).not.toHaveBeenCalled();
  });
});
