// SPDX-License-Identifier: MIT OR Apache-2.0
// Mods Library: game banner cards, Update all, a sticky toolbar (search, Category,
// Characters, Show sensitive; Profiles, Enable all, Disable all, Select) and a card per mod
// with its stored preview, an On/Off switch and a settings button. GMM's mods and
// hand-installed ones show together; the versions of one GameBanana file share a card.
// Switching on a mod that conflicts with one that's on asks first ("Enable anyway").
// Select mode enables, disables or deletes several at once (deleting is confirmed, R8).

import { useEffect, useMemo, useState } from "react";
import {
  api,
  asCmdError,
  type Conflict,
  type GameInfo,
  type ModKey,
  type ModRef,
  type ModRow,
  type ModUpdateInfo,
  type UninstallItem,
} from "../api";
import type { Owned } from "../catalog";
import { LaunchButton } from "../components/Launch";
import { ModSettings } from "../components/ModSettings";
import { Preview } from "../components/Preview";
import { ProfilesPanel } from "../components/ProfilesPanel";
import { UninstallDialog } from "../components/UninstallDialog";
import { type AppData, modCounts } from "../data";
import { bytes, day, plural } from "../format";
import { ALL, bannerClass, type Scope } from "../games";
import { usePrefs } from "../prefs";
import { Badge, Button, Checkbox, Dropdown, Icon, IconButton, Notice, Switch, ToggleChip, useToast } from "../ui";

/** One installed version of a mod that has several (a row of the Versions section). */
export type LibraryVersion = {
  id: number;
  key: ModKey;
  /** The version's name in the archive, or the folder name. */
  name: string;
  enabled: boolean;
  usable: boolean;
  size: number;
  folder: string;
};

/** One card: a GMM-installed or hand-installed mod in one game. A mod installed as several
 *  versions is one card; its `key` is the current version (the one on, else the first). */
export type LibraryItem = {
  /** Unique across games: `game|...`. */
  id: string;
  game: GameInfo;
  key: ModKey;
  name: string;
  /** At least one version is on. */
  enabled: boolean;
  /** False when the folder is missing or there are two copies: the switch is off-limits. */
  usable: boolean;
  problem: string | null;
  hand: boolean;
  /** "Installed 24 Oct 2026" or "Added by hand". */
  added: string;
  /** GameBanana's upload date of the newest installed file, when known. */
  updated: number | null;
  size: number | null;
  version: string | null;
  folder: string;
  /** A `gmmimg` address stored at install. */
  preview: string | null;
  sensitive: boolean;
  /** GameBanana's root category and subcategory (the character) at install. */
  rootCategory: string | null;
  category: string | null;
  /** GameBanana mod and file ids, for GMM's mods from GameBanana. */
  gbModId: number | null;
  gbFileId: number | null;
  /** A folder (hand-installed loose `.ini` files aren't). */
  isDir: boolean;
  /** Installed versions, when there are several (empty otherwise). */
  versions: LibraryVersion[];
};

/** Every key a card stands for (all versions), for deleting. */
export function itemKeys(item: LibraryItem): ModKey[] {
  return item.versions.length > 0 ? item.versions.map((v) => v.key) : [item.key];
}

function refName(ref: ModRef): string {
  return ref.type === "managed" ? ref.folder.replace(/^DISABLED /, "") : ref.path;
}

const usableRow = (m: ModRow) => m.disk === "enabled" || m.disk === "disabled";

function managedItem(game: GameInfo, rows: ModRow[]): LibraryItem {
  const current = rows.find((r) => r.enabled) ?? rows[0];
  const versions =
    rows.length > 1 || current.group !== null
      ? rows.map<LibraryVersion>((r) => ({
          id: r.id,
          key: { type: "managed", id: r.id },
          name: r.variant ?? r.folder.replace(/^DISABLED /, ""),
          enabled: r.enabled,
          usable: usableRow(r),
          size: r.size_bytes,
          folder: `GMM/${r.folder}`,
        }))
      : [];
  return {
    id: `${game.id}|${current.group ?? current.id}`,
    game,
    key: { type: "managed", id: current.id },
    name: current.name,
    enabled: rows.some((r) => r.enabled),
    usable: rows.some(usableRow),
    problem: current.disk === "missing" ? "Folder missing" : current.disk === "ambiguous" ? "Two copies" : null,
    hand: false,
    added: `Installed ${day(Math.min(...rows.map((r) => r.installed_at)))}`,
    updated: rows.reduce<number | null>(
      (n, r) => (r.file_date !== null && (n === null || r.file_date > n) ? r.file_date : n),
      null,
    ),
    size: rows.reduce((n, r) => n + r.size_bytes, 0),
    version: current.version,
    folder: `GMM/${current.folder}`,
    preview: rows.find((r) => r.preview)?.preview ?? null,
    sensitive: rows.some((r) => r.sensitive),
    rootCategory: current.root_category,
    category: current.category,
    gbModId: current.gb_mod_id,
    gbFileId: current.gb_file_id,
    isDir: true,
    versions,
  };
}

