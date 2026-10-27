// SPDX-License-Identifier: MIT OR Apache-2.0
// Typed wrappers for GMM's Tauri commands (apps/desktop/src-tauri/src/commands.rs). The
// webview only ever passes ids; it never names a path or a URL (docs/security.md, R5).
// The types come from bindings.ts, which is generated from the Rust types.

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AdoptCandidate,
  Adopted,
  AppRelease,
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
  InstalledFolder,
  InstallProgress,
  InstallResult,
  Job,
  KeyBindings,
  Launched,
  LaunchInfo,
  LibraryView,
  LocalPlan,
  LogFile,
  ModKey,
  ModProfile,
  ModUpdate,
  NetworkStatus,
  Page,
  Preferences,
  ProfileApplied,
  ProfileImported,
  ProfileInfo,
  RollbackFile,
  Settings,
  Size,
  StartupReport,
  StartupStep,
  StorageView,
  Toggled,
  ToggleOutcome,
  Uninstalled,
  UninstallItem,
  VariantPick,
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

  /** The confirmed install goes to the queue (the Downloads page). */
  queueInstall: (args: InstallArgs) => invoke<number>("queue_install", { args }),
  /** Goes ahead with an install that needs action (versions chosen, findings accepted). */
  queueProceed: (id: number, variants: VariantPick | null) => invoke<boolean>("queue_proceed", { id, variants }),
  /** Moves an installed mod to its newest file (an update) or to `fileId` (a roll back). */
  queueChangeFile: (gbModId: number, fileId: number | null) => invoke<number>("queue_change_file", { gbModId, fileId }),
  /** Earlier files of an installed mod still in the download cache. */
  rollbackFiles: (game: string, gbModId: number) => invoke<RollbackFile[]>("rollback_files", { game, gbModId }),
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

  /** Whether a newer GMM is on GitHub (GMM's own releases list, nothing downloaded). */
  appUpdateCheck: () => invoke<AppRelease | null>("app_update_check"),
  /** Opens the page of the release the last check found, after a native dialog; false when cancelled. */
  openAppRelease: (version: string) => invoke<boolean>("open_app_release", { version }),

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
  modKeybindings: (game: string, key: ModKey) => invoke<KeyBindings>("mod_keybindings", { game, key }),
  adoptCandidates: (game: string, path: string) => invoke<AdoptCandidate[]>("adopt_candidates", { game, path }),
  adopt: (game: string, path: string, gbModId: number, fileId: number | null) =>
    invoke<Adopted>("adopt", { game, path, gbModId, fileId }),

  profiles: (game: string) => invoke<ProfileInfo[]>("profiles", { game }),
  profileSave: (game: string, name: string) => invoke<ProfileInfo>("profile_save", { game, name }),
  profileApply: (game: string, id: number) => invoke<ProfileApplied>("profile_apply", { game, id }),
  profileRename: (game: string, id: number, name: string) => invoke<null>("profile_rename", { game, id, name }),
  profileDelete: (game: string, id: number) => invoke<null>("profile_delete", { game, id }),
  profileExport: (game: string, id: number) => invoke<string>("profile_export", { game, id }),
  profileImport: (game: string, code: string, name: string | null) =>
    invoke<ProfileImported>("profile_import", { game, code, name }),

  /** Calls `onDrop` with the name of each file dropped on the window (Rust keeps the path). */
  dropWatch: (onDrop: (name: string) => void) => {
    const channel = new Channel<string>();
    channel.onmessage = onDrop;
    return invoke<null>("drop_watch", { onDrop: channel });
  },
  /** A native file dialog for an archive; resolves with its name, or null when cancelled. */
  pickArchive: () => invoke<string | null>("pick_archive"),
  /** Copies the dropped or picked archive into the cache and looks inside, for `game`. */
  prepareLocal: (game: string) => invoke<LocalPlan>("prepare_local", { game }),
  installLocal: (variants: VariantPick, skipBlocked: boolean, startDisabled: boolean) =>
    invoke<InstalledFolder[]>("install_local", { variants, skipBlocked, startDisabled }),
  discardLocal: () => invoke<null>("discard_local"),

  storage: () => invoke<StorageView>("storage"),
  clearDownloadCache: () => invoke<ClearedCache>("clear_download_cache"),
  clearHttpCache: () => invoke<number>("clear_http_cache"),
  /** Deletes every cached preview image; they download again when shown. */
  clearImageCache: () => invoke<Size>("clear_image_cache"),
  /** Whether GameBanana asked GMM to slow down, and whether saved data is showing. Local only. */
  networkStatus: () => invoke<NetworkStatus>("network_status"),

  /** Each asks in a native dialog first; resolves false when the user cancels. */
  openModPage: (modId: number) => invoke<boolean>("open_mod_page", { modId }),
  openXxmiReleases: () => invoke<boolean>("open_xxmi_releases"),
  /** A link from rewritten description HTML (`#gmm-link:<key>`). */
  openLink: (key: string) => invoke<boolean>("open_link", { key }),
  openModFolder: (game: string, key: ModKey) => invoke<null>("open_mod_folder", { game, key }),

  /** The log file a bug report attaches: the one picked, else the newest; null when none. */
  reportLog: () => invoke<LogFile | null>("report_log"),
  /** A file dialog in the log folder; the picked log, or null when cancelled. */
  pickReportLog: () => invoke<LogFile | null>("pick_report_log"),
  /** An unhandled webview error, for the log (cut short in Rust). */
  logUiError: (message: string) => invoke<null>("log_ui_error", { message }),
  /** Asks in a native dialog, opens the issue in the browser, then shows the log folder;
   *  false when cancelled. */
  openBugReport: (title: string, description: string) => invoke<boolean>("open_bug_report", { title, description }),

  launchPlan: (game: string) => invoke<LaunchInfo>("launch_plan", { game }),
  launch: (game: string, confirm: boolean) => invoke<Launched>("launch", { game, confirm }),

  settings: () => invoke<Settings>("settings"),
  preferences: () => invoke<Preferences>("preferences"),
  setPreferences: (preferences: Preferences) => invoke<Preferences>("set_preferences", { preferences }),
  pickXxmiFolder: () => invoke<string | null>("pick_xxmi_folder"),
  clearXxmiFolder: () => invoke<null>("clear_xxmi_folder"),
  pickGameFolder: (game: string) => invoke<string | null>("pick_game_folder", { game }),
  removeGameFolder: (game: string, folder: string) => invoke<null>("remove_game_folder", { game, folder }),

  /** The title bar's buttons and drag area; each acts on this window only. */
  windowMinimize: () => invoke<null>("window_minimize"),
  /** Resolves with whether the window is maximized afterwards. */
  windowToggleMaximize: () => invoke<boolean>("window_toggle_maximize"),
  windowIsMaximized: () => invoke<boolean>("window_is_maximized"),
  windowClose: () => invoke<null>("window_close"),
  windowStartDrag: () => invoke<null>("window_start_drag"),
};
