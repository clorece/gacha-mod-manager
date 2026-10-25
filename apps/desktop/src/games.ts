// SPDX-License-Identifier: MIT OR Apache-2.0
// Game banners. Until the banner source is decided (docs/08, open decisions) a banner is the
// game's colors with its short name as a faint watermark; text on it is always white.

import type { GameStatus, SetupState } from "./api";
import type { Tone } from "./ui";

/** The "All games" scope in pickers. */
export const ALL = "all";
export type Scope = typeof ALL | string;

const knownBanners = new Set(["zzz", "wuwa"]);

/** The banner class for a game id; unknown games get the neutral banner. */
export function bannerClass(scope: Scope): string {
  if (scope === ALL) return "banner bn-all";
  return knownBanners.has(scope) ? `banner bn-${scope}` : "banner";
}

const installSource: Record<string, string> = {
  steam: "Steam",
  ho_yo_play: "HoYoPlay",
  epic: "Epic Games",
  manual: "a folder you added",
};

export function sourceLabel(source: string): string {
  return installSource[source] ?? source;
}

/** The short label and tone for a setup state (always shown as text, never color alone). */
export function setupLabel(state: SetupState): { label: string; tone: Tone } {
  switch (state) {
    case "ready":
      return { label: "Ready", tone: "ok" };
    case "xxmi_not_found":
      return { label: "XXMI not found", tone: "bad" };
    default:
      return { label: "Needs setup", tone: "warn" };
  }
}

/** "ZZMI through HoYoPlay": which importer starts which install. */
export function launchRoute(status: GameStatus): string {
  const source = status.xxmi_game_folder_source ?? status.detected_installs[0]?.source;
  return source ? `${status.importer} through ${sourceLabel(source)}` : status.importer;
}