export function libraryItems(data: AppData, games: GameInfo[]): LibraryItem[] {
  return games.flatMap((game) => {
    const view = data.libraries[game.id];
    if (!view) return [];
    // Versions of one GameBanana file share a group; everything else stands alone.
    const groups = new Map<string, ModRow[]>();
    for (const m of view.managed) {
      const k = m.group ?? `id:${m.id}`;
      groups.set(k, [...(groups.get(k) ?? []), m]);
    }
    return [
      ...[...groups.values()].map((rows) => managedItem(game, rows)),
      ...view.unmanaged.map<LibraryItem>((u) => ({
        id: `${game.id}|${u.path}`,
        game,
        key: { type: "unmanaged", path: u.path },
        name: u.name,
        enabled: u.enabled,
        usable: true,
        problem: null,
        hand: true,
        added: "Added by hand",
        updated: null,
        size: null,
        version: null,
        folder: u.path,
        preview: null,
        sensitive: false,
        rootCategory: null,
        category: null,
        gbModId: null,
        gbFileId: null,
        isDir: u.is_dir,
        versions: [],
      })),
    ].sort((a, b) => a.name.localeCompare(b.name, "en", { sensitivity: "base" }));
  });
}

function isSelf(ref: ModRef, key: ModKey): boolean {
  return ref.type === "managed"
    ? key.type === "managed" && ref.id === key.id
    : key.type === "unmanaged" && ref.path === key.path;
}

const keyOf = (game: string, key: ModKey) => `${game}|${key.type === "managed" ? key.id : key.path}`;

/** Category and character choices from what's installed (GameBanana's names at install). */
function choices(items: LibraryItem[], pick: (i: LibraryItem) => string | null): string[] {
  return [...new Set(items.map(pick).filter((v): v is string => v !== null))].sort((a, b) => a.localeCompare(b));
}

