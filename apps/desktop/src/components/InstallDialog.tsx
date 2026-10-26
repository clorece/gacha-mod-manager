// SPDX-License-Identifier: MIT OR Apache-2.0
// The install confirmation. Nothing is downloaded while
// it's open: it reads the mod page (usually already cached) and asks, in up to three steps,
// which of GameBanana's files to install (several can be picked, and each one picked is
// switched on), what to do about mods already on for the same character, and to confirm.
// Each confirmed file goes to the queue (`queue_install`), which downloads, checks and
// installs it in the background; the user stays where they are and follows it in Downloads,
// where an install whose check finds something waits as "Needs action".

import { useEffect, useMemo, useState } from "react";
import { api, asCmdError, type GameInfo, type LibraryView, type ModFile, type ModProfile, type ModRow } from "../api";
import { imageAt } from "../catalog";
import { bytes, day, plural } from "../format";
import { Button, Checkbox, Dialog, Notice, Radio, Switch, useToast } from "../ui";

type Step = "files" | "conflicts" | "confirm";
/** What to do about mods already on for the same character. */
type Resolve = "new" | "keep" | "both";

const STEP_NAMES: Record<Step, string> = { files: "Variants", conflicts: "Conflicts", confirm: "Confirm" };

/** GameBanana's files GMM may install (scanned clean), newest first; an archived file only
 *  when it was picked on the mod page. */
export function installableFiles(profile: ModProfile, picked: number | null): ModFile[] {
  const clean = (f: ModFile) => f.av_result === "clean";
  const current = profile.files.files.filter(clean);
  const archived = profile.files.archived_files.filter((f) => f.id === picked && clean(f));
  return [...current, ...archived].sort((a, b) => b.date_added - a.date_added);
}

/** Mods that are on in the game and dress the same character (GMM's own installs, whose
 *  category is known), one entry per mod: they are very likely to replace the same things. */
