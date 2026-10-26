// SPDX-License-Identifier: MIT OR Apache-2.0
// Dashboard (docs/08): a bubble per game that enlarges to reveal Launch, then recently
// installed mods and each game's setup. A game that isn't Ready gets its setup card here
// instead of a Launch button.

import { useState } from "react";
import type { GameInfo, GameStatus } from "../api";
import { useLaunch } from "../components/Launch";
import { Preview } from "../components/Preview";
import { SetupCard } from "../components/SetupCard";
import { type AppData, modCounts } from "../data";
import { ago, bytes, day, plural } from "../format";
import { bannerClass, launchRoute, setupLabel, sourceLabel } from "../games";
import { Button, Chip, Icon, Notice } from "../ui";

/** Placeholder art for a mod without a preview (Downloads rows): one of six stripes, by id. */
export function artClass(id: number): string {
  return `art a${(id % 6) + 1}`;
}

export function Dashboard({ data }: { data: AppData }) {
  const [open, setOpen] = useState<string | null>(null);
  const { games, statuses, libraries } = data;

  const totals = games.map((g) => modCounts(libraries[g.id]));
  const total = totals.reduce((n, c) => n + c.total, 0);
  const on = totals.reduce((n, c) => n + c.on, 0);
  const xxmiFound = statuses.some((s) => s.xxmi_root);

  const recent = games
    .flatMap((g) => (libraries[g.id]?.managed ?? []).map((m) => ({ ...m, gameName: g.name })))
    .sort((a, b) => b.installed_at - a.installed_at)
    .slice(0, 5);
  const notReady = statuses.filter((s) => s.state !== "ready");

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Dashboard</h1>
          <p>Choose a game to start it with your mods. Mods only load when the game starts through XXMI.</p>
        </div>
        <div className="chips">
          <Chip icon="download">{plural(total, "mod")} installed</Chip>
          <Chip>{on} on</Chip>
          <Chip dot={xxmiFound ? "ok" : "bad"}>{xxmiFound ? "XXMI found" : "XXMI not found"}</Chip>
        </div>
      </div>

      {data.error && (
        <Notice tone="bad" title="Something went wrong.">
          {data.error}
        </Notice>
      )}

      <div className="games-row">
        {games.map((g) => {
          const status = statuses.find((s) => s.game_id === g.id);
          return (
            status && (
              <GameBubble
                key={g.id}
                game={g}
                status={status}
                counts={modCounts(libraries[g.id])}
                open={open === g.id}
                onToggle={() => setOpen((o) => (o === g.id ? null : g.id))}
              />
            )
          );
        })}
        <div className="ghost-bubble">
          <Icon name="plus" size={28} />
          More games later
          <span className="sub">Arknights: Endfield is next</span>
        </div>
      </div>

      <div className="two-col">
        <section className="card panel" aria-labelledby="recent-h">
          <h2 id="recent-h">Recently installed</h2>
          {recent.length === 0 && <p className="sub">Nothing installed through GMM yet.</p>}
          {recent.map((m) => (
            <div className="row" key={m.id}>
              <Preview
                className="thumb"
                image={m.preview ? { url: m.preview, variants: [] } : undefined}
                width={52}
                sensitive={m.sensitive}
                alt={m.name}
              />
              <div className="grow">
                <div className="title-sm">{m.name}</div>
                <div className="sub">
                  {m.gameName} · {day(m.installed_at)} · {bytes(m.size_bytes)}
                </div>
              </div>
              <span className={`switch-label ${m.enabled ? "state-on" : "state-off"}`}>{m.enabled ? "On" : "Off"}</span>
            </div>
          ))}
        </section>
        <section className="card panel" aria-labelledby="setup-h">
          <h2 id="setup-h">Setup</h2>
          {statuses.map((s) => (
            <SetupRow key={s.game_id} status={s} />
          ))}
          <div className="row">
            <span className={`dot${xxmiFound ? "" : " dot-bad"}`} aria-hidden="true" />
            <div className="grow">
              <div className="title-sm">XXMI Launcher</div>
              <div className="sub">{statuses.find((s) => s.xxmi_root)?.xxmi_root ?? "Not found"}</div>
            </div>
          </div>
        </section>
      </div>

      {notReady.length > 0 && (
        <div className="stack mt-6">
          {notReady.map((s) => (
            <SetupCard key={s.game_id} status={s} onRecheck={data.refresh} />
          ))}
        </div>
      )}
    </>
  );
}

function SetupRow({ status }: { status: GameStatus }) {
  const state = setupLabel(status.state);
  const install = status.detected_installs[0];
  const version = status.importer_info?.importer_version;
  const parts = [
    state.label,
    install && `${sourceLabel(install.source)} install`,
    version ? `${status.importer} ${version}` : status.importer,
  ].filter(Boolean);
  return (
    <div className="row">
      <span className={`dot${state.tone === "ok" ? "" : ` dot-${state.tone}`}`} aria-hidden="true" />
      <div className="grow">
        <div className="title-sm">{status.game_name}</div>
        <div className="sub">{parts.join(" · ")}</div>
      </div>
    </div>
  );
}

function GameBubble({
  game,
  status,
  counts,
  open,
  onToggle,
}: {
  game: GameInfo;
  status: GameStatus;
  counts: { total: number; on: number };
  open: boolean;
  onToggle: () => void;
}) {
  const { launch, busy, dialog } = useLaunch(game);
  const state = setupLabel(status.state);
  const ready = status.state === "ready";
  return (
    <div className={`bubble${open ? " is-open" : ""}`}>
      <div className={bannerClass(game.id)} data-mark={game.short_name}>
        <span className="fade" />
      </div>
      <button
        type="button"
        className="bubble-hit"
        aria-expanded={open}
        aria-label={`${game.name}: ${open ? "hide" : "show"} launch options`}
        onClick={onToggle}
      />
      <div className="bubble-info">
        <div className="bubble-name">{game.name}</div>
        <div className="bubble-meta">
          <span className="meta" title="Mods installed">
            <Icon name="download" size={14} stroke={2.6} />
            {plural(counts.total, "mod")}
          </span>
          <span className="meta" title="Last started from GMM">
            <Icon name="clock" size={14} stroke={2.6} />
            {game.last_played ? `Played ${ago(game.last_played)}` : "Not played from GMM yet"}
          </span>
          <span className="meta">
            <span className={`dot${state.tone === "ok" ? "" : ` dot-${state.tone}`}`} aria-hidden="true" />
            {state.label}
          </span>
        </div>
        {open && (
          <div className="bubble-actions">
            {ready ? (
              <>
                <Button variant="launch" icon="play" disabled={busy} onClick={launch}>
                  Launch
                </Button>
                <small>
                  {counts.on} of {plural(counts.total, "mod")} on · {launchRoute(status)}
                </small>
              </>
            ) : (
              <small>{status.summary} The steps are below.</small>
            )}
          </div>
        )}
      </div>
      {dialog}
    </div>
  );
}
