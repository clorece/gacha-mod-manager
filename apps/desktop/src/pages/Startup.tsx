// SPDX-License-Identifier: MIT OR Apache-2.0
// The startup check: a full-window screen while GMM pulls the
// newest catalogue pages and checks the installed mods for updates (`startup_check`). The
// app works offline, so a failing step is shown and the app opens anyway; Skip for now
// opens it at once while the check finishes in the background. When a newer GMM is on GitHub
// it updates itself where it can (Windows), like Discord: the bar follows the download, then
// Windows Installer takes over and GMM reopens. If that fails, or elsewhere, the screen stays
// up with its release page (Get the update) and Later.

import { useEffect, useRef, useState } from "react";
import { api, asCmdError, type GameInfo, type StartupReport, type StartupStep, type StepState } from "../api";
import logo from "../assets/gmm-logo.svg";
import { bytes, plural } from "../format";
import { Button, Icon, useToast } from "../ui";
import { APP_VERSION, openRelease, updatePercent, useAppUpdate, versionLabel } from "../version";

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
  const toast = useToast();
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
    // A new GMM waits for the user (Get the update / Later); otherwise the app opens by itself.
    if (!finished || report?.app_update) return;
    const timer = setTimeout(() => finishRef.current(report), OPEN_AFTER_MS);
    return () => clearTimeout(timer);
  }, [finished, report]);

  const appUpdate = report?.app_update ?? null;
  const [run, startUpdate] = useAppUpdate();
  // A new GMM that can install itself starts at once, once.
  const started = useRef(false);
  useEffect(() => {
    if (!appUpdate?.installable || started.current) return;
    started.current = true;
    startUpdate(appUpdate.version);
  }, [appUpdate, startUpdate]);
  const selfUpdate = appUpdate?.installable === true;
  // From the moment it's found (the download starts on the next tick) until it fails.
  const updating = selfUpdate && run.state !== "failed";
  const updateFailed = selfUpdate && run.state === "failed";

  const settled = rows.filter((r) => r.state === "done" || r.state === "failed").length;
  const failed = rows.filter((r) => r.state === "failed").length;
  const updates = report?.updates.length ?? 0;
  const working = rows.find((r) => r.state === "working");
  let pct = finished ? 100 : Math.round((settled / Math.max(rows.length, 1)) * 100);
  let detail = working?.label.replace(/ mods$/, "") ?? `GMM ${APP_VERSION}`;
  if (finished) {
    detail = updates > 0 ? `${plural(updates, "mod update")} found` : "Opening GMM";
  }
  if (updating) {
    pct = updatePercent(run);
    detail =
      run.state === "downloading" && run.total
        ? `${bytes(run.downloaded)} of ${bytes(run.total)}`
        : run.state === "installing"
          ? "Checked and signed"
          : "Starting the download";
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
            {updating
              ? "Updating GMM"
              : updateFailed
                ? "GMM couldn’t update"
                : appUpdate
                  ? "New software update available"
                  : finished && failed === 0 && !failure
                    ? "You’re up to date"
                    : "Checking for updates"}
          </h1>
          <div
            className={`su-status${updating ? "" : finished ? (appUpdate || failure || failed ? " is-warn" : " is-done") : ""}`}
          >
            {(!finished || updating) && <span className="spin" aria-hidden="true" />}
            {updating && appUpdate
              ? run.state === "installing"
                ? "Installing; GMM closes and reopens by itself"
                : `Downloading GMM ${versionLabel(appUpdate.version)}`
              : updateFailed
                ? run.message
                : appUpdate
                  ? `GMM ${versionLabel(appUpdate.version)} is out; you have ${APP_VERSION}`
                  : failure
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
            aria-label={updating ? "Update" : "Startup check"}
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
          {appUpdate && updating && (
            <div className="su-update">
              <p>
                GMM checks the installer is really from GMM before it runs. Windows may ask you to allow it; your mods
                and settings stay as they are.
              </p>
            </div>
          )}
          {appUpdate && !updating && (
            <div className="su-update">
              <p>
                {updateFailed
                  ? `${appUpdate.name ?? `GMM ${versionLabel(appUpdate.version)}`} couldn't install by itself. Its page on GitHub has the installer; your mods and settings stay as they are.`
                  : `${appUpdate.name ?? `GMM ${versionLabel(appUpdate.version)}`} is on GitHub. Its page has the new installer and what changed; your mods and settings stay as they are.`}
              </p>
              <div className="su-update-actions">
                <Button onClick={() => finish(report)}>Later</Button>
                <Button
                  variant="primary"
                  icon="download"
                  onClick={() => openRelease(appUpdate.version, (m) => toast("bad", m))}
                >
                  Get the update
                </Button>
              </div>
            </div>
          )}
        </section>

        <div className="su-foot">
          <span>v{APP_VERSION} · talks only to GameBanana, and to GitHub for GMM updates</span>
          {!updating && (
            <Button variant="link" onClick={() => finish(report)}>
              {finished ? "Open GMM" : "Skip for now"}
            </Button>
          )}
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
    default:
      return <span className="su-ic su-ic-wait" role="img" aria-label="Waiting" />;
  }
}
