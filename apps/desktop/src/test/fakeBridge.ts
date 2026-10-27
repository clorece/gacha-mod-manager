// SPDX-License-Identifier: MIT OR Apache-2.0
// A stand-in for GMM's Rust commands, answering from fixtures.ts. Vitest installs it with
// `@tauri-apps/api/mocks` (setup.ts); the contrast audit (tests/ui-audit.mjs) installs it as
// `window.__TAURI_INTERNALS__.invoke` in a plain browser. It keeps a little state (preferences,
// switches) so pages behave like the real app, and records every call for assertions.

import type { CmdError, ModKey, ModSummary, Preferences } from "../api";
import * as fx from "./fixtures";

type Args = Record<string, unknown> | undefined;

export const calls: { cmd: string; args: Args }[] = [];

/** Switches a test can flip; `resetBridge` turns them off again. */
export const scenario = {
  /** The startup check finds a newer GMM that can install itself. */
  appUpdateAtStartup: false,
};

let prefs: Preferences = { ...fx.preferences };
let libraries = structuredClone(fx.libraries);

/** Back to the fixtures (between tests); `overrides` changes the preferences. */
export function resetBridge(overrides: Partial<Preferences> = {}): void {
  calls.length = 0;
  scenario.appUpdateAtStartup = false;
  prefs = { ...fx.preferences, ...overrides };
  libraries = structuredClone(fx.libraries);
}

/** The newer GMM the update checks find. */
const newerGmm = {
  version: "0.2.0-beta",
  name: "GMM 0.2 beta",
  prerelease: true,
  published_at: "2026-11-02T10:00:00Z",
  installable: true,
};

function fail(message: string): never {
  const err: CmdError = { kind: "other", message };
  throw err;
}

function setEnabled(game: string, key: ModKey, enabled: boolean): void {
  const view = libraries[game];
  if (!view) fail(`unknown game: ${game}`);
  if (key.type === "managed") {
    const m = view.managed.find((r) => r.id === key.id);
    if (!m) fail(`no installed mod #${key.id}`);
    m.enabled = enabled;
    m.disk = enabled ? "enabled" : "disabled";
  } else {
    const u = view.unmanaged.find((r) => r.path === key.path);
    if (!u) fail("that folder isn't in the Mods folder");
    u.enabled = enabled;
  }
}

/** The fixture mods are ZZZ's; WuWa's lists get their own ids (real mods never share one). */
function forGame(m: ModSummary, game: string | undefined): ModSummary {
  return game === "wuwa" ? { ...m, id: m.id + 1_000_000, game_id: 20357, name: `${m.name} (WuWa)` } : m;
}

export async function fakeInvoke(cmd: string, args: Args = {}): Promise<unknown> {
  calls.push({ cmd, args });
  const a = (args ?? {}) as Record<string, never>;
  switch (cmd) {
    case "games":
      return fx.games;
    case "statuses":
      return fx.statuses;
    case "library":
      return libraries[a.game] ?? fail(`unknown game: ${a.game}`);
    case "preferences":
      return prefs;
    case "set_preferences":
      prefs = { ...(a.preferences as Preferences) };
      return prefs;
    case "settings":
      return fx.settings;
    case "storage":
      return fx.storage;
    case "startup_check":
      return { checked: 4, updates: fx.updates, notes: [], app_update: scenario.appUpdateAtStartup ? newerGmm : null };
    case "app_update_check":
      return newerGmm;
    case "open_app_release":
      return true;
    case "app_update_install": {
      const send = (a.onProgress as { onmessage?: (p: unknown) => void } | undefined)?.onmessage;
      send?.({ phase: "downloading", downloaded: 4_500_000, total: 9_000_000 });
      send?.({ phase: "installing" });
      return null;
    }
    case "catalog_list":
    case "catalog_search": {
      const game = (a.query as { game?: string } | undefined)?.game ?? a.game;
      const page = fx.catalogPage.page;
      return { ...fx.catalogPage, page: { ...page, items: page.items.map((m) => forGame(m, game)) } };
    }
    case "catalog_featured":
      return fx.featured.map((f) => ({ ...f, summary: forGame(f.summary, a.game) }));
    case "catalog_stats":
      return null;
    case "catalog_categories":
      return fx.categories;
    case "catalog_mod":
      return a.modId === 700101 ? fx.jacketProfile : fx.modProfile;
    case "catalog_updates":
      return fx.modUpdates;
    case "queue_jobs":
      return fx.jobs;
    case "queue_watch":
      // The page gets the list the way the app does: through the channel.
      (a.onChange as { onmessage?: (jobs: unknown) => void } | undefined)?.onmessage?.([...fx.jobs, fx.reviewJob]);
      return null;
    case "drop_watch":
      return null;
    case "queue_proceed":
    case "queue_cancel":
      return true;
    case "queue_install":
    case "queue_change_file":
      return 99;
    case "conflicts":
      return {
        game: a.game,
        mods_checked: 4,
        conflicts:
          a.game === "zzz"
            ? [
                {
                  a: { type: "managed", id: 1, folder: "Casual Outfit [gb-688491]" },
                  b: { type: "unmanaged", path: "Hand Folder" },
                  hashes: ["0a1b2c3d"],
                },
              ]
            : [],
        libraries: [],
        notes: [],
      };
    case "set_enabled":
      {
        // "Swimsuit Recolor" clashes with "Casual Outfit", which is on: refused unless anyway.
        const key = a.key as ModKey;
        if (a.enabled && !a.anyway && key.type === "managed" && key.id === 2) {
          const err: CmdError = {
            kind: "conflicts",
            message: "switching this on would conflict with 1 mod(s) that are already on",
            conflicts: [
              {
                a: { type: "managed", id: 2, folder: "DISABLED Swimsuit Recolor" },
                b: { type: "managed", id: 1, folder: "Casual Outfit [gb-688491]" },
                hashes: ["0a1b2c3d", "4e5f6a7b"],
              },
            ],
          };
          throw err;
        }
      }
      setEnabled(a.game, a.key as ModKey, a.enabled as boolean);
      return { key: a.key, enabled: a.enabled, changed: true };
    case "switch_version":
      return [];
    case "uninstall_preview":
      return fx.uninstallPreview(a.keys as never);
    case "uninstall":
      return (a.keys as ModKey[]).map((key) => ({ key, removed: true }));
    case "mod_keybindings":
      return fx.keybindings;
    case "rollback_files":
      return [];
    case "profiles":
      return a.game === "zzz" ? fx.profiles : [];
    case "profile_apply":
      return { switched_on: ["Beach outfit"], switched_off: ["Casual Velina"], missing: [] };
    case "network_status":
      return { cooldown_secs: null, showing_saved: false };
    case "launch_plan":
      return { exe: "C:\\XXMI\\XXMI Launcher.exe", args: ["--nogui", "--xxmi", "ZZMI"], confirmed: true };
    case "clear_image_cache":
      return { bytes: 52_000_000, files: 300, truncated: false };
    case "clear_download_cache":
      return { archives: 6, bytes: 180_000_000 };
    case "clear_http_cache":
      return 120;
    case "report_log":
      return fx.reportLog;
    case "pick_report_log":
      return fx.pickedLog;
    case "log_ui_error":
      return null;
    case "open_bug_report":
      return true;
    case "window_is_maximized":
      return false;
    case "window_minimize":
    case "window_close":
    case "window_start_drag":
      return null;
    case "window_toggle_maximize":
      return true;
    default:
      return fail(`the fake bridge doesn't answer ${cmd}`);
  }
}
