// SPDX-License-Identifier: MIT OR Apache-2.0
// Settings › About: the version as people read it, and the update check. A release GMM can
// install itself is installed with progress; it only ever asks for the version it found.

import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useAppData } from "../data";
import { calls } from "../test/fakeBridge";
import { renderApp } from "../test/render";
import { Settings } from "./Settings";

function Harness() {
  return <Settings data={useAppData()} />;
}

describe("Settings › About", () => {
  it("checks GitHub for a newer GMM and installs the version it found", async () => {
    renderApp(<Harness />);
    expect(await screen.findByText(/^Gacha Mod Manager \d+\.\d+\.\d+/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    await screen.findByText("GMM 0.2.0 beta is out.");
    fireEvent.click(screen.getByRole("button", { name: "Update now" }));
    expect(await screen.findByText("Installing; GMM closes and reopens by itself.")).toBeTruthy();
    expect(screen.getByRole("progressbar", { name: "Update" }).getAttribute("aria-valuenow")).toBe("100");
    expect(calls.find((c) => c.cmd === "app_update_install")?.args).toMatchObject({ version: "0.2.0-beta" });
    expect(calls.some((c) => c.cmd === "open_app_release")).toBe(false);
  });
});
