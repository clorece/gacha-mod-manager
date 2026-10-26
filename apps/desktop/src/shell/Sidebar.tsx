// SPDX-License-Identifier: MIT OR Apache-2.0
// The fixed left sidebar (docs/08, "Navigation"): brand, General, Mods (with Select Game),
// Development, and the theme switch with the version at the bottom.

import type { ReactNode } from "react";
import type { AppRelease } from "../api";
import logo from "../assets/gmm-logo.svg";
import { plural } from "../format";
import type { Scope } from "../games";
import { isActive, useJobs } from "../queue";
import { Badge, Icon, type IconName, useToast } from "../ui";
import { APP_VERSION, openRelease, versionLabel } from "../version";
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

/** The Downloads count (installs queued, running or waiting for the user): a disc in the
 *  theme's text colour with an inset ring and the number in its base colour, so it stands
 *  out in both themes and on the selected item. Reads the queue itself, so progress redraws
 *  only this. */
function DownloadsCount() {
  const active = useJobs().filter(isActive);
  const count = active.length;
  const needsAction = active.filter((j) => j.state.type === "needs_action").length;
  if (count === 0) return null;
  const label = [
    plural(count, "download"),
    needsAction > 0 ? `${needsAction} ${needsAction === 1 ? "needs" : "need"} your attention` : null,
  ]
    .filter(Boolean)
    .join(", ");
  return (
    <span className={`count-bubble${count > 9 ? " is-wide" : ""}`} title={label}>
      <span className="sr-only">{label}</span>
      <span aria-hidden="true">{count > 99 ? "99+" : count}</span>
    </span>
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
  appUpdate,
}: {
  page: Page;
  onGo: (page: Page) => void;
  games: PickerGame[];
  scope: Scope;
  onScope: (scope: Scope) => void;
  theme: "dark" | "light";
  onToggleTheme: () => void;
  /** A newer GMM on GitHub: the version line becomes a button to its release page. */
  appUpdate: AppRelease | null;
}) {
  const toast = useToast();
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
          <DownloadsCount />
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
        {appUpdate ? (
          <button
            type="button"
            className="update-pill"
            title={`GMM ${versionLabel(appUpdate.version)} is out; you have ${APP_VERSION}`}
            onClick={() => openRelease(appUpdate.version, (m) => toast("bad", m))}
          >
            <Icon name="download" size={13} stroke={2.6} />
            {versionLabel(appUpdate.version)}
          </button>
        ) : (
          <span>v{APP_VERSION}</span>
        )}
      </div>
    </aside>
  );
}