export function sameCharacter(
  profile: ModProfile,
  game: GameInfo | undefined,
  view: LibraryView | undefined,
): ModRow[] {
  const root = profile.summary.root_category;
  const character = profile.summary.category;
  if (!game || !view || !root || !character) return [];
  if (!game.categories.some((c) => c.id === root.id && c.characters)) return [];
  const seen = new Set<string>();
  return view.managed.filter((m) => {
    if (!m.enabled || m.category_id !== character.id || m.gb_mod_id === profile.summary.id) return false;
    const key = m.group ?? `id:${m.id}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function InstallDialog({
  modId,
  initialFileId = null,
  games,
  libraries,
  onClose,
  onQueued,
}: {
  modId: number;
  /** A file picked on the mod page's Files tab (or "Add versions"); otherwise GameBanana's
   *  newest clean file starts ticked. */
  initialFileId?: number | null;
  games: GameInfo[];
  /** For the files already installed and the mods on for the same character. */
  libraries: Record<string, LibraryView>;
  onClose: () => void;
  /** After the installs were queued: how many, and the mod's name. */
  onQueued: (count: number, modName: string) => void;
}) {
  const toast = useToast();
  const [profile, setProfile] = useState<ModProfile | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [picked, setPicked] = useState<number[]>([]);
  const [step, setStep] = useState<Step>("files");
  const [resolve, setResolve] = useState<Resolve>("new");
  const [switchOn, setSwitchOn] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    api
      .catalogMod(modId)
      .then((p) => {
        if (!live) return;
        setProfile(p);
        const files = installableFiles(p, initialFileId);
        const start = files.find((f) => f.id === initialFileId) ?? files.find((f) => !f.archived);
        setPicked(start ? [start.id] : []);
      })
      .catch((e) => live && setFailed(asCmdError(e).message));
    return () => {
      live = false;
    };
  }, [modId, initialFileId]);

  const game = games.find((g) => g.gamebanana_id === profile?.summary.game_id);
  const view = game ? libraries[game.id] : undefined;
  const files = useMemo(() => (profile ? installableFiles(profile, initialFileId) : []), [profile, initialFileId]);
  const clashes = useMemo(() => (profile ? sameCharacter(profile, game, view) : []), [profile, game, view]);

  /** Files whose whole archive is installed already (one mod, no versions to add). */
  const installed = useMemo(() => {
    const rows = view?.managed.filter((m) => m.gb_mod_id === modId) ?? [];
    return new Map(
      files.map((f) => {
        const mine = rows.filter((r) => r.gb_file_id === f.id);
        return [f.id, mine.length === 0 ? null : mine.some((r) => r.variant !== null) ? "versions" : "whole"] as const;
      }),
    );
  }, [files, view, modId]);

  const steps: Step[] = [
    ...(files.length > 1 ? (["files"] as const) : []),
    ...(clashes.length > 0 && switchOn ? (["conflicts"] as const) : []),
    "confirm",
  ];
  const at = steps.includes(step) ? step : steps[0];
  const index = steps.indexOf(at);
  const go = (d: number) => setStep(steps[Math.max(0, Math.min(steps.length - 1, index + d))]);

  const toggle = (id: number, on: boolean) => setPicked((now) => (on ? [...now, id] : now.filter((x) => x !== id)));
  const choosable = files.filter((f) => installed.get(f.id) !== "whole");
  const allPicked = choosable.length > 0 && choosable.every((f) => picked.includes(f.id));
  const chosen = files.filter((f) => picked.includes(f.id));

  const confirm = async () => {
    if (!profile || chosen.length === 0) return;
    setBusy(true);
    const keep = clashes.length > 0 && switchOn && resolve === "keep";
    const switchOff = clashes.length > 0 && switchOn && resolve === "new" ? clashes.map((m) => m.id) : [];
    let queued = 0;
    try {
      for (const f of chosen) {
        await api.queueInstall({
          mod_id: modId,
          file_id: f.id,
          variants: { type: "only" },
          skip_blocked: false,
          start_disabled: !switchOn || keep,
          // Switched off once, after the first file installs.
          switch_off: queued === 0 ? switchOff : [],
        });
        queued += 1;
      }
      onQueued(queued, profile.summary.name);
    } catch (e) {
      toast("bad", asCmdError(e).message);
      if (queued > 0) onQueued(queued, profile.summary.name);
      setBusy(false);
    }
  };

  const viewOnGameBanana = () => {
    api.openModPage(modId).catch((e) => toast("bad", asCmdError(e).message));
  };

  const summary = profile?.summary;
  const thumb = imageAt(summary?.images[0], 100);
  const byline = [summary?.submitter.name && `by ${summary.submitter.name}`, summary?.category?.name, game?.name]
    .filter(Boolean)
    .join(" · ");
  const nothing = profile !== null && files.length === 0;
  const unsupported = profile !== null && !game;
  const blocked = failed !== null || nothing || unsupported;

  let primary: string;
  if (at === "confirm") primary = chosen.length > 1 ? `Install ${plural(chosen.length, "variant")}` : "Install";
  else primary = "Next";
  const canGo = profile !== null && !blocked && chosen.length > 0 && !busy;

  return (
    <Dialog
      eyebrow={[...installed.values()].some((v) => v !== null) ? "Add versions" : "Install"}
      title={summary?.name ?? "Getting the mod's details…"}
      subtitle={byline || undefined}
      media={thumb ? <img className="thumb cf-thumb" src={thumb} alt="" /> : <div className="thumb cf-thumb art a2" />}
      onClose={onClose}
      footer={
        <>
          {index > 0 && !blocked ? (
            <Button onClick={() => go(-1)}>Back</Button>
          ) : (
            <Button onClick={onClose}>Cancel</Button>
          )}
          {blocked ? (
            <Button variant="primary" onClick={viewOnGameBanana}>
              View on GameBanana
            </Button>
          ) : (
            <Button
              variant="primary"
              icon={at === "confirm" ? "download" : undefined}
              onClick={at === "confirm" ? confirm : () => go(1)}
              disabled={!canGo}
            >
              {primary}
            </Button>
          )}
        </>
      }
    >
      {failed !== null && (
        <Notice tone="bad" title="GMM couldn't read this mod from GameBanana.">
          {failed}
        </Notice>
      )}
      {nothing && (
        <Notice tone="bad" title="There's no file GMM can install.">
          GMM installs only files GameBanana scanned as clean, and this mod has none right now.
        </Notice>
      )}
      {unsupported && !nothing && (
        <Notice tone="bad" title="This mod is for a game GMM doesn't manage.">
          GMM installs mods for {games.map((g) => g.name).join(" and ")}.
        </Notice>
      )}

      {!profile && failed === null && (
        <div className="cf-loading" aria-hidden="true">
          <span className="skel-line" />
          <span className="skel-line short" />
        </div>
      )}

      {profile && !blocked && steps.length > 1 && (
        <ol className="cf-steps" aria-label="Steps">
          {steps.map((s, i) => (
            <li key={s} className={i === index ? "is-current" : i < index ? "is-done" : undefined}>
              <span className="cf-step-n">{i + 1}</span>
              {STEP_NAMES[s]}
            </li>
          ))}
        </ol>
      )}

      {profile && !blocked && at === "files" && (
        <>
          <div className="cf-h-row">
            <h3 className="cf-h">Choose variants</h3>
            <span className="count">
              {picked.length} of {files.length} selected
            </span>
            <span className="grow" />
            {choosable.length > 1 && (
              <Button variant="link" onClick={() => setPicked(allPicked ? [] : choosable.map((f) => f.id))}>
                {allPicked ? "Clear" : "Select all"}
              </Button>
            )}
          </div>
          <p className="sub">
            This mod has {files.length} files on GameBanana. Pick every variant you want: some mods come in pieces that
            work together, so each one you pick is switched on.
          </p>
          <div className="opt-list cf-vlist">
            {files.map((f) => {
              const has = installed.get(f.id);
              const on = picked.includes(f.id);
              return (
                <Checkbox
                  key={f.id}
                  className="opt-row"
                  checked={has === "whole" || on}
                  disabled={has === "whole"}
                  onChange={(next) => toggle(f.id, next)}
                >
                  <span className="grow">
                    <b className="block font-extrabold">{f.description ?? f.name}</b>
                    <span className="sub">
                      {f.description ? `${f.name} · ` : ""}
                      {bytes(f.size)} · {day(f.date_added)}
                      {f.archived ? " · an older file" : ""}
                    </span>
                  </span>
                  {on && switchOn && <span className="tag-on">On after install</span>}
                  {has === "whole" && <span className="tag-done">Installed</span>}
                  {has === "versions" && <span className="tag-done">Some installed</span>}
                </Checkbox>
              );
            })}
          </div>
          <p className="hint">
            Variants that replace exactly the same things can't be on together; GMM keeps the first one on and the other
            off. Switch between them later in the mod's settings.
          </p>
        </>
      )}

      {profile && !blocked && at === "conflicts" && (
        <>
          <div className="cf-h-row">
            <h3 className="cf-h">Resolve a conflict</h3>
          </div>
          <p className="sub">
            {plural(clashes.length, "mod")} for {summary?.category?.name} {clashes.length === 1 ? "is" : "are"} on
            already. Two mods for one character usually replace the same things, and only one of them shows in game.
          </p>
          <div className="cf-clash">
            {clashes.map((m) => (
              <div className="cf-clash-row" key={m.id}>
                {m.preview ? (
                  <img className="thumb cf-clash-thumb" src={m.preview} alt="" loading="lazy" decoding="async" />
                ) : (
                  <div className="thumb cf-clash-thumb art a3" />
                )}
                <b className="truncate">{m.name}</b>
                <span className="badge badge-ok">On</span>
              </div>
            ))}
          </div>
          <div className="opt-list">
            <Radio className="opt-row" name="resolve" checked={resolve === "new"} onChange={() => setResolve("new")}>
              <span className="grow">
                <b className="block font-extrabold">Use {summary?.name}</b>
                <span className="sub">
                  Switch {clashes.length === 1 ? clashes[0].name : "those mods"} off once this one is installed.
                </span>
              </span>
            </Radio>
            <Radio className="opt-row" name="resolve" checked={resolve === "keep"} onChange={() => setResolve("keep")}>
              <span className="grow">
                <b className="block font-extrabold">Keep what's on</b>
                <span className="sub">Install {summary?.name} switched off; switch it on later in the Library.</span>
              </span>
            </Radio>
            <Radio className="opt-row" name="resolve" checked={resolve === "both"} onChange={() => setResolve("both")}>
              <span className="grow">
                <b className="block font-extrabold">Skip</b>
                <span className="sub">Leave both on and decide later. The Library marks the conflict.</span>
              </span>
            </Radio>
          </div>
        </>
      )}

      {profile && !blocked && at === "confirm" && (
        <>
          <div className="kv">
            <div className="kv-row">
              <span>{chosen.length > 1 ? "Variants" : "File"}</span>
              <b>
                {chosen.length === 1
                  ? `${chosen[0].name} · ${bytes(chosen[0].size)}`
                  : `${chosen.length} files · ${bytes(chosen.reduce((n, f) => n + f.size, 0))}`}
              </b>
            </div>
            {clashes.length > 0 && switchOn && (
              <div className="kv-row">
                <span>Conflict</span>
                <b>
                  {resolve === "new"
                    ? `Switches off ${clashes.length === 1 ? clashes[0].name : plural(clashes.length, "mod")}`
                    : resolve === "keep"
                      ? "Installs switched off"
                      : "Both stay on"}
                </b>
              </div>
            )}
            <div className="kv-row">
              <span>Installs to</span>
              <b>
                {game?.name} › {game?.importer}\Mods\GMM
              </b>
            </div>
            <div className="kv-row">
              <span>Safety</span>
              <b>Scanned clean by GameBanana; GMM checks every file again before installing</b>
            </div>
          </div>
          <div className="cf-switch">
            <div className="grow">
              <b>Switch it on after installing</b>
              <div className="sub">Otherwise it's installed switched off.</div>
            </div>
            <Switch checked={switchOn} onChange={setSwitchOn} label="Switch it on after installing" />
          </div>
          <p className="hint">
            GMM downloads and checks it in the background, so you can keep browsing. Follow it in Downloads; if the
            check finds something (a file that holds several versions, files GMM won't install), it waits there for you.
          </p>
        </>
      )}
    </Dialog>
  );
}
