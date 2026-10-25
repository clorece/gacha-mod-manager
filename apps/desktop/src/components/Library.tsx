// SPDX-License-Identifier: MIT OR Apache-2.0
// A game's mods: GMM's and hand-installed ones, with on/off toggles. Conflicts on enable are
// shown and need an explicit "Enable anyway". The full library screen (filters, grid,
// actions menu, uninstall) comes later (docs/07, "Library").

import { useCallback, useEffect, useState } from "react";
import { api, asCmdError, type Conflict, type GameInfo, type LibraryView, type ModKey, type ModRef } from "../api";
import { bytes } from "../format";

function label(ref: ModRef): string {
  return ref.type === "managed" ? ref.folder : `${ref.path} (not installed by GMM)`;
}

export function Library({ game, ready }: { game: GameInfo; ready: boolean }) {
  const [view, setView] = useState<LibraryView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<{ key: ModKey; conflicts: Conflict[] } | null>(null);

  const load = useCallback(async () => {
    try {
      setView(await api.library(game.id));
    } catch (e) {
      setError(asCmdError(e).message);
    }
  }, [game.id]);

  useEffect(() => {
    void load();
  }, [load]);

  const toggle = async (key: ModKey, enabled: boolean, anyway = false) => {
    setError(null);
    setPending(null);
    try {
      await api.setEnabled(game.id, key, enabled, anyway);
    } catch (e) {
      const err = asCmdError(e);
      if (err.kind === "conflicts") {
        setPending({ key, conflicts: err.conflicts });
      } else {
        setError(err.message);
      }
    }
    await load();
  };

  if (!view) {
    return <p className="text-sm text-zinc-400">Loading mods…</p>;
  }
  if (view.problem) {
    return <p className="text-sm text-zinc-400">No mods to show: {view.problem}</p>;
  }

  const rows = [
    ...view.managed.map((m) => ({
      key: { type: "managed", id: m.id } as ModKey,
      name: m.name,
      detail: `#${m.id} · ${m.version ? `v${m.version} · ` : ""}${bytes(m.size_bytes)}${
        m.disk === "missing" ? " · folder missing" : m.disk === "ambiguous" ? " · two copies on disk" : ""
      }`,
      enabled: m.enabled,
      usable: m.disk === "enabled" || m.disk === "disabled",
    })),
    ...view.unmanaged.map((u) => ({
      key: { type: "unmanaged", path: u.path } as ModKey,
      name: u.name,
      detail: `${u.path} · not installed by GMM`,
      enabled: u.enabled,
      usable: true,
    })),
  ];

  return (
    <section aria-labelledby="library-title">
      <h2 id="library-title" className="font-semibold">
        Mods ({rows.length})
      </h2>
      {!ready && rows.length > 0 && (
        <p className="mt-1 text-sm text-warn">These mods are installed but XXMI isn't set up for this game yet.</p>
      )}
      {error && (
        <p role="alert" className="mt-2 text-bad text-sm">
          {error}
        </p>
      )}
      {pending && (
        <div role="alert" className="mt-3 rounded-lg border border-warn/50 bg-warn/10 p-3 text-sm">
          <p>Switching this on would conflict with mods that are already on:</p>
          <ul className="mt-1 list-disc pl-5">
            {pending.conflicts.map((c) => (
              <li key={`${label(c.a)}|${label(c.b)}`}>
                {label(c.a)} and {label(c.b)} ({c.hashes.length} shared hashes)
              </li>
            ))}
          </ul>
          <div className="mt-2 flex gap-2">
            <button type="button" className="btn" onClick={() => setPending(null)}>
              Keep it off
            </button>
            <button type="button" className="btn" onClick={() => toggle(pending.key, true, true)}>
              Enable anyway
            </button>
          </div>
        </div>
      )}
      {rows.length === 0 ? (
        <p className="mt-2 text-sm text-zinc-400">No mods yet.</p>
      ) : (
        <ul className="mt-3 divide-y divide-line rounded-xl border border-line bg-panel">
          {rows.map((r) => (
            <li key={JSON.stringify(r.key)} className="flex items-center gap-4 px-4 py-3">
              <input
                type="checkbox"
                role="switch"
                aria-label={`${r.name} ${r.enabled ? "on" : "off"}`}
                checked={r.enabled}
                aria-checked={r.enabled}
                disabled={!r.usable}
                onChange={(e) => toggle(r.key, e.target.checked)}
                className="h-4 w-4 accent-accent"
              />
              <div className="min-w-0">
                <p className="truncate">{r.name}</p>
                <p className="truncate text-xs text-zinc-400">{r.detail}</p>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
