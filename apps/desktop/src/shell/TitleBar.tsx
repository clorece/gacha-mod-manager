// SPDX-License-Identifier: MIT OR Apache-2.0
// The window's title bar (docs/08, "Navigation"). The window has no system frame, so this
// strip continues the sidebar and the page up to the top edge in the theme's colours, with
// no title, and draws the caption buttons. A press on the strip moves the window; a double
// click maximizes or restores it. Everything goes through GMM's own window commands
// (window.rs), which act on this window only.

import { type MouseEvent, useEffect, useState } from "react";
import { api } from "../api";

/** `withSide`: the sidebar is below, so the strip over it takes the sidebar's colour. */
export function TitleBar({ withSide }: { withSide: boolean }) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let live = true;
    // Maximizing, restoring and snapping all resize the page.
    const sync = () => {
      api
        .windowIsMaximized()
        .then((m) => live && setMaximized(m))
        .catch(() => {});
    };
    sync();
    window.addEventListener("resize", sync);
    return () => {
      live = false;
      window.removeEventListener("resize", sync);
    };
  }, []);

  const toggle = () => {
    api
      .windowToggleMaximize()
      .then(setMaximized)
      .catch(() => {});
  };

  const press = (e: MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    if (e.detail === 2) toggle();
    else api.windowStartDrag().catch(() => {});
  };

  return (
    <header className={`titlebar${withSide ? " with-side" : ""}`}>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: the drag area is mouse-only, like a system title bar; the buttons below are the accessible controls */}
      <div className="titlebar-drag" onMouseDown={press} />
      <div className="caption" role="toolbar" aria-label="Window">
        <button
          type="button"
          className="cap"
          aria-label="Minimize"
          onClick={() => void api.windowMinimize().catch(() => {})}
        >
          <svg viewBox="0 0 10 10" aria-hidden="true">
            <path d="M0 5h10" />
          </svg>
        </button>
        <button type="button" className="cap" aria-label={maximized ? "Restore" : "Maximize"} onClick={toggle}>
          <svg viewBox="0 0 10 10" aria-hidden="true">
            {maximized ? (
              <>
                <path d="M2.5 2.5V.5h7v7h-2" />
                <rect x=".5" y="2.5" width="7" height="7" />
              </>
            ) : (
              <rect x=".5" y=".5" width="9" height="9" />
            )}
          </svg>
        </button>
        <button
          type="button"
          className="cap cap-close"
          aria-label="Close"
          onClick={() => void api.windowClose().catch(() => {})}
        >
          <svg viewBox="0 0 10 10" aria-hidden="true">
            <path d="M.5.5l9 9M9.5.5l-9 9" />
          </svg>
        </button>
      </div>
    </header>
  );
}
