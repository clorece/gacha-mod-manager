// SPDX-License-Identifier: MIT OR Apache-2.0
// Vitest setup: every test talks to the fake bridge instead of Rust, starts from the fixtures
// with no remembered page state, and unmounts what it rendered. jsdom lacks the native
// <dialog> methods and scrollIntoView, which the primitives call, so they get small stand-ins
// here.

import { mockIPC } from "@tauri-apps/api/mocks";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";
import { forgetAll } from "../remember";
import { fakeInvoke, resetBridge } from "./fakeBridge";

beforeEach(() => {
  resetBridge();
  forgetAll();
  mockIPC((cmd, payload) => fakeInvoke(cmd, payload as Record<string, unknown>));
});

afterEach(() => cleanup());

const proto = HTMLDialogElement.prototype;
proto.showModal ??= function (this: HTMLDialogElement) {
  this.setAttribute("open", "");
};
proto.close ??= function (this: HTMLDialogElement) {
  this.removeAttribute("open");
};
Element.prototype.scrollIntoView ??= () => {};
window.matchMedia ??= (query: string) =>
  ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }) as MediaQueryList;