export function Library({
  data,
  scope,
  onScope,
  updates,
  owned,
  focus,
  onFocusDone,
  onOpenMod,
  onAddVersions,
}: {
  data: AppData;
  scope: Scope;
  onScope: (s: Scope) => void;
  /** Newer files found by the startup check. */
  updates: ModUpdateInfo[];
  /** Installed GameBanana mods with their conflict state. */
  owned: Map<number, Owned>;
  /** A mod whose settings to open (asked for from the Store or a mod page). */
  focus: { game: string; key: ModKey } | null;
  onFocusDone: () => void;
  /** GMM's mod page for a GameBanana mod. */
  onOpenMod: (gbModId: number) => void;
  /** The install confirmation for more versions of an installed file (from the cache). */
  onAddVersions: (gbModId: number, gbFileId: number | null) => void;
}) {
  const toast = useToast();
  const { prefs, update: setPrefs } = usePrefs();
  const [query, setQuery] = useState("");
  const [rootCategory, setRootCategory] = useState("");
  const [characters, setCharacters] = useState<string[]>([]);
  const [pending, setPending] = useState<{ item: LibraryItem; conflicts: Conflict[] } | null>(null);
  const [blocked, setBlocked] = useState<string[]>([]);
  const [settingsFor, setSettingsFor] = useState<string | null>(null);
  const [profilesOpen, setProfilesOpen] = useState(false);
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [deleting, setDeleting] = useState<{ game: string; items: UninstallItem[] }[] | null>(null);
  const [busy, setBusy] = useState(false);

  // Filters and the selection belong to one scope.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset when the scope changes
  useEffect(() => {
    setRootCategory("");
    setCharacters([]);
    setSelected(new Set());
  }, [scope]);

  const inScope = useMemo(
    () => (scope === ALL ? data.games : data.games.filter((g) => g.id === scope)),
    [scope, data.games],
  );
  const items = useMemo(() => libraryItems(data, inScope), [data, inScope]);

  useEffect(() => {
    if (!focus) return;
    const id = keyOf(focus.game, focus.key);
    const item = items.find((i) => itemKeys(i).some((k) => keyOf(i.game.id, k) === id));
    if (item) setSettingsFor(item.id);
    onFocusDone();
  }, [focus, items, onFocusDone]);

  const show = prefs.show_sensitive;
  const q = query.trim().toLowerCase();
  const visible = items.filter((i) => show || !i.sensitive);
  const hiddenSensitive = items.length - visible.length;
  const shown = visible.filter(
    (i) =>
      (q === "" || i.name.toLowerCase().includes(q) || (i.category ?? "").toLowerCase().includes(q)) &&
      (rootCategory === "" || i.rootCategory === rootCategory) &&
      (characters.length === 0 || (i.category !== null && characters.includes(i.category))),
  );
  const categoryOptions = choices(visible, (i) => i.rootCategory);
  const characterOptions = choices(
    visible.filter((i) => rootCategory === "" || i.rootCategory === rootCategory),
    (i) => i.category,
  );
  const filtered = q !== "" || rootCategory !== "" || characters.length > 0;

  const problems = inScope.flatMap((g) => {
    const p = data.libraries[g.id]?.problem;
    return p ? [{ game: g, problem: p }] : [];
  });
  const notReady = inScope.filter((g) => {
    const s = data.statuses.find((st) => st.game_id === g.id);
    return s && s.state !== "ready" && !data.libraries[g.id]?.problem && modCounts(data.libraries[g.id]).total > 0;
  });
  /** Games in scope that XXMI is set up for: the header's Launch buttons. */
  const launchable = inScope.filter((g) => data.statuses.find((st) => st.game_id === g.id)?.state === "ready");
  const scopedUpdates = updates.filter((u) => inScope.some((g) => g.id === u.game));
  const updateFor = (item: LibraryItem) =>
    item.gbModId === null ? undefined : scopedUpdates.find((u) => u.gb_mod_id === item.gbModId);

  /** Returns the conflicts when switching on was refused because of them. */
  const setEnabled = async (game: string, key: ModKey, enabled: boolean, anyway = false) => {
    try {
      await api.setEnabled(game, key, enabled, anyway);
      return null;
    } catch (e) {
      const err = asCmdError(e);
      if (err.kind === "conflicts") return err.conflicts;
      toast("bad", err.message);
      return null;
    }
  };

  /** A card's switch: on switches its current version on; off switches every version off. */
  const switchItem = async (item: LibraryItem, enabled: boolean, anyway = false): Promise<Conflict[] | null> => {
    if (enabled) return setEnabled(item.game.id, item.key, true, anyway);
    const on = item.versions.length > 0 ? item.versions.filter((v) => v.enabled).map((v) => v.key) : [item.key];
    for (const key of on) await setEnabled(item.game.id, key, false);
    return null;
  };

  const toggle = async (item: LibraryItem, enabled: boolean, anyway = false) => {
    setPending(null);
    const conflicts = await switchItem(item, enabled, anyway);
    if (conflicts) setPending({ item, conflicts });
    await data.refreshLibrary(item.game.id);
  };

  /** Enable all applies the same conflict rule: a mod that would clash stays off. */
  const setMany = async (targets: LibraryItem[], enabled: boolean) => {
    setPending(null);
    setBusy(true);
    const stayedOff: string[] = [];
    for (const item of targets) {
      if (item.enabled === enabled || !item.usable) continue;
      if (await switchItem(item, enabled)) stayedOff.push(item.name);
    }
    setBlocked(stayedOff);
    await Promise.all(inScope.map((g) => data.refreshLibrary(g.id)));
    setBusy(false);
  };

  const chosen = shown.filter((i) => selected.has(i.id));
  const pick = (item: LibraryItem, on: boolean) =>
    setSelected((all) => {
      const next = new Set(all);
      if (on) next.add(item.id);
      else next.delete(item.id);
      return next;
    });
  const stopSelecting = () => {
    setSelecting(false);
    setSelected(new Set());
  };

  const askDelete = async () => {
    const games = [...new Set(chosen.map((i) => i.game.id))];
    try {
      const batches = await Promise.all(
        games.map(async (game) => ({
          game,
          items: await api.uninstallPreview(game, chosen.filter((i) => i.game.id === game).flatMap(itemKeys)),
        })),
      );
      setDeleting(batches);
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  };

  /** Queues every update the startup check found, one at a time through the queue. */
  const updateAll = async () => {
    setBusy(true);
    let queued = 0;
    for (const u of scopedUpdates) {
      try {
        await api.queueChangeFile(u.gb_mod_id, u.newest.id);
        queued += 1;
      } catch (e) {
        toast("bad", `${u.name}: ${asCmdError(e).message}`);
      }
    }
    setBusy(false);
    if (queued > 0) {
      toast(
        "ok",
        `${plural(queued, "update")} added to Downloads. Each version keeps its folder and its on/off state.`,
      );
    }
  };

  const others = pending?.conflicts.map((c) => refName(isSelf(c.a, pending.item.key) ? c.b : c.a));
  const shared = pending ? Math.max(...pending.conflicts.map((c) => c.hashes.length)) : 0;
  const settingsItem = items.find((i) => i.id === settingsFor);

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Mods Library</h1>
          <p>Your installed mods. Switch one off and XXMI skips it next time the game starts.</p>
        </div>
        <div className="chips">
          {scopedUpdates.length > 0 && (
            <Button variant="primary" icon="download" disabled={busy} onClick={updateAll}>
              Update all ({scopedUpdates.length})
            </Button>
          )}
          {launchable.map((g) => (
            <LaunchButton key={g.id} game={g} label={launchable.length > 1 ? `Launch ${g.short_name}` : "Launch"} />
          ))}
        </div>
      </div>

      <div className="lib-games">
        {[{ id: ALL, name: "All games", short_name: "ALL" }, ...data.games].map((g) => {
          const counts =
            g.id === ALL
              ? data.games
                  .map((x) => modCounts(data.libraries[x.id]))
                  .reduce((a, b) => ({ total: a.total + b.total, on: a.on + b.on }), { total: 0, on: 0 })
              : modCounts(data.libraries[g.id]);
          return (
            <button
              key={g.id}
              type="button"
              className={`${bannerClass(g.id)} lib-game`}
              data-mark={g.short_name}
              aria-current={g.id === scope ? "true" : undefined}
              onClick={() => onScope(g.id)}
            >
              <span className="fade" />
              <span className="banner-title">{g.name}</span>
              <span className="lib-sub">
                {plural(counts.total, "mod")} · {counts.on} on
              </span>
            </button>
          );
        })}
      </div>

      <div className="notices">
        {problems.map((p) => (
          <Notice key={p.game.id} tone="warn" title={`${p.game.name}: no mods to show.`}>
            {p.problem}
          </Notice>
        ))}
        {notReady.map((g) => (
          <Notice key={g.id} tone="info" title={`${g.name}: XXMI isn't set up for this game yet.`}>
            These mods are installed, but they won't load until it is. The Dashboard shows what to do.
          </Notice>
        ))}
        {pending && others && (
          <Notice
            tone="warn"
            title={`${pending.item.name} conflicts with ${others.join(", ")}.`}
            actions={
              <>
                <Button onClick={() => setPending(null)}>Keep it off</Button>
                <Button variant="primary" onClick={() => toggle(pending.item, true, true)}>
                  Enable anyway
                </Button>
              </>
            }
          >
            Both replace the same {plural(shared, "thing")} in game; only one shows. Keep one on.
          </Notice>
        )}
        {blocked.length > 0 && (
          <Notice
            tone="warn"
            title={`${plural(blocked.length, "mod")} stayed off.`}
            actions={<IconButton icon="close" label="Dismiss" size="sm" onClick={() => setBlocked([])} />}
          >
            {blocked.join(", ")} would clash with mods that are on. Switch them on one at a time to choose.
          </Notice>
        )}
      </div>

      <div className="filterbar">
        <div className="toolbar">
          <div className="search">
            <label className="sr-only" htmlFor="lib-q">
              Search your mods
            </label>
            <Icon name="search" size={18} stroke={2.4} />
            <input
              id="lib-q"
              type="search"
              placeholder="Search your mods"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <Dropdown<string>
            label="Category"
            value={rootCategory}
            onChange={(v) => {
              setRootCategory(v);
              setCharacters([]);
            }}
            options={[{ value: "", label: "All mods" }, ...categoryOptions.map((c) => ({ value: c, label: c }))]}
            disabled={categoryOptions.length === 0}
            disabledReason="Mods installed from GameBanana carry their category"
          />
          <Dropdown<string>
            multiple
            label="Characters"
            allLabel="All characters"
            value={characters}
            onChange={setCharacters}
            options={characterOptions.map((c) => ({
              value: c,
              label: c,
              icon: (
                <span className="ch-icon ch-av" aria-hidden="true">
                  {c.slice(0, 2)}
                </span>
              ),
            }))}
            disabled={characterOptions.length === 0}
            disabledReason="Mods installed from GameBanana carry their character"
          />
          <ToggleChip
            label="Show sensitive"
            checked={show}
            onChange={(next) => void setPrefs({ show_sensitive: next })}
          />
          <span className="count">
            {plural(shown.length, "mod")} · {shown.filter((i) => i.enabled).length} on
            {hiddenSensitive > 0 && ` · ${hiddenSensitive} hidden as sensitive`}
          </span>
          <span className="spacer" />
          <Button
            icon="layers"
            onClick={() => setProfilesOpen(true)}
            disabled={scope === ALL}
            title={scope === ALL ? "Pick a game first: profiles are per game" : undefined}
          >
            Profiles
          </Button>
          <Button onClick={() => setMany(shown, true)} disabled={shown.length === 0 || busy}>
            Enable all
          </Button>
          <Button onClick={() => setMany(shown, false)} disabled={shown.length === 0 || busy}>
            Disable all
          </Button>
          <Button
            icon={selecting ? "close" : "check"}
            aria-pressed={selecting}
            onClick={() => (selecting ? stopSelecting() : setSelecting(true))}
            disabled={!selecting && shown.length === 0}
          >
            Select
          </Button>
        </div>
        {selecting && (
          <div className="selbar" role="toolbar" aria-label="Selected mods">
            <b>{plural(chosen.length, "mod")} selected</b>
            <span className="sub">Tick cards to choose them.</span>
            <Button
              variant="link"
              onClick={() => setSelected(new Set(chosen.length === shown.length ? [] : shown.map((i) => i.id)))}
            >
              {chosen.length === shown.length ? "Select none" : "Select all"}
            </Button>
            <span className="spacer" />
            <Button disabled={chosen.length === 0 || busy} onClick={() => setMany(chosen, true)}>
              Enable selected
            </Button>
            <Button disabled={chosen.length === 0 || busy} onClick={() => setMany(chosen, false)}>
              Disable selected
            </Button>
            <Button variant="danger" icon="trash" disabled={chosen.length === 0 || busy} onClick={askDelete}>
              Delete selected
            </Button>
            <Button variant="primary" onClick={stopSelecting}>
              Done
            </Button>
          </div>
        )}
      </div>

      {data.loading ? (
        <div className="card empty">Loading mods…</div>
      ) : shown.length === 0 ? (
        <div className="card empty">
          {items.length === 0
            ? "No mods here yet. Find some in the Mods Store."
            : visible.length === 0
              ? "Every mod here is marked sensitive. Turn on Show sensitive to see them."
              : "No mods match these filters."}
          {filtered && visible.length > 0 && (
            <div>
              <Button
                variant="link"
                onClick={() => {
                  setQuery("");
                  setRootCategory("");
                  setCharacters([]);
                }}
              >
                Reset filters
              </Button>
            </div>
          )}
        </div>
      ) : (
        <div className="grid">
          {shown.map((item) => (
            <ModCard
              key={item.id}
              item={item}
              showGame={scope === ALL}
              update={updateFor(item) !== undefined}
              conflict={item.gbModId !== null && item.enabled && (owned.get(item.gbModId)?.conflict ?? false)}
              selecting={selecting}
              selected={selected.has(item.id)}
              onSelect={(on) => pick(item, on)}
              onToggle={(on) => toggle(item, on)}
              onSettings={() => setSettingsFor(item.id)}
            />
          ))}
        </div>
      )}

      {profilesOpen && inScope.length === 1 && inScope[0] && (
        <ProfilesPanel
          game={inScope[0]}
          onClose={() => setProfilesOpen(false)}
          onApplied={() => data.refreshLibrary(scope)}
        />
      )}
      {deleting && (
        <UninstallDialog
          batches={deleting}
          onClose={() => setDeleting(null)}
          onDone={async () => {
            setDeleting(null);
            stopSelecting();
            await Promise.all(inScope.map((g) => data.refreshLibrary(g.id)));
          }}
        />
      )}
      {settingsItem && (
        <ModSettings
          item={settingsItem}
          onClose={() => setSettingsFor(null)}
          onToggle={(on) => toggle(settingsItem, on)}
          onRefresh={() => data.refreshLibrary(settingsItem.game.id)}
          onRemoved={async () => {
            setSettingsFor(null);
            await data.refreshLibrary(settingsItem.game.id);
          }}
          update={updateFor(settingsItem)}
          onQueued={() => setSettingsFor(null)}
          onChanged={async () => {
            setSettingsFor(null);
            await data.refreshLibrary(settingsItem.game.id);
          }}
          onOpenMod={(id) => {
            setSettingsFor(null);
            onOpenMod(id);
          }}
          onAddVersions={(modId, fileId) => {
            setSettingsFor(null);
            onAddVersions(modId, fileId);
          }}
        />
      )}
    </>
  );
}

