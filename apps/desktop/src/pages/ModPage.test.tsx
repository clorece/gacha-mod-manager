// SPDX-License-Identifier: MIT OR Apache-2.0
// GMM's mod page from the recorded-shape fixture: a snapshot of the header and license (the
// part built from GameBanana's fields), plus the tabs and the per-file Install.

import { fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NOW } from "../test/fixtures";
import { renderApp } from "../test/render";
import { ModPage } from "./ModPage";

describe("ModPage", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(NOW * 1000);
  });
  afterEach(() => vi.useRealTimers());

  it("renders the header and license from the mod page fixture", async () => {
    const { container } = renderApp(
      <ModPage
        modId={553203}
        owned={undefined}
        backLabel="Mods Store"
        onBack={() => {}}
        onDownload={() => {}}
        onManage={() => {}}
      />,
    );
    await screen.findByRole("heading", { name: "Sample Sword Pack", level: 1 });
    expect(container.querySelector(".mp-head")?.outerHTML).toMatchSnapshot();
    expect(container.querySelector(".mp-license")?.outerHTML).toMatchSnapshot();
  });

  it("shows the description safely and installs a chosen file", async () => {
    const onDownload = vi.fn();
    renderApp(
      <ModPage
        modId={553203}
        owned={undefined}
        backLabel="Mods Store"
        onBack={() => {}}
        onDownload={onDownload}
        onManage={() => {}}
      />,
    );
    await screen.findByRole("heading", { name: "Sample Sword Pack", level: 1 });
    expect(screen.getByText("the guide").getAttribute("href")).toBe("#gmm-link:0123456789abcdef0123456789abcdef");

    fireEvent.click(screen.getByRole("tab", { name: /Files/ }));
    const lite = screen.getByText("sample_swords_lite.7z").closest(".mp-file") as HTMLElement;
    fireEvent.click(within(lite).getByRole("button", { name: /Install/ }));
    expect(onDownload).toHaveBeenCalledWith(expect.objectContaining({ id: 553203 }), 1314111);
  });
});
