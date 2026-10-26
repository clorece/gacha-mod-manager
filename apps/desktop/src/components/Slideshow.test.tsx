// SPDX-License-Identifier: MIT OR Apache-2.0
// The hand-built slideshow: arrows and ←/→ wrap around, thumbnails jump, a click opens the
// lightbox, and one or no image means no controls.

import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { summary } from "../test/fixtures";
import { renderApp } from "../test/render";
import { Slideshow } from "./Slideshow";

const images = summary(1, "x").images;

describe("Slideshow", () => {
  it("steps with the arrows and the keyboard, wrapping at the ends", () => {
    renderApp(<Slideshow images={images} name="Sample" sensitive={false} />);
    expect(screen.getByText("1 / 3")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Next image" }));
    expect(screen.getByText("2 / 3")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Previous image" }));
    fireEvent.click(screen.getByRole("button", { name: "Previous image" }));
    expect(screen.getByText("3 / 3")).toBeTruthy();
    fireEvent.keyDown(screen.getByRole("button", { name: /Enlarge image 3 of 3/ }), { key: "ArrowRight" });
    expect(screen.getByText("1 / 3")).toBeTruthy();
  });

  it("opens the lightbox on the current image", () => {
    renderApp(<Slideshow images={images} name="Sample" sensitive={false} />);
    fireEvent.click(screen.getByRole("button", { name: /Enlarge image 1 of 3/ }));
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("shows no controls for one image and a placeholder for none", () => {
    const { unmount } = renderApp(<Slideshow images={images.slice(0, 1)} name="One" sensitive={false} />);
    expect(screen.queryByRole("button", { name: "Next image" })).toBeNull();
    unmount();
    renderApp(<Slideshow images={[]} name="None" sensitive={false} />);
    expect(screen.getByRole("button", { name: "None: no images" })).toHaveProperty("disabled", true);
  });
});
