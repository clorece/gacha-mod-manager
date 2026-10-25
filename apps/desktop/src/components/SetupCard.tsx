// SPDX-License-Identifier: MIT OR Apache-2.0
// One game's setup state with the steps to fix it, from gmm-xxmi (docs/07, "Setup &
// onboarding"). Shown on the Dashboard instead of Launch for a game that isn't Ready. The
// guided stepper with screenshots comes later.

import { useState } from "react";
import { api, asCmdError, type GameStatus } from "../api";
import { setupLabel, sourceLabel } from "../games";
import { Badge, Button, useToast } from "../ui";

export function SetupCard({ status, onRecheck }: { status: GameStatus; onRecheck: () => Promise<void> }) {
  const toast = useToast();
  const [busy, setBusy] = useState(false);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await action();
      await onRecheck();
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const install = status.detected_installs[0];
  const state = setupLabel(status.state);
  return (
    <section aria-label={`${status.game_name} setup`} className="card panel stack">
      <div className="flex items-center gap-3">
        <h2 className="card-title grow">{status.game_name}</h2>
        <Badge tone={state.tone}>{state.label}</Badge>
      </div>
      <p>{status.summary}</p>
      <div className="kv">
        <div className="kv-row">
          <span>Game</span>
          <b>{install ? `Found through ${sourceLabel(install.source)} · ${install.folder}` : "Not found"}</b>
        </div>
        <div className="kv-row">
          <span>XXMI</span>
          <b>
            {status.xxmi_root
              ? `Found · ${status.xxmi_root}${status.xxmi_version ? ` (v${status.xxmi_version})` : ""}`
              : "Not found"}
          </b>
        </div>
        <div className="kv-row">
          <span>{status.importer}</span>
          <b>
            {status.importer_info?.folder_exists
              ? `Installed${status.importer_info.importer_version ? ` (${status.importer_info.importer_version})` : ""}`
              : "Not installed"}
          </b>
        </div>
      </div>
      {status.steps.length > 0 && (
        <div>
          <h3 className="sec-h">Next steps</h3>
          <ol className="mt-2 list-decimal space-y-1 pl-5 leading-[1.45]">
            {status.steps.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
        </div>
      )}
      <div className="flex flex-wrap gap-2">
        <Button icon="refresh" disabled={busy} onClick={() => run(async () => {})}>
          Check again
        </Button>
        {!status.xxmi_root && (
          <Button icon="folder" disabled={busy} onClick={() => run(api.pickXxmiFolder)}>
            Choose XXMI folder…
          </Button>
        )}
        {!install && (
          <Button icon="folder" disabled={busy} onClick={() => run(() => api.pickGameFolder(status.game_id))}>
            Choose game folder…
          </Button>
        )}
      </div>
    </section>
  );
}
