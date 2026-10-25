// SPDX-License-Identifier: MIT OR Apache-2.0
// Typed wrappers for GMM's Tauri commands (apps/desktop/src-tauri/src/commands.rs). The
// webview only ever passes ids; it never names a path or a URL (docs/04-security.md, R5).
// The types come from bindings.ts, which is generated from the Rust types.

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  CatalogPage,
  CatalogQuery,
  Category,
  ClearedCache,
  CmdError,
  ConflictReport,
  GameInfo,
  GameStatus,
  InstallArgs,
  InstallProgress,
  InstallResult,
  Launched,
  LaunchInfo,
  LibraryView,
  ModKey,
  ModProfile,
  ModUpdate,
  Page,
  Settings,
  StorageView,
  Toggled,
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

  install: (args: InstallArgs, onProgress: (p: InstallProgress) => void) => {
    const channel = new Channel<InstallProgress>();
    channel.onmessage = onProgress;
    return invoke<InstallResult>("install", { args, onProgress: channel });
  },

  library: (game: string) => invoke<LibraryView>("library", { game }),
  setEnabled: (game: string, key: ModKey, enabled: boolean, anyway = false) =>
    invoke<Toggled>("set_enabled", { game, key, enabled, anyway }),
  uninstallPreview: (game: string, keys: ModKey[]) => invoke<UninstallItem[]>("uninstall_preview", { game, keys }),
  uninstall: (game: string, keys: ModKey[]) => invoke<Uninstalled[]>("uninstall", { game, keys }),
  conflicts: (game: string) => invoke<ConflictReport>("conflicts", { game }),

  storage: () => invoke<StorageView>("storage"),
  clearDownloadCache: () => invoke<ClearedCache>("clear_download_cache"),

  launchPlan: (game: string) => invoke<LaunchInfo>("launch_plan", { game }),
  launch: (game: string, confirm: boolean) => invoke<Launched>("launch", { game, confirm }),

  settings: () => invoke<Settings>("settings"),
  pickXxmiFolder: () => invoke<string | null>("pick_xxmi_folder"),
  clearXxmiFolder: () => invoke<null>("clear_xxmi_folder"),
  pickGameFolder: (game: string) => invoke<string | null>("pick_game_folder", { game }),
  removeGameFolder: (game: string, folder: string) => invoke<null>("remove_game_folder", { game, folder }),
};
