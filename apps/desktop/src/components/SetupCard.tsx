// SPDX-License-Identifier: MIT OR Apache-2.0
// One game's setup state with the steps to fix it, from gmm-xxmi (docs/07, "Setup &
// onboarding"). The guided stepper with screenshots comes later.

import { useState } from "react";
import { api, asCmdError, type GameStatus } from "../api";

const sourceLabel: Record<string, string> = {
  steam: "Steam",
  ho_yo_play: "HoYoPlay",
  epic: "Epic Games",
  manual: "added by you",
};

export function SetupCard({ status, onRecheck }: { status: GameStatus; onRecheck: () => Promise<void> }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      await onRecheck();
    } catch (e) {
      setError(asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const install = status.detected_installs[0];
  return (
    <section aria-labelledby="setup-title" className="rounded-xl border border-line bg-panel p-5">
      <h2 id="setup-title" className="font-semibold">
        {status.game_name}: {status.summary}
      </h2>
      <dl className="mt-3 grid grid-cols-[6rem_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="text-zinc-400">Game</dt>
        <dd>{install ? `Found (${sourceLabel[install.source] ?? install.source}) ${install.folder}` : "Not found"}</dd>
        <dt className="text-zinc-400">XXMI</dt>
        <dd>
          {status.xxmi_root
            ? `Found ${status.xxmi_root}${status.xxmi_version ? ` (v${status.xxmi_version})` : ""}`
            : "Not found"}
        </dd>
        <dt className="text-zinc-400">{status.importer}</dt>
        <dd>
          {status.importer_info?.folder_exists
            ? `Installed${status.importer_info.importer_version ? ` (${status.importer_info.importer_version})` : ""}`
            : "Not installed"}
        </dd>
      </dl>
      {status.steps.length > 0 && (
        <>
          <h3 className="mt-4 font-medium text-sm">Next steps</h3>
          <ol className="mt-1 list-decimal space-y-1 pl-5 text-sm">
            {status.steps.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
        </>
      )}
      {error && (
        <p role="alert" className="mt-3 text-bad text-sm">
          {error}
        </p>
      )}
      <div className="mt-4 flex flex-wrap gap-2">
        <button type="button" disabled={busy} onClick={() => run(async () => {})} className="btn">
          Re-check
        </button>
        {!status.xxmi_root && (
          <button type="button" disabled={busy} onClick={() => run(api.pickXxmiFolder)} className="btn">
            Choose XXMI folder…
          </button>
        )}
        {!install && (
          <button
            type="button"
            disabled={busy}
            onClick={() => run(() => api.pickGameFolder(status.game_id))}
            className="btn"
          >
            Choose game folder…
          </button>
        )}
      </div>
    </section>
  );
}
