// SPDX-License-Identifier: MIT OR Apache-2.0
// Settings › About: the version as people read it, and the update check, which only ever
// opens the release page of the version it found.

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useAppData } from "../data";
import { calls } from "../test/fakeBridge";
import { renderApp } from "../test/render";
import { Settings } from "./Settings";

function Harness() {
  return <Settings data={useAppData()} />;
}

describe("Settings › About", () => {
  it("checks GitHub for a newer GMM and opens that release's page", async () => {
    renderApp(<Harness />);
    expect(await screen.findByText(/^Gacha Mod Manager \d+\.\d+\.\d+/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    await screen.findByText("GMM 0.2.0 beta is out.");
    fireEvent.click(screen.getByRole("button", { name: "Get the update" }));
    await waitFor(() =>
      expect(calls.find((c) => c.cmd === "open_app_release")?.args).toEqual({ version: "0.2.0-beta" }),
    );
  });
});
