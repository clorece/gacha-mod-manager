// SPDX-License-Identifier: MIT OR Apache-2.0
// The storage breakdown (docs/07, "Storage"), opened from Settings › Storage: per game, a bar
// of the biggest mods (the top eight, the rest folded into "Other") with a legend, and a
// sortable table with multi-select to switch mods off or delete them (confirmed, R8). "All
// games" shows one bar per game: GMM's mods, mods added by hand, other files. Decided for
// Phase 5: no "Last used" column and no drive line (docs/05-roadmap.md).

import { useCallback, useEffect, useMemo, useState } from "react";
import { api, asCmdError, type ModKey, type StorageView, type UninstallItem } from "../api";
import { UninstallDialog } from "../components/UninstallDialog";
import type { AppData } from "../data";
import { bytes, plural } from "../format";
import { ALL } from "../games";
import { Button, Checkbox, Segmented, Spinner, useToast } from "../ui";

/** Categorical slots in fixed order (validated on both themes' card colors; styles.css). */
const SLOTS = 8;

type Row = {
  id: string;
  key: ModKey;
  name: string;
  character: string | null;
  bytes: number;
  truncated: boolean;
  enabled: boolean;
  hand: boolean;
};

type Segment = { label: string; bytes: number; slot: number | "other" };
type SortKey = "name" | "character" | "bytes" | "enabled";

function rowsFor(view: StorageView, data: AppData, game: string): Row[] {
  const storage = view.games.find((g) => g.game === game)?.storage;
  if (!storage) return [];
  const lib = data.libraries[game];
  return [...storage.managed, ...storage.unmanaged].map((u) => {
    const ref = u.mod_ref;
    const managed = ref.type === "managed" ? lib?.managed.find((m) => m.id === ref.id) : undefined;
    const name =
      ref.type === "managed"
        ? (managed?.name ?? ref.folder.replace(/^DISABLED /, ""))
        : (lib?.unmanaged.find((x) => x.path === ref.path)?.name ?? ref.path);
    return {
      id: ref.type === "managed" ? `m${ref.id}` : `u${ref.path}`,
      key: ref.type === "managed" ? { type: "managed", id: ref.id } : { type: "unmanaged", path: ref.path },
      name: managed?.variant ? `${name} · ${managed.variant}` : name,
      character: managed?.category ?? null,
      bytes: u.size.bytes,
      truncated: u.size.truncated,
      enabled: u.enabled,
      hand: ref.type === "unmanaged",
    };
  });
}

/** The eight biggest mods, then "Other" (a ninth hue is never generated). */
function segmentsFor(rows: Row[]): Segment[] {
  const sorted = [...rows].sort((a, b) => b.bytes - a.bytes);
  const top = sorted.slice(0, SLOTS).map<Segment>((r, i) => ({ label: r.name, bytes: r.bytes, slot: i + 1 }));
  const rest = sorted.slice(SLOTS).reduce((n, r) => n + r.bytes, 0);
  return rest > 0 ? [...top, { label: `Other (${sorted.length - SLOTS})`, bytes: rest, slot: "other" }] : top;
}

