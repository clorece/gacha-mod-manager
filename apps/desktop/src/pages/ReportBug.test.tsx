// SPDX-License-Identifier: MIT OR Apache-2.0
// Report a Bug: opening needs a title and sends only what was typed, the newest log is
// chosen to attach, and Change… swaps in the one picked from the log folder.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { calls } from "../test/fakeBridge";
import { renderApp } from "../test/render";
import { ReportBug } from "./ReportBug";

describe("Report a Bug", () => {
  it("opens the issue with what was typed", async () => {
    renderApp(<ReportBug />);
    const open = await screen.findByRole("button", { name: "Open on GitHub…" });
    expect(open).toHaveProperty("disabled", true);

    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "RAR install fails" } });
    fireEvent.change(screen.getByLabelText("What happened?"), { target: { value: "It stops at 40%." } });
    expect(open).toHaveProperty("disabled", false);
    fireEvent.click(open);
    await waitFor(() => expect(calls.some((c) => c.cmd === "open_bug_report")).toBe(true));
    const sent = calls.find((c) => c.cmd === "open_bug_report")?.args;
    expect(sent).toEqual({ title: "RAR install fails", description: "It stops at 40%." });
  });

  it("chooses the newest log and lets another be picked", async () => {
    renderApp(<ReportBug />);
    const group = screen.getByRole("group", { name: "Log file" });
    expect(await within(group).findByText("errors.log")).toBeTruthy();
    expect(within(group).getByText(/^3\.3 KiB · written /)).toBeTruthy();

    fireEvent.click(within(group).getByRole("button", { name: "Change…" }));
    expect(await within(group).findByText("errors.1.log")).toBeTruthy();
    expect(calls.some((c) => c.cmd === "pick_report_log")).toBe(true);
  });
});
