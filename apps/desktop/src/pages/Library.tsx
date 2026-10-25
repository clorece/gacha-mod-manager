// SPDX-License-Identifier: MIT OR Apache-2.0
// Mods Library (docs/08): game banner cards, a sticky toolbar, and a card per mod with an
// On/Off switch and a settings button. GMM's mods and hand-installed ones show together.
// Switching on a mod that conflicts with one that's on asks first ("Enable anyway").
// Category, character and select mode come with Phase 5's stored categories.

import { useEffect, useState } from "react";
import { api, asCmdError, type Conflict, type GameInfo, type ModKey, type ModRef, type ModUpdateInfo } from "../api";
import { ModSettings } from "../components/ModSettings";
import { ProfilesPanel } from "../components/ProfilesPanel";
import { type AppData, modCounts } from "../data";
import { bytes, day, plural } from "../format";
import { ALL, bannerClass, type Scope } from "../games";
import { Badge, Button, Icon, IconButton, Notice, Switch, useToast } from "../ui";
import { artClass } from "./Dashboard";

/** One card: a GMM-installed or hand-installed mod in one game. */
export type LibraryItem = {
  game: GameInfo;
  key: ModKey;
  name: string;
  enabled: boolean;
  /** False when the folder is missing or there are two copies: the switch is off-limits. */
  usable: boolean;
  problem: string | null;
  hand: boolean;
  /** "Installed 24 Oct 2026" or "Added by hand". */
  added: string;
  size: number | null;
  version: string | null;
  folder: string;
  art: string;
  /** GameBanana mod id, for GMM's mods from GameBanana. */
  gbModId: number | null;
  /** A folder (hand-installed loose `.ini` files aren't). */
  isDir: boolean;
};

function refName(ref: ModRef): string {
  return ref.type === "managed" ? ref.folder.replace(/^DISABLED /, "") : ref.path;
}

function itemsFor(data: AppData, games: GameInfo[]): LibraryItem[] {
  return games.flatMap((game) => {
    const view = data.libraries[game.id];
    if (!view) return [];
    return [
      ...view.managed.map<LibraryItem>((m) => ({
        game,
        key: { type: "managed", id: m.id },
        name: m.name,
        enabled: m.enabled,
        usable: m.disk === "enabled" || m.disk === "disabled",
        problem: m.disk === "missing" ? "Folder missing" : m.disk === "ambiguous" ? "Two copies" : null,
        hand: false,
        added: `Installed ${day(m.installed_at)}`,
        size: m.size_bytes,
        version: m.version,
        folder: `GMM/${m.folder}`,
        art: artClass(m.id),
        gbModId: m.gb_mod_id,
        isDir: true,
      })),
      ...view.unmanaged.map<LibraryItem>((u) => ({
        game,
        key: { type: "unmanaged", path: u.path },
        name: u.name,
        enabled: u.enabled,
        usable: true,
        problem: null,
        hand: true,
        added: "Added by hand",
        size: null,
        version: null,
        folder: u.path,
        art: "art art-hand",
        gbModId: null,
        isDir: u.is_dir,
      })),
    ];
  });
}

function isSelf(ref: ModRef, key: ModKey): boolean {
  return ref.type === "managed"
    ? key.type === "managed" && ref.id === key.id
    : key.type === "unmanaged" && ref.path === key.path;
}

const keyId = (game: string, key: ModKey) => `${game}|${key.type === "managed" ? key.id : key.path}`;

