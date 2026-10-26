// SPDX-License-Identifier: MIT OR Apache-2.0
// The install confirmation's version picker ("Add versions" when some are installed): the
// installed versions are locked, a pick goes to the queue by name, blocked files are listed,
// and the file choice reloads the plan.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { calls } from "../test/fakeBridge";
import { renderApp } from "../test/render";
import { InstallDialog } from "./InstallDialog";

const gameName = (id: string) => (id === "zzz" ? "Zenless Zone Zero" : id);

describe("InstallDialog", () => {
  it("locks installed versions and queues only the new pick", async () => {
    const onQueued = vi.fn();
    renderApp(
      <InstallDialog
        modId={553203}
        initialFileId={1314110}
        gameName={gameName}
        onClose={() => {}}
        onQueued={onQueued}
      />,
    );
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Choose versions");
    expect(within(dialog).getByText("Add versions")).toBeTruthy();
    expect(within(dialog).getByText("0 of 2 selected")).toBeTruthy();
    expect(within(dialog).getAllByText("Installed")).toHaveLength(3);

    const blue = within(dialog).getByRole("checkbox", { name: /Blue Blade/ });
    expect((blue as HTMLInputElement).disabled).toBe(true);

    fireEvent.click(within(dialog).getByRole("checkbox", { name: /Void Blade/ }));
    expect(within(dialog).getByText("1 of 2 selected")).toBeTruthy();
    expect(within(dialog).getByText("On after install")).toBeTruthy();

    const install = within(dialog)
      .getAllByRole("button")
      .find((b) => b.className.includes("btn-primary"));
    if (!install) throw new Error("no primary button");
    fireEvent.click(install);
    await waitFor(() => expect(onQueued).toHaveBeenCalled());
    const queued = calls.find((c) => c.cmd === "queue_install");
    expect(queued?.args?.args).toMatchObject({
      mod_id: 553203,
      file_id: 1314110,
      variants: { type: "named", names: ["Void Blade"] },
    });
  });

  it("lists files that won't be installed once a version with them is picked", async () => {
    renderApp(<InstallDialog modId={553203} gameName={gameName} onClose={() => {}} onQueued={() => {}} />);
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Choose versions");
    fireEvent.click(within(dialog).getByRole("checkbox", { name: /Spirit Blade/ }));
    expect(within(dialog).getByText("1 file won't be installed.")).toBeTruthy();
    expect(within(dialog).getByText(/tools\/setup\.exe/)).toBeTruthy();
  });

  it("offers GameBanana's other files and looks inside the one picked", async () => {
    renderApp(<InstallDialog modId={553203} gameName={gameName} onClose={() => {}} onQueued={() => {}} />);
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Choose a file");
    fireEvent.click(within(dialog).getByRole("radio", { name: /sample_swords_lite\.7z/ }));
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === "prepare_install" && c.args?.fileId === 1314111)).toBe(true),
    );
  });
});
