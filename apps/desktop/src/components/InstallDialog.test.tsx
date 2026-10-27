// SPDX-License-Identifier: MIT OR Apache-2.0
// The install confirmation: it never downloads; it asks for the variants (GameBanana's files,
// several at once), then about mods on for the same character, then confirms, and queues one
// install per file picked.

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { calls } from "../test/fakeBridge";
import * as fx from "../test/fixtures";
import { renderApp } from "../test/render";
import { InstallDialog } from "./InstallDialog";

function primary(dialog: HTMLElement): HTMLElement {
  const button = within(dialog)
    .getAllByRole("button")
    .find((b) => b.className.includes("btn-primary"));
  if (!button) throw new Error("no primary button");
  return button;
}

const queued = () => calls.filter((c) => c.cmd === "queue_install").map((c) => c.args?.args);

describe("InstallDialog", () => {
  it("asks for variants first and queues every one picked, switched on", async () => {
    const onQueued = vi.fn();
    renderApp(
      <InstallDialog modId={553203} games={fx.games} libraries={fx.libraries} onClose={() => {}} onQueued={onQueued} />,
    );
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Choose variants");
    // Some of the newest file's versions are installed already.
    expect(within(dialog).getByText("Add versions")).toBeTruthy();
    expect(within(dialog).getByText("Some installed")).toBeTruthy();
    expect(within(dialog).getByText("1 of 2 selected")).toBeTruthy();

    fireEvent.click(within(dialog).getByRole("checkbox", { name: /sample_swords_lite\.7z/ }));
    expect(within(dialog).getAllByText("On after install")).toHaveLength(2);

    fireEvent.click(primary(dialog));
    await within(dialog).findByText("Switch it on after installing");
    fireEvent.click(primary(dialog));
    await waitFor(() => expect(onQueued).toHaveBeenCalledWith(2, "Sample Sword Pack"));
    expect(queued()).toEqual([
      expect.objectContaining({ file_id: 1314110, start_disabled: false, variants: { type: "only" } }),
      expect.objectContaining({ file_id: 1314111, start_disabled: false }),
    ]);
    expect(calls.some((c) => c.cmd.startsWith("prepare"))).toBe(false);
  });

  it("never picks or queues a file that's installed whole", async () => {
    // The newest file is installed as one mod; the user adds the other file.
    const libraries = structuredClone(fx.libraries);
    const zzz = libraries.zzz;
    if (!zzz) throw new Error("no zzz library");
    const sword = zzz.managed.find((m) => m.gb_mod_id === 553203);
    if (!sword) throw new Error("no sword pack");
    zzz.managed = [
      ...zzz.managed.filter((m) => m.gb_mod_id !== 553203),
      { ...sword, gb_file_id: 1314110, variant: null, group: null },
    ];
    const onQueued = vi.fn();
    renderApp(
      <InstallDialog modId={553203} games={fx.games} libraries={libraries} onClose={() => {}} onQueued={onQueued} />,
    );
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Choose variants");
    expect(within(dialog).getByText("Installed")).toBeTruthy();
    expect(within(dialog).getByText("0 of 2 selected")).toBeTruthy();

    fireEvent.click(within(dialog).getByRole("checkbox", { name: /sample_swords_lite\.7z/ }));
    expect(within(dialog).getByText("1 of 2 selected")).toBeTruthy();
    fireEvent.click(primary(dialog));
    await within(dialog).findByText("Switch it on after installing");
    expect(primary(dialog).textContent).toBe("Install");
    fireEvent.click(primary(dialog));
    await waitFor(() => expect(onQueued).toHaveBeenCalledWith(1, "Sample Sword Pack"));
    expect(queued()).toEqual([expect.objectContaining({ file_id: 1314111 })]);
  });

  it("resolves a conflict with a mod on for the same character", async () => {
    renderApp(
      <InstallDialog modId={700101} games={fx.games} libraries={fx.libraries} onClose={() => {}} onQueued={() => {}} />,
    );
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Resolve a conflict");
    expect(within(dialog).getByText("Casual Outfit")).toBeTruthy();

    // Switch the one that's on off once this one is installed.
    fireEvent.click(primary(dialog));
    await within(dialog).findByText("Switches off Casual Outfit");
    fireEvent.click(within(dialog).getByRole("button", { name: "Back" }));

    // Or keep it, and install this one switched off.
    fireEvent.click(within(dialog).getByRole("radio", { name: /Keep what's on/ }));
    fireEvent.click(primary(dialog));
    await within(dialog).findByText("Installs switched off");
    fireEvent.click(primary(dialog));
    await waitFor(() => expect(queued()).toHaveLength(1));
    expect(queued()[0]).toMatchObject({ mod_id: 700101, file_id: 1500101, start_disabled: true, switch_off: [] });
  });

  it("switches off the other side when the new mod wins", async () => {
    renderApp(
      <InstallDialog modId={700101} games={fx.games} libraries={fx.libraries} onClose={() => {}} onQueued={() => {}} />,
    );
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("Resolve a conflict");
    fireEvent.click(primary(dialog));
    await within(dialog).findByText("Switches off Casual Outfit");
    fireEvent.click(primary(dialog));
    await waitFor(() => expect(queued()).toHaveLength(1));
    expect(queued()[0]).toMatchObject({ start_disabled: false, switch_off: [1] });
  });
});