export function Library({
  data,
  scope,
  onScope,
  updates,
  onQueued,
  focus,
  onFocusDone,
  onOpenMod,
}: {
  data: AppData;
  scope: Scope;
  onScope: (s: Scope) => void;
  /** Newer files found by the startup check. */
  updates: ModUpdateInfo[];
  onQueued: () => void;
  /** A mod whose settings to open (asked for from the Store or a mod page). */
  focus: { game: string; key: ModKey } | null;
  onFocusDone: () => void;
  /** GMM's mod page for a GameBanana mod. */
  onOpenMod: (gbModId: number) => void;
}) {
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [pending, setPending] = useState<{ item: LibraryItem; conflicts: Conflict[] } | null>(null);
  const [blocked, setBlocked] = useState<string[]>([]);
  const [settingsFor, setSettingsFor] = useState<string | null>(null);
  const [profilesOpen, setProfilesOpen] = useState(false);

  useEffect(() => {
    if (!focus) return;
    setSettingsFor(keyId(focus.game, focus.key));
    onFocusDone();
  }, [focus, onFocusDone]);

  const inScope = scope === ALL ? data.games : data.games.filter((g) => g.id === scope);
  const items = itemsFor(data, inScope);
  const shown = items.filter((i) => i.name.toLowerCase().includes(query.trim().toLowerCase()));
  const problems = inScope.flatMap((g) => {
    const p = data.libraries[g.id]?.problem;
    return p ? [{ game: g, problem: p }] : [];
  });

  /** Returns the conflicts when switching on was refused because of them. */
  const setEnabled = async (item: LibraryItem, enabled: boolean, anyway = false): Promise<Conflict[] | null> => {
    try {
      await api.setEnabled(item.game.id, item.key, enabled, anyway);
      return null;
    } catch (e) {
      const err = asCmdError(e);
      if (err.kind === "conflicts") return err.conflicts;
      toast("bad", err.message);
      return null;
    }
  };

  const toggle = async (item: LibraryItem, enabled: boolean, anyway = false) => {
    setPending(null);
    const conflicts = await setEnabled(item, enabled, anyway);
    if (conflicts) setPending({ item, conflicts });
    await data.refreshLibrary(item.game.id);
  };

  /** Enable all applies the same conflict rule: a mod that would clash stays off. */
  const setAll = async (enabled: boolean) => {
    setPending(null);
    const stayedOff: string[] = [];
    for (const item of shown) {
      if (item.enabled === enabled || !item.usable) continue;
      if (await setEnabled(item, enabled)) stayedOff.push(item.name);
    }
    setBlocked(stayedOff);
    await Promise.all(inScope.map((g) => data.refreshLibrary(g.id)));
  };

  const others = pending?.conflicts.map((c) => refName(isSelf(c.a, pending.item.key) ? c.b : c.a));
  const shared = pending ? Math.max(...pending.conflicts.map((c) => c.hashes.length)) : 0;
  const settingsItem = items.find((i) => keyId(i.game.id, i.key) === settingsFor);

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Mods Library</h1>
          <p>Your installed mods. Switch one off and XXMI skips it next time the game starts.</p>
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
          <span className="count">
            {plural(shown.length, "mod")} · {shown.filter((i) => i.enabled).length} on
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
          <Button onClick={() => setAll(true)} disabled={shown.length === 0}>
            Enable all
          </Button>
          <Button onClick={() => setAll(false)} disabled={shown.length === 0}>
            Disable all
          </Button>
        </div>
      </div>

      {data.loading ? (
        <div className="card empty">Loading mods…</div>
      ) : shown.length === 0 ? (
        <div className="card empty">
          {items.length === 0 ? "No mods here yet. Find some in the Mods Store." : "No mods match your search."}
        </div>
      ) : (
        <div className="grid">
          {shown.map((item) => (
            <ModCard
              key={keyId(item.game.id, item.key)}
              item={item}
              showGame={scope === ALL}
              onToggle={(on) => toggle(item, on)}
              onSettings={() => setSettingsFor(keyId(item.game.id, item.key))}
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
      {settingsItem && (
        <ModSettings
          item={settingsItem}
          onClose={() => setSettingsFor(null)}
          onToggle={(on) => toggle(settingsItem, on)}
          onRemoved={async () => {
            setSettingsFor(null);
            await data.refreshLibrary(settingsItem.game.id);
          }}
          update={updates.find((u) => u.game === settingsItem.game.id && u.gb_mod_id === settingsItem.gbModId)}
          onQueued={() => {
            setSettingsFor(null);
            onQueued();
          }}
          onChanged={async () => {
            setSettingsFor(null);
            await data.refreshLibrary(settingsItem.game.id);
          }}
          onOpenMod={(id) => {
            setSettingsFor(null);
            onOpenMod(id);
          }}
        />
      )}
    </>
  );
}

function ModCard({
  item,
  showGame,
  onToggle,
  onSettings,
}: {
  item: LibraryItem;
  showGame: boolean;
  onToggle: (on: boolean) => void;
  onSettings: () => void;
}) {
  return (
    <article className="card mod">
      <div className={item.art}>
        <div className="badges">
          <div className="badges-l">{item.problem && <Badge tone="art">{item.problem}</Badge>}</div>
          <div className="badges-r">
            {showGame && <Badge tone="game">{item.game.short_name}</Badge>}
            <IconButton
              icon="sliders"
              label={`Mod settings for ${item.name}`}
              variant="art"
              size="sm"
              onClick={onSettings}
            />
          </div>
        </div>
        <span className="art-chip">{item.hand ? "Hand-installed" : "Preview image"}</span>
      </div>
      <div className="mod-body">
        <h3 className="mod-title">{item.name}</h3>
        <div className="mod-foot">
          <div className="lib-meta">
            <span>{item.added}</span>
            {item.size !== null && <span>{bytes(item.size)}</span>}
          </div>
          <span className="spacer" />
          <Switch
            checked={item.enabled}
            onChange={onToggle}
            label={`${item.name} on or off`}
            disabled={!item.usable}
            showState
          />
        </div>
      </div>
    </article>
  );
}