function SegmentBar({ segments, total, label }: { segments: Segment[]; total: number; label: string }) {
  const shown = segments.filter((s) => s.bytes > 0);
  return (
    <div className="sbar">
      <div className="sbar-track" role="img" aria-label={label}>
        {shown.map((s) => (
          <span
            key={s.label}
            className={`sbar-seg sbar-${s.slot}`}
            style={{ flexGrow: total > 0 ? s.bytes / total : 0 }}
            title={`${s.label}: ${bytes(s.bytes)} · ${total > 0 ? Math.round((s.bytes / total) * 100) : 0}%`}
          />
        ))}
      </div>
      <ul className="sbar-legend">
        {shown.map((s) => (
          <li key={s.label}>
            <span className={`sbar-key sbar-${s.slot}`} aria-hidden="true" />
            <span className="truncate">{s.label}</span>
            <b>{bytes(s.bytes)}</b>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function StorageBreakdown({ data, onBack }: { data: AppData; onBack: () => void }) {
  const toast = useToast();
  const [view, setView] = useState<StorageView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [game, setGame] = useState<string>(ALL);
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({ key: "bytes", desc: true });
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [deleting, setDeleting] = useState<UninstallItem[] | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setView(await api.storage());
      setError(null);
    } catch (e) {
      setError(asCmdError(e).message);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: a new game starts with nothing ticked
  useEffect(() => setSelected(new Set()), [game]);

  const rows = useMemo(() => (view && game !== ALL ? rowsFor(view, data, game) : []), [view, data, game]);
  const sorted = useMemo(() => {
    const out = [...rows].sort((a, b) => {
      const v =
        sort.key === "bytes"
          ? a.bytes - b.bytes
          : sort.key === "enabled"
            ? Number(a.enabled) - Number(b.enabled)
            : (a[sort.key] ?? "").localeCompare(b[sort.key] ?? "");
      return v === 0 ? a.name.localeCompare(b.name) : v;
    });
    return sort.desc ? out.reverse() : out;
  }, [rows, sort]);
  const chosen = rows.filter((r) => selected.has(r.id));
  const chosenBytes = chosen.reduce((n, r) => n + r.bytes, 0);

  const header = (key: SortKey, label: string, className?: string) => (
    <th className={className} aria-sort={sort.key === key ? (sort.desc ? "descending" : "ascending") : "none"}>
      <button
        type="button"
        className="th-sort"
        onClick={() => setSort((s) => ({ key, desc: s.key === key ? !s.desc : key === "bytes" }))}
      >
        {label}
        {sort.key === key ? (sort.desc ? " ↓" : " ↑") : ""}
      </button>
    </th>
  );

  const disable = async () => {
    setBusy(true);
    for (const r of chosen.filter((c) => c.enabled)) {
      try {
        await api.setEnabled(game, r.key, false);
      } catch (e) {
        toast("bad", `${r.name}: ${asCmdError(e).message}`);
      }
    }
    await data.refreshLibrary(game);
    await load();
    setBusy(false);
  };

  const askDelete = async () => {
    try {
      setDeleting(
        await api.uninstallPreview(
          game,
          chosen.map((r) => r.key),
        ),
      );
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  };

  const gameName = (id: string) => data.games.find((g) => g.id === id)?.name ?? id;

  return (
    <>
      <button type="button" className="back-link" onClick={onBack}>
        ← Settings
      </button>
      <div className="page-head">
        <div>
          <h1>Storage</h1>
          <p>Space your mods take, per game and per mod. Deleting is for good, after a confirmation.</p>
        </div>
      </div>
      <div className="toolbar">
        <Segmented
          label="Game"
          value={game}
          onChange={setGame}
          options={[{ value: ALL, label: "All games" }, ...data.games.map((g) => ({ value: g.id, label: g.name }))]}
        />
      </div>

      {error ? (
        <div className="card empty">
          Couldn't measure: {error}
          <div>
            <Button variant="link" onClick={() => void load()}>
              Try again
            </Button>
          </div>
        </div>
      ) : !view ? (
        <div className="card empty">
          <Spinner /> Measuring your Mods folders…
        </div>
      ) : game === ALL ? (
        <div className="storage-games">
          {view.games.map((g) => {
            const s = g.storage;
            const total = s ? s.managed_total.bytes + s.unmanaged_total.bytes + s.other.bytes : 0;
            return (
              <section className="card panel" key={g.game}>
                <div className="set-row">
                  <h2 className="card-title grow">{gameName(g.game)}</h2>
                  {s && <b>{bytes(total)}</b>}
                  {s && (
                    <Button size="sm" onClick={() => setGame(g.game)}>
                      Per mod
                    </Button>
                  )}
                </div>
                {s ? (
                  <SegmentBar
                    label={`${gameName(g.game)}: ${bytes(total)} in its Mods folder`}
                    total={total}
                    segments={[
                      { label: `GMM's mods (${s.managed.length})`, bytes: s.managed_total.bytes, slot: 1 },
                      { label: `Added by hand (${s.unmanaged.length})`, bytes: s.unmanaged_total.bytes, slot: 2 },
                      { label: "Other files", bytes: s.other.bytes, slot: 3 },
                    ]}
                  />
                ) : (
                  <p className="sub">{g.problem ?? "Not measured."}</p>
                )}
              </section>
            );
          })}
          <section className="card panel">
            <h2 className="card-title">GMM's cache</h2>
            <SegmentBar
              label="GMM's cache"
              total={view.cache.downloads.bytes + view.cache.images.bytes + view.cache.http_bytes}
              segments={[
                { label: "Downloaded archives", bytes: view.cache.downloads.bytes, slot: 1 },
                { label: "Preview images", bytes: view.cache.images.bytes, slot: 2 },
                { label: "GameBanana answers", bytes: view.cache.http_bytes, slot: 3 },
              ]}
            />
            <p className="sub">Clear these in Settings › Storage.</p>
          </section>
        </div>
      ) : rows.length === 0 ? (
        <div className="card empty">
          {view.games.find((g) => g.game === game)?.problem ?? "No mods in this game's Mods folder."}
        </div>
      ) : (
        <>
          <section className="card panel">
            <SegmentBar
              label={`The biggest mods of ${gameName(game)}`}
              total={rows.reduce((n, r) => n + r.bytes, 0)}
              segments={segmentsFor(rows)}
            />
          </section>
          <div className="selbar storage-bar">
            <b>{chosen.length > 0 ? `${plural(chosen.length, "mod")} · ${bytes(chosenBytes)}` : "Nothing selected"}</b>
            <Button variant="link" onClick={() => setSelected(new Set(rows.map((r) => r.id)))}>
              Select all
            </Button>
            <Button
              variant="link"
              onClick={() => setSelected(new Set(rows.filter((r) => !r.enabled).map((r) => r.id)))}
            >
              Select switched off
            </Button>
            <Button variant="link" disabled={chosen.length === 0} onClick={() => setSelected(new Set())}>
              Clear
            </Button>
            <span className="spacer" />
            <Button disabled={busy || !chosen.some((r) => r.enabled)} onClick={disable}>
              Switch off
            </Button>
            <Button variant="danger" icon="trash" disabled={busy || chosen.length === 0} onClick={askDelete}>
              Delete…
            </Button>
          </div>
          <div className="card table-card">
            <table className="storage-table">
              <thead>
                <tr>
                  <th className="col-check">
                    <span className="sr-only">Selected</span>
                  </th>
                  {header("name", "Mod")}
                  {header("character", "Character")}
                  {header("bytes", "Size", "num")}
                  {header("enabled", "State")}
                </tr>
              </thead>
              <tbody>
                {sorted.map((r) => (
                  <tr key={r.id} className={selected.has(r.id) ? "is-selected" : undefined}>
                    <td className="col-check">
                      <Checkbox
                        checked={selected.has(r.id)}
                        onChange={(on) =>
                          setSelected((all) => {
                            const next = new Set(all);
                            if (on) next.add(r.id);
                            else next.delete(r.id);
                            return next;
                          })
                        }
                      >
                        <span className="sr-only">Select {r.name}</span>
                      </Checkbox>
                    </td>
                    <td>
                      <b>{r.name}</b>
                      {r.hand && <span className="sub"> · added by hand</span>}
                    </td>
                    <td>{r.character ?? <span className="sub">—</span>}</td>
                    <td className="num">
                      {r.truncated ? "≥ " : ""}
                      {bytes(r.bytes)}
                    </td>
                    <td>
                      <span className={`switch-label ${r.enabled ? "state-on" : "state-off"}`}>
                        {r.enabled ? "On" : "Off"}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {deleting && (
        <UninstallDialog
          batches={[{ game, items: deleting }]}
          onClose={() => setDeleting(null)}
          onDone={async () => {
            setDeleting(null);
            setSelected(new Set());
            await data.refreshLibrary(game);
            await load();
          }}
        />
      )}
    </>
  );
}
