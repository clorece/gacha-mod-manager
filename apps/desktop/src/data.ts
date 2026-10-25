// SPDX-License-Identifier: MIT OR Apache-2.0
// What the shell loads once and shares: the games, their setup states and each game's
// library (the sidebar, Dashboard and Library all count mods from it).

import { useCallback, useEffect, useState } from "react";
import { api, asCmdError, type GameInfo, type GameStatus, type LibraryView } from "./api";

export type AppData = {
  games: GameInfo[];
  statuses: GameStatus[];
  libraries: Record<string, LibraryView>;
  /** The first load hasn't finished. */
  loading: boolean;
  error: string | null;
  /** Reload everything (after a setup change). */
  refresh: () => Promise<void>;
  /** Reload one game's library (after a toggle or uninstall). */
  refreshLibrary: (game: string) => Promise<void>;
};

export function useAppData(): AppData {
  const [games, setGames] = useState<GameInfo[]>([]);
  const [statuses, setStatuses] = useState<GameStatus[]>([]);
  const [libraries, setLibraries] = useState<Record<string, LibraryView>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [g, s] = await Promise.all([api.games(), api.statuses()]);
      const views = await Promise.all(g.map((game) => api.library(game.id)));
      setGames(g);
      setStatuses(s);
      setLibraries(Object.fromEntries(views.map((v) => [v.game, v])));
      setError(null);
    } catch (e) {
      setError(asCmdError(e).message);
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshLibrary = useCallback(async (game: string) => {
    try {
      const view = await api.library(game);
      setLibraries((all) => ({ ...all, [game]: view }));
    } catch (e) {
      setError(asCmdError(e).message);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { games, statuses, libraries, loading, error, refresh, refreshLibrary };
}

/** Mods in a game's library (GMM's and hand-installed) and how many are on. */
export function modCounts(view: LibraryView | undefined): { total: number; on: number } {
  if (!view) return { total: 0, on: 0 };
  const all = [...view.managed.map((m) => m.enabled), ...view.unmanaged.map((u) => u.enabled)];
  return { total: all.length, on: all.filter(Boolean).length };
}
