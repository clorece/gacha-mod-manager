// SPDX-License-Identifier: MIT OR Apache-2.0
// The startup check (docs/08, "Startup check"): a full-window screen while GMM pulls the
// newest catalogue pages and checks the installed mods for updates (`startup_check`). The
// app works offline, so a failing step is shown and the app opens anyway; Skip for now
// opens it at once while the check finishes in the background.

import { useEffect, useRef, useState } from "react";
import { api, asCmdError, type GameInfo, type StartupReport, type StartupStep, type StepState } from "../api";
import logo from "../assets/gmm-logo.svg";
import { plural } from "../format";
import { Button, Icon } from "../ui";

/** How long "All set" stays up before the app opens. */
const OPEN_AFTER_MS = 900;

type Row = { label: string; state: StepState | "waiting"; detail: string };

/** The steps the check will report, before it reports them (same order as startup.rs). */
function plannedRows(games: GameInfo[]): Row[] {
  return [
    { label: "GMM app", state: "waiting", detail: "Waiting" },
    ...games.map((g) => ({ label: `${g.name} mods`, state: "waiting" as const, detail: "Waiting" })),
    { label: "Your installed mods", state: "waiting", detail: "Waiting" },
  ];
}

function statusLine(rows: Row[], finished: boolean): string {
  if (finished) return "All set";
  const working = rows.findIndex((r) => r.state === "working");
  if (working === -1) return "Starting";
  if (working === rows.length - 1) return "Checking your installed mods for updates";
  return "Pulling new GameBanana mods";
}

export function Startup({
  games,
  onDone,
}: {
  games: GameInfo[];
  /** The report, or null when the check failed outright or was skipped before it ended. */
  onDone: (report: StartupReport | null) => void;
}) {
  const [rows, setRows] = useState<Row[]>(() => plannedRows(games));
  const [report, setReport] = useState<StartupReport | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const finished = report !== null || failure !== null;
  // Only the first call wins: Skip, the timer, or the check failing.
  const done = useRef(false);
  const finish = (r: StartupReport | null) => {
    if (done.current) return;
    done.current = true;
    onDone(r);
  };
  const finishRef = useRef(finish);
  finishRef.current = finish;

  // The planned list waits for the game list, which loads alongside.
  useEffect(() => {
    setRows((current) => (current.every((r) => r.state === "waiting") ? plannedRows(games) : current));
  }, [games]);

  useEffect(() => {
    let live = true;
    const onStep = (s: StartupStep) => {
      if (!live) return;
      setRows((current) => {
        const next = [...current];
        while (next.length <= s.step) next.push({ label: "", state: "waiting", detail: "Waiting" });
        next[s.step] = { label: s.label, state: s.state, detail: s.detail };
        return next;
      });
    };
    api
      .startupCheck(onStep)
      .then((r) => live && setReport(r))
      .catch((e) => live && setFailure(asCmdError(e).message));
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    if (!finished) return;
    const timer = setTimeout(() => finishRef.current(report), OPEN_AFTER_MS);
    return () => clearTimeout(timer);
  }, [finished, report]);

  const settled = rows.filter((r) => r.state === "done" || r.state === "skipped" || r.state === "failed").length;
  const pct = finished ? 100 : Math.round((settled / Math.max(rows.length, 1)) * 100);
  const working = rows.find((r) => r.state === "working");
  const failed = rows.filter((r) => r.state === "failed").length;
  const updates = report?.updates.length ?? 0;

  let detail = working?.label.replace(/ mods$/, "") ?? "GMM 0.1.0";
  if (finished) {
    detail = updates > 0 ? `${plural(updates, "mod update")} found` : "Opening GMM";
  }

  return (
    <div className="su">
      <div className="su-glow" />
      <div className="su-col">
        <img className="su-logo" src={logo} alt="" />
        <div className="su-title">Gacha Mod Manager</div>
        <div className="su-pill">
          <span className="su-dot" />
          Powered by GameBanana
        </div>

        <section className="su-card" aria-live="polite">
          <h1 className="su-h">
            {finished && failed === 0 && !failure ? "You’re up to date" : "Checking for updates"}
          </h1>
          <div className={`su-status${finished ? (failure || failed ? " is-warn" : " is-done") : ""}`}>
            {!finished && <span className="spin" aria-hidden="true" />}
            {failure
              ? "Couldn't finish the check; GMM opens with what it has"
              : failed
                ? `All set, ${plural(failed, "step")} couldn't run`
                : statusLine(rows, finished)}
          </div>
          <div
            className="bar"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={pct}
            aria-label="Startup check"
          >
            <span style={{ width: `${pct}%` }} />
          </div>
          <div className="su-bar-row">
            <span>{detail}</span>
            <span>{pct}%</span>
          </div>
          <div className="su-steps">
            {rows.map((r, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: a row is a position in the check's fixed step list (StartupStep.step)
              <div className="su-step" key={i}>
                <StepIcon state={r.state} />
                <span className="grow">
                  <b>{r.label}</b>
                  <span className="sub">{r.detail}</span>
                </span>
              </div>
            ))}
          </div>
          {failure && <p className="sub">{failure}</p>}
        </section>

        <div className="su-foot">
          <span>v0.1.0 · talks only to GameBanana</span>
          <Button variant="link" onClick={() => finish(report)}>
            {finished ? "Open GMM" : "Skip for now"}
          </Button>
        </div>
      </div>
    </div>
  );
}

function StepIcon({ state }: { state: Row["state"] }) {
  switch (state) {
    case "done":
      return (
        <span className="su-ic su-ic-done" role="img" aria-label="Done">
          <Icon name="check" size={13} stroke={4} />
        </span>
      );
    case "working":
      return <span className="su-ic su-ic-run" role="img" aria-label="Working" />;
    case "failed":
      return (
        <span className="su-ic su-ic-up" role="img" aria-label="Needs attention">
          !
        </span>
      );
    case "skipped":
      return (
        <span className="su-ic su-ic-skip" role="img" aria-label="Skipped">
          –
        </span>
      );
    default:
      return <span className="su-ic su-ic-wait" role="img" aria-label="Waiting" />;
  }
}
