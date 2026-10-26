// SPDX-License-Identifier: MIT OR Apache-2.0
// Launching a game through XXMI. The first time (or after the exe moved) the user confirms
// the exact launcher path in a dialog before GMM runs it (security rule R7). The Dashboard and
// the Mods Library both launch from here.

import { useState } from "react";
import { api, asCmdError, type GameInfo, type LaunchInfo } from "../api";
import { Button, Dialog, useToast } from "../ui";

/** `launch()` starts the game, asking first when needed; render `dialog` somewhere. */
export function useLaunch(game: GameInfo) {
  const toast = useToast();
  const [confirming, setConfirming] = useState<LaunchInfo | null>(null);
  const [busy, setBusy] = useState(false);

  const start = async (confirm: boolean) => {
    setConfirming(null);
    try {
      await api.launch(game.id, confirm);
      toast("ok", `Started XXMI. It opens ${game.name} with your mods.`);
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  };

  const launch = async () => {
    setBusy(true);
    try {
      const plan = await api.launchPlan(game.id);
      if (plan.confirmed) {
        await start(false);
      } else {
        setConfirming(plan);
      }
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const dialog = confirming && (
    <Dialog
      eyebrow="Launch"
      title="Run XXMI Launcher for the first time?"
      onClose={() => setConfirming(null)}
      footer={
        <>
          <Button onClick={() => setConfirming(null)}>Cancel</Button>
          <Button variant="primary" onClick={() => start(true)}>
            Run it
          </Button>
        </>
      }
    >
      <p>
        GMM is about to run this program to start {game.name}. It's XXMI Launcher's own exe; it isn't code-signed, so
        check the path. Windows may ask for administrator rights.
      </p>
      <p className="hint mono">{confirming.exe}</p>
      <p className="sub">GMM asks again only if this path changes.</p>
    </Dialog>
  );

  return { launch, busy, dialog };
}

/** "Launch" for one game (the Library's header; the Dashboard has its own in each bubble). */
export function LaunchButton({ game, label }: { game: GameInfo; label: string }) {
  const { launch, busy, dialog } = useLaunch(game);
  return (
    <>
      <Button variant="launch" icon="play" disabled={busy} onClick={launch}>
        {label}
      </Button>
      {dialog}
    </>
  );
}
