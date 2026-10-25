// SPDX-License-Identifier: MIT OR Apache-2.0
// Typed wrappers for GMM's Tauri commands (apps/desktop/src-tauri/src/commands.rs). The
// webview only ever passes ids; it never names a path or a URL (docs/04-security.md, R5).
// The types come from bindings.ts, which is generated from the Rust types.

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  CardStats,
  CatalogPage,
  CatalogQuery,
  Category,
  ClearedCache,
  CmdError,
  ConflictReport,
  FeaturedMod,
  GameInfo,
  GameStatus,
  InstallArgs,
  InstallPlan,
  InstallProgress,
  InstallResult,
  Job,
  Launched,
  LaunchInfo,
  LibraryView,
  ModKey,
  ModProfile,
  ModUpdate,
  Page,
  Settings,
  StartupReport,
  StartupStep,
  StorageView,
  Toggled,
  ToggleOutcome,
  Uninstalled,
  UninstallItem,
} from "./bindings";

export type * from "./bindings";

/** Commands reject with a `CmdError`; anything else (a bridge failure) becomes `other`. */
export function asCmdError(e: unknown): CmdError {
  if (typeof e === "object" && e !== null && "kind" in e && "message" in e) {
    return e as CmdError;
  }
  return { kind: "other", message: String(e) };
}

export const api = {
  games: () => invoke<GameInfo[]>("games"),
  statuses: () => invoke<GameStatus[]>("statuses"),

  catalogList: (query: CatalogQuery) => invoke<CatalogPage>("catalog_list", { query }),
  catalogSearch: (game: string, text: string, page: number, showNsfw: boolean) =>
    invoke<CatalogPage>("catalog_search", { game, text, page, showNsfw }),
  catalogMod: (modId: number) => invoke<ModProfile>("catalog_mod", { modId }),
  catalogUpdates: (modId: number, page: number) => invoke<Page<ModUpdate>>("catalog_updates", { modId, page }),
  catalogCategories: (game: string, parent: number | null) =>
    invoke<Category[]>("catalog_categories", { game, parent }),
  /** Trending mods for the featured gallery (safe-for-work first unless `showNsfw`). */
  catalogFeatured: (game: string, showNsfw: boolean) => invoke<FeaturedMod[]>("catalog_featured", { game, showNsfw }),
  /** Download counts for cards, sent to `onStats` one mod at a time (at most 60 ids). */
  catalogStats: (modIds: number[], onStats: (s: CardStats) => void) => {
    const channel = new Channel<CardStats>();
    channel.onmessage = onStats;
    return invoke<null>("catalog_stats", { modIds, onStats: channel });
  },

  /** Fetches the file into the cache and looks inside, for the install confirmation. */
  prepareInstall: (modId: number, fileId: number | null, onProgress: (p: InstallProgress) => void) => {
    const channel = new Channel<InstallProgress>();
    channel.onmessage = onProgress;
    return invoke<InstallPlan>("prepare_install", { modId, fileId, onProgress: channel });
  },

  /** The confirmed install goes to the queue (the Downloads page). */
  queueInstall: (args: InstallArgs) => invoke<number>("queue_install", { args }),
  queueJobs: () => invoke<Job[]>("queue_jobs"),
  /** Calls `onChange` with the whole list now and whenever it changes. */
  queueWatch: (onChange: (jobs: Job[]) => void) => {
    const channel = new Channel<Job[]>();
    channel.onmessage = onChange;
    return invoke<null>("queue_watch", { onChange: channel });
  },
  queuePause: (id: number) => invoke<boolean>("queue_pause", { id }),
  queueResume: (id: number) => invoke<boolean>("queue_resume", { id }),
  queueCancel: (id: number) => invoke<boolean>("queue_cancel", { id }),
  queueDismiss: (id: number) => invoke<boolean>("queue_dismiss", { id }),
  queueClearFinished: () => invoke<number>("queue_clear_finished"),

  startupCheck: (onStep: (s: StartupStep) => void) => {
    const channel = new Channel<StartupStep>();
    channel.onmessage = onStep;
    return invoke<StartupReport>("startup_check", { onStep: channel });
  },

  /** Installs at once, outside the queue (kept for the CLI-like flow). */
  install: (args: InstallArgs, onProgress: (p: InstallProgress) => void) => {
    const channel = new Channel<InstallProgress>();
    channel.onmessage = onProgress;
    return invoke<InstallResult>("install", { args, onProgress: channel });
  },

  library: (game: string) => invoke<LibraryView>("library", { game }),
  setEnabled: (game: string, key: ModKey, enabled: boolean, anyway = false) =>
    invoke<Toggled>("set_enabled", { game, key, enabled, anyway }),
  switchVersion: (game: string, id: number) => invoke<ToggleOutcome[]>("switch_version", { game, id }),
  uninstallPreview: (game: string, keys: ModKey[]) => invoke<UninstallItem[]>("uninstall_preview", { game, keys }),
  uninstall: (game: string, keys: ModKey[]) => invoke<Uninstalled[]>("uninstall", { game, keys }),
  conflicts: (game: string) => invoke<ConflictReport>("conflicts", { game }),

  storage: () => invoke<StorageView>("storage"),
  clearDownloadCache: () => invoke<ClearedCache>("clear_download_cache"),
  clearHttpCache: () => invoke<number>("clear_http_cache"),

  /** Each asks in a native dialog first; resolves false when the user cancels. */
  openModPage: (modId: number) => invoke<boolean>("open_mod_page", { modId }),
  openXxmiReleases: () => invoke<boolean>("open_xxmi_releases"),
  /** A link from rewritten description HTML (`#gmm-link:<key>`). */
  openLink: (key: string) => invoke<boolean>("open_link", { key }),
  openModFolder: (game: string, key: ModKey) => invoke<null>("open_mod_folder", { game, key }),

  launchPlan: (game: string) => invoke<LaunchInfo>("launch_plan", { game }),
  launch: (game: string, confirm: boolean) => invoke<Launched>("launch", { game, confirm }),

  settings: () => invoke<Settings>("settings"),
  pickXxmiFolder: () => invoke<string | null>("pick_xxmi_folder"),
  clearXxmiFolder: () => invoke<null>("clear_xxmi_folder"),
  pickGameFolder: (game: string) => invoke<string | null>("pick_game_folder", { game }),
  removeGameFolder: (game: string, folder: string) => invoke<null>("remove_game_folder", { game, folder }),
};
