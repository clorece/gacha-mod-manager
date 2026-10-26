// SPDX-License-Identifier: MIT OR Apache-2.0
// The fixed left sidebar (docs/08, "Navigation"): brand, General, Mods (with Select Game),
// Development, and the theme switch with the version at the bottom.

import type { ReactNode } from "react";
import logo from "../assets/gmm-logo.svg";
import type { Scope } from "../games";
import { Badge, Icon, type IconName } from "../ui";
import { GamePicker, type PickerGame } from "./GamePicker";

export type Page = "dashboard" | "downloads" | "settings" | "library" | "store" | "bug" | "docs" | "styleguide";

function NavItem({
  page,
  current,
  icon,
  onGo,
  children,
}: {
  page: Page;
  current: Page;
  icon: IconName;
  onGo: (page: Page) => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className="nav-item"
      aria-current={page === current ? "page" : undefined}
      onClick={() => onGo(page)}
    >
      <Icon name={icon} />
      {children}
    </button>
  );
}

export function Sidebar({
  page,
  onGo,
  games,
  scope,
  onScope,
  theme,
  onToggleTheme,
  activeDownloads,
}: {
  page: Page;
  onGo: (page: Page) => void;
  games: PickerGame[];
  scope: Scope;
  onScope: (scope: Scope) => void;
  theme: "dark" | "light";
  onToggleTheme: () => void;
  /** Installs queued or running, shown on the Downloads item. */
  activeDownloads: number;
}) {
  const other = theme === "dark" ? "light" : "dark";
  return (
    <aside className="side" aria-label="Navigation">
      <div className="brand">
        <img src={logo} alt="" />
        <div className="brand-text">
          <div className="brand-title">Gacha Mod Manager</div>
          <div className="gb-pill">
            <span className="gb-dot" />
            Powered by GameBanana
          </div>
        </div>
      </div>

      <nav aria-label="Pages" className="contents">
        <div className="nav-head kicker">General</div>
        <NavItem page="dashboard" current={page} icon="dashboard" onGo={onGo}>
          Dashboard
        </NavItem>
        <NavItem page="downloads" current={page} icon="download" onGo={onGo}>
          Downloads
          {activeDownloads > 0 && (
            <Badge tone="yellow" title={`${activeDownloads} in the queue`}>
              {activeDownloads}
            </Badge>
          )}
        </NavItem>
        <NavItem page="settings" current={page} icon="sliders" onGo={onGo}>
          Settings
        </NavItem>

        <div className="nav-head kicker">Mods</div>
        <GamePicker games={games} scope={scope} onScope={onScope} />
        <NavItem page="library" current={page} icon="layers" onGo={onGo}>
          Mods Library
        </NavItem>
        <NavItem page="store" current={page} icon="bag" onGo={onGo}>
          Mods Store
        </NavItem>

        <div className="nav-head kicker">Development</div>
        <NavItem page="bug" current={page} icon="bug" onGo={onGo}>
          Report a Bug
        </NavItem>
        <NavItem page="docs" current={page} icon="book" onGo={onGo}>
          Documentation
        </NavItem>
        {import.meta.env.DEV && (
          <NavItem page="styleguide" current={page} icon="palette" onGo={onGo}>
            Style guide
            <Badge tone="quiet">DEV</Badge>
          </NavItem>
        )}
      </nav>

      <div className="side-foot">
        <button type="button" className="theme-btn" onClick={onToggleTheme} aria-label={`Switch to ${other} theme`}>
          <Icon name={theme === "dark" ? "moon" : "sun"} size={16} />
          {theme === "dark" ? "Dark" : "Light"}
        </button>
        <span>v{__APP_VERSION__}</span>
      </div>
    </aside>
  );
}