function ModCard({
  item,
  showGame,
  update,
  conflict,
  selecting,
  selected,
  onSelect,
  onToggle,
  onSettings,
}: {
  item: LibraryItem;
  showGame: boolean;
  update: boolean;
  conflict: boolean;
  selecting: boolean;
  selected: boolean;
  onSelect: (on: boolean) => void;
  onToggle: (on: boolean) => void;
  onSettings: () => void;
}) {
  const on = item.versions.find((v) => v.enabled);
  return (
    <article className={`card mod lib-card${selected ? " is-selected" : ""}`}>
      {selecting && (
        <button
          type="button"
          className="card-hit"
          aria-label={`${selected ? "Unselect" : "Select"} ${item.name}`}
          aria-pressed={selected}
          onClick={() => onSelect(!selected)}
        />
      )}
      <Preview
        image={item.preview ? { url: item.preview, variants: [] } : undefined}
        width={260}
        sensitive={item.sensitive}
        alt={item.name}
      >
        <div className="badges">
          <div className="badges-l">
            {selecting ? (
              <Checkbox checked={selected} onChange={onSelect} className="lib-check">
                <span className="sr-only">Select {item.name}</span>
              </Checkbox>
            ) : item.problem ? (
              <Badge tone="art">{item.problem}</Badge>
            ) : (
              item.category && <Badge tone="art">{item.category}</Badge>
            )}
          </div>
          <div className="badges-r">
            {showGame && <Badge tone="game">{item.game.short_name}</Badge>}
            {item.sensitive && <Badge tone="art">Sensitive</Badge>}
            {update ? (
              <Badge tone="yellow">Update</Badge>
            ) : (
              conflict && (
                <Badge tone="warn" title="Replaces the same things as another mod that's on">
                  Conflict
                </Badge>
              )
            )}
            {!selecting && (
              <IconButton
                icon="sliders"
                label={`Mod settings for ${item.name}`}
                variant="art"
                size="sm"
                onClick={onSettings}
              />
            )}
          </div>
        </div>
        {item.versions.length > 0 ? (
          <span className="art-chip">
            {on ? `${on.name} · ` : ""}
            {plural(item.versions.length, "version")}
          </span>
        ) : (
          item.hand && <span className="art-chip">Hand-installed</span>
        )}
      </Preview>
      <div className="mod-body">
        <h3 className="mod-title">{item.name}</h3>
        <div className="mod-foot">
          <div className="lib-meta">
            <span>{item.added}</span>
            {item.updated !== null && (
              <span title="When the creator uploaded the installed file to GameBanana">
                Updated {day(item.updated)}
              </span>
            )}
            {item.size !== null && <span>{bytes(item.size)}</span>}
          </div>
          <span className="spacer" />
          <Switch
            checked={item.enabled}
            onChange={onToggle}
            label={`${item.name} on or off`}
            disabled={!item.usable || selecting}
            showState
          />
        </div>
      </div>
    </article>
  );
}
