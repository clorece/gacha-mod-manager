// SPDX-License-Identifier: MIT OR Apache-2.0
// "Launch with XXMI": enabled only when Ready. The first time (or after the exe moved) the
// user confirms the exact launcher path before GMM runs it (security rule R7).

import { useState } from "react";
import { api, asCmdError, type GameInfo, type GameStatus, type LaunchInfo } from "../api";

export function LaunchButton({ game, status }: { game: GameInfo; status: GameStatus }) {
  const [confirming, setConfirming] = useState<LaunchInfo | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const ready = status.state === "ready";

  const start = async (confirm: boolean) => {
    setConfirming(null);
    try {
      await api.launch(game.id, confirm);
      setMessage(`Started XXMI; it starts ${game.name} with mods.`);
    } catch (e) {
      setMessage(asCmdError(e).message);
    }
  };

  const onClick = async () => {
    setMessage(null);
    try {
      const plan = await api.launchPlan(game.id);
      if (plan.confirmed) {
        await start(false);
      } else {
        setConfirming(plan);
      }
    } catch (e) {
      setMessage(asCmdError(e).message);
    }
  };

  return (
    <div className="flex items-center gap-3">
      {message && <span className="text-sm text-zinc-400">{message}</span>}
      <button
        type="button"
        disabled={!ready}
        onClick={onClick}
        title="Mods only load when the game is started through XXMI."
        className="btn btn-primary"
      >
        Launch with XXMI
      </button>
      {confirming && (
        <div role="dialog" aria-modal="true" aria-labelledby="launch-title" className="modal-backdrop">
          <div className="modal">
            <h2 id="launch-title" className="font-semibold">
              Run XXMI Launcher for the first time?
            </h2>
            <p className="mt-2 text-sm text-zinc-300">
              GMM is about to run this program. It's XXMI Launcher's own exe; it isn't code-signed, so please check the
              path. Windows may ask for administrator rights.
            </p>
            <code className="mt-3 block break-all rounded bg-surface p-2 text-xs">{confirming.exe}</code>
            <div className="mt-4 flex justify-end gap-2">
              <button type="button" className="btn" onClick={() => setConfirming(null)}>
                Cancel
              </button>
              <button type="button" className="btn btn-primary" onClick={() => start(true)}>
                Run it
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
