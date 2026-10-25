// SPDX-License-Identifier: MIT OR Apache-2.0
// Downloads (docs/08, "Downloads"): the install queue, grouped by game, newest first. Every
// choice was made in the install confirmation, so rows only show progress and the actions
// that fit their state. Installed rows clear themselves after a few seconds (queue.ts).

import { useState } from "react";
import { api, asCmdError, type GameInfo, type Job } from "../api";
import { bytes, plural } from "../format";
import { bannerClass } from "../games";
import { CLEAR_DONE_AFTER_MS } from "../queue";
import { Button, IconButton, ProgressBar, useToast } from "../ui";
import { artClass } from "./Dashboard";

export function Downloads({
  jobs,
  games,
  onShowLibrary,
  onInstallFile,
}: {
  jobs: Job[];
  games: GameInfo[];
  onShowLibrary: (game: string) => void;
  /** Opens a file dialog for a local archive. */
  onInstallFile: () => Promise<void>;
}) {
  const toast = useToast();
  const finished = jobs.filter((j) => ["done", "failed", "cancelled"].includes(j.state.type)).length;

  const act = (what: Promise<unknown>) => {
    what.catch((e) => toast("bad", asCmdError(e).message));
  };

  const groups = games
    .map((g) => ({
      game: g,
      jobs: jobs.filter((j) => j.game === g.id).sort((a, b) => b.queued_at - a.queued_at || b.id - a.id),
    }))
    .filter((g) => g.jobs.length > 0);

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Downloads</h1>
          <p>Active and recent installs, grouped by game. Finished ones clear themselves after a few seconds.</p>
        </div>
        <div className="chips">
          <Button icon="plus" onClick={() => act(onInstallFile())}>
            Install from a file
          </Button>
          <Button disabled={finished === 0} onClick={() => act(api.queueClearFinished())}>
            Clear finished
          </Button>
        </div>
      </div>

      {groups.length === 0 && (
        <div className="card empty">
          Nothing downloading. Mods you install from the Mods Store show up here. You can also drop a .zip, .7z or .rar
          on the window.
        </div>
      )}

      {groups.map(({ game, jobs: list }) => (
        <section className="dl-group" key={game.id} aria-label={game.name}>
          <div className="dl-group-head">
            <div className={`dl-group-chip ${bannerClass(game.id)}`} />
            <h2>{game.name}</h2>
            <span className="count">{list.length}</span>
          </div>
          {list.map((job) => (
            <DownloadRow key={job.id} job={job} act={act} onShowLibrary={() => onShowLibrary(job.game)} />
          ))}
        </section>
      ))}
    </>
  );
}

function DownloadRow({
  job,
  act,
  onShowLibrary,
}: {
  job: Job;
  act: (p: Promise<unknown>) => void;
  onShowLibrary: () => void;
}) {
  const [opening, setOpening] = useState(false);
  const s = job.state;
  const sub = [
    job.author,
    job.file_name,
    job.file_size !== null && s.type !== "downloading" ? bytes(job.file_size) : null,
    job.versions > 1 ? `${plural(job.versions, "version")} chosen` : null,
  ]
    .filter(Boolean)
    .join(" · ");

  let line: string;
  let tone: "run" | "ok" | "bad" | "wait";
  let bar: number | "ind" | null = null;
  switch (s.type) {
    case "queued":
      [line, tone] = ["Queued", "wait"];
      break;
    case "downloading": {
      const total = s.total ?? job.file_size;
      const pct = total ? Math.floor((s.downloaded / total) * 100) : null;
      line =
        pct === null
          ? `Downloading · ${bytes(s.downloaded)}`
          : `Downloading · ${pct}% · ${bytes(s.downloaded)} of ${bytes(total ?? 0)}`;
      tone = "run";
      bar = total ? s.downloaded / total : "ind";
      break;
    }
    case "extracting":
      [line, tone, bar] = ["Extracting and scanning", "run", "ind"];
      break;
    case "installing":
      [line, tone, bar] = [`Installing · ${s.folder}`, "run", "ind"];
      break;
    case "waiting": {
      const at = new Date(s.until * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      [line, tone] = [`Waiting · GameBanana asked GMM to slow down · carries on at ${at}`, "wait"];
      break;
    }
    case "paused":
      [line, tone] = ["Paused · the part already downloaded is kept", "wait"];
      break;
    case "done":
      [line, tone] = [
        `${job.action === "change_file" ? "Updated" : "Installed"} · just now${s.installed.length > 1 ? ` · ${plural(s.installed.length, "version")}` : ""} · clears in ${CLEAR_DONE_AFTER_MS / 1000} s`,
        "ok",
      ];
      break;
    case "failed":
      [line, tone] = [`Can't install: ${s.message}`, "bad"];
      break;
    case "cancelled":
      [line, tone] = ["Cancelled", "wait"];
      break;
  }

  const viewOnGameBanana = async () => {
    setOpening(true);
    try {
      await api.openModPage(job.mod_id);
    } catch (e) {
      act(Promise.reject(e));
    } finally {
      setOpening(false);
    }
  };

  return (
    <div className={`card dl${s.type === "done" ? " is-done" : ""}`}>
      {job.image ? (
        <img className="thumb dl-thumb" src={job.image} alt="" loading="lazy" />
      ) : (
        <div className={`thumb dl-thumb ${artClass(job.mod_id)}`} />
      )}
      <div className="grow">
        <div className="title-sm">{job.mod_name}</div>
        {sub && <div className="sub dl-sub">{sub}</div>}
        <div className={`dl-state st-${tone}`}>{line}</div>
        {bar !== null && <ProgressBar value={bar === "ind" ? undefined : bar} label={`${job.mod_name}: ${line}`} />}
      </div>
      {(s.type === "queued" || s.type === "downloading" || s.type === "waiting") && (
        <>
          {(s.type === "downloading" || s.type === "waiting") && (
            <IconButton icon="pause" label="Pause" onClick={() => act(api.queuePause(job.id))} />
          )}
          <IconButton icon="close" label="Cancel" onClick={() => act(api.queueCancel(job.id))} />
        </>
      )}
      {s.type === "paused" && (
        <>
          <IconButton icon="play" label="Resume" onClick={() => act(api.queueResume(job.id))} />
          <IconButton icon="close" label="Cancel" onClick={() => act(api.queueCancel(job.id))} />
        </>
      )}
      {s.type === "done" && <Button onClick={onShowLibrary}>Show in library</Button>}
      {s.type === "failed" && (
        <Button onClick={viewOnGameBanana} disabled={opening}>
          View on GameBanana
        </Button>
      )}
      {(s.type === "failed" || s.type === "cancelled") && (
        <IconButton icon="close" label="Dismiss" onClick={() => act(api.queueDismiss(job.id))} />
      )}
    </div>
  );
}
