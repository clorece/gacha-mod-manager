// SPDX-License-Identifier: MIT OR Apache-2.0
// The Dashboard's open bubble: Launch, and the game's profiles applied from there (Phase 6).

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useAppData } from "../data";
import { calls } from "../test/fakeBridge";
import { renderApp } from "../test/render";
import { Dashboard } from "./Dashboard";

function Harness() {
  const data = useAppData();
  return <Dashboard data={data} />;
}

describe("Dashboard", () => {
  it("applies a profile from the open bubble and reloads that game's library", async () => {
    renderApp(<Harness />);
    fireEvent.click(await screen.findByRole("button", { name: /^Zenless Zone Zero: show/ }));
    expect(screen.getByRole("button", { name: "Launch" })).toBeTruthy();

    const picker = await screen.findByLabelText("Profile");
    const apply = screen.getByRole("button", { name: "Apply" });
    expect(apply).toHaveProperty("disabled", true);

    fireEvent.change(picker, { target: { value: "1" } });
    const before = calls.filter((c) => c.cmd === "library" && c.args?.game === "zzz").length;
    fireEvent.click(apply);
    expect(await screen.findByText(/“Beach day” applied: 1 switched on, 1 switched off\./)).toBeTruthy();
    expect(calls.find((c) => c.cmd === "profile_apply")?.args).toEqual({ game: "zzz", id: 1 });
    await waitFor(() =>
      expect(calls.filter((c) => c.cmd === "library" && c.args?.game === "zzz").length).toBeGreaterThan(before),
    );
  });

  it("offers no profiles for a game that isn't ready", async () => {
    renderApp(<Harness />);
    fireEvent.click(await screen.findByRole("button", { name: /^Wuthering Waves: show/ }));
    expect(await screen.findByText(/The steps are below\./)).toBeTruthy();
    expect(screen.queryByLabelText("Profile")).toBeNull();
    expect(calls.some((c) => c.cmd === "profiles")).toBe(false);
  });
});
