// SPDX-License-Identifier: MIT OR Apache-2.0
// Report a Bug: the preview comes from Rust's draft, opening needs a title and sends only
// what was typed, and the error log lists newest first and clears.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { calls } from "../test/fakeBridge";
import { renderApp } from "../test/render";
import { ReportBug } from "./ReportBug";

describe("Report a Bug", () => {
  it("previews the issue Rust drafts and opens it with what was typed", async () => {
    renderApp(<ReportBug />);
    const open = await screen.findByRole("button", { name: "Open on GitHub…" });
    expect(open).toHaveProperty("disabled", true);

    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "RAR install fails" } });
    fireEvent.change(screen.getByLabelText("What happened?"), { target: { value: "It stops at 40%." } });
    expect(await screen.findByText(/It stops at 40%\./, { selector: "pre" })).toBeTruthy();
    expect(screen.getByText("RAR install fails", { selector: ".report-preview b" })).toBeTruthy();

    expect(open).toHaveProperty("disabled", false);
    fireEvent.click(open);
    await waitFor(() => expect(calls.some((c) => c.cmd === "open_bug_report")).toBe(true));
    const sent = calls.find((c) => c.cmd === "open_bug_report")?.args;
    expect(sent).toEqual({ title: "RAR install fails", description: "It stops at 40%." });
  });

  it("warns when the description had to be shortened", async () => {
    renderApp(<ReportBug />);
    fireEvent.change(await screen.findByLabelText("What happened?"), { target: { value: "x".repeat(2000) } });
    expect(await screen.findByText("Your description was shortened to fit the link.")).toBeTruthy();
  });

  it("lists the error log newest first and clears it", async () => {
    renderApp(<ReportBug />);
    const list = await screen.findByRole("list", { name: "The newest 2 errors" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows[0].textContent).toContain("XXMI Launcher Config.json");
    expect(rows[1].textContent).toContain("the archive is damaged");

    fireEvent.click(screen.getByRole("button", { name: "Clear log" }));
    expect(await screen.findByText("No errors logged.")).toBeTruthy();
    expect(calls.some((c) => c.cmd === "clear_error_log")).toBe(true);
  });
});
