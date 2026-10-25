// SPDX-License-Identifier: MIT OR Apache-2.0
// The app shell: game rail, top bar with the XXMI state pill and "Launch with XXMI", and the
// page for the selected game. Screens beyond setup and the library come next (docs/07).

import { useCallback, useEffect, useState } from "react";
import { api, asCmdError, type GameInfo, type GameStatus } from "./api";
import logo from "./assets/gmm-logo.svg";
import { LaunchButton } from "./components/LaunchButton";
import { Library } from "./components/Library";
import { SetupCard } from "./components/SetupCard";
import { StatePill } from "./components/StatePill";

export default function App() {
  const [games, setGames] = useState<GameInfo[]>([]);
  const [statuses, setStatuses] = useState<GameStatus[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [g, s] = await Promise.all([api.games(), api.statuses()]);
      setGames(g);
      setStatuses(s);
      setSelected((current) => current ?? g[0]?.id ?? null);
      setError(null);
    } catch (e) {
      setError(asCmdError(e).message);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const game = games.find((g) => g.id === selected);
  const status = statuses.find((s) => s.game_id === selected);

  return (
    <div className="flex h-full">
      <nav aria-label="Games" className="flex w-20 flex-col items-center gap-2 border-line border-r bg-panel py-4">
        <img src={logo} alt="Gacha Mod Manager" className="mb-2 h-12 w-12" />
        {games.map((g) => {
          const s = statuses.find((x) => x.game_id === g.id);
          const active = g.id === selected;
          return (
            <button
              key={g.id}
              type="button"
              onClick={() => setSelected(g.id)}
              aria-current={active ? "page" : undefined}
              title={g.name}
              className={`relative flex h-12 w-12 items-center justify-center rounded-xl font-semibold text-sm ${
                active ? "bg-accent text-white" : "bg-surface text-zinc-300 hover:bg-line"
              } ${s && s.state !== "ready" ? "opacity-70" : ""}`}
            >
              {g.short_name}
              {s && s.state !== "ready" && (
                <>
                  <span className="-right-1 -top-1 absolute h-3 w-3 rounded-full bg-warn" aria-hidden="true" />
                  <span className="sr-only">(needs setup)</span>
                </>
              )}
            </button>
          );
        })}
      </nav>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center gap-3 border-line border-b px-6 py-3">
          <h1 className="font-semibold text-lg">{game?.name ?? "Gacha Mod Manager"}</h1>
          {status && <StatePill state={status.state} />}
          <div className="flex-1" />
          {game && status && <LaunchButton game={game} status={status} />}
        </header>

        <div className="flex-1 space-y-6 overflow-y-auto p-6">
          {error && (
            <p role="alert" className="rounded-lg border border-bad/50 bg-bad/10 p-3 text-sm">
              {error}
            </p>
          )}
          {status && status.state !== "ready" && <SetupCard status={status} onRecheck={refresh} />}
          {game && <Library key={game.id} game={game} ready={status?.state === "ready"} />}
        </div>
      </main>
    </div>
  );
}
