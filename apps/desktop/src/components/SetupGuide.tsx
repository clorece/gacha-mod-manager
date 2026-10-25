// SPDX-License-Identifier: MIT OR Apache-2.0
// The setup guide (docs/07, "Setup & onboarding"): four steps in XXMI Launcher, text only,
// opened from a game's setup card. It starts at the step the game's setup state points to,
// and "Check again" re-runs detection and moves on when a step got done. Quoted names are
// XXMI's own labels. XXMI itself asks for the game settings mods need (for example HD
// instead of UHD for a Steam copy) when it installs and starts a mod loader, so the last
// step is "follow its prompts" rather than a list of settings.

import { type ReactNode, useState } from "react";
import { api, asCmdError, type GameStatus, type SetupState } from "../api";
import { Button, Dialog, Icon, Notice, useToast } from "../ui";

/** The step a setup state needs; 4 means every step is done (Ready). */
export function guideStep(state: SetupState): number {
  switch (state) {
    case "xxmi_not_found":
      return 0;
    case "importer_missing":
      return 1;
    case "importer_broken":
      return 2;
    case "ready":
      return 4;
    default:
      // Game folder or launch setting: XXMI's own prompts and settings fix these.
      return 3;
  }
}

const TITLES = ["Install XXMI Launcher", "Select the game", "Install the mod loader", "Follow XXMI's prompts"];

export function SetupGuide({
  status,
  onClose,
  onRecheck,
}: {
  status: GameStatus;
  onClose: () => void;
  /** Re-runs detection; resolves once the statuses are fresh. */
  onRecheck: () => Promise<GameStatus | undefined>;
}) {
  const toast = useToast();
  const [current, setCurrent] = useState(status);
  const needed = guideStep(current.state);
  const [step, setStep] = useState(Math.min(needed, 3));
  const [busy, setBusy] = useState(false);
  const game = current.game_name;
  const importer = current.importer;

  const recheck = async () => {
    setBusy(true);
    try {
      const fresh = await onRecheck();
      if (fresh) {
        setCurrent(fresh);
        const next = guideStep(fresh.state);
        if (next > step) setStep(Math.min(next, 3));
        toast(fresh.state === "ready" ? "ok" : "info", fresh.state === "ready" ? `${game} is ready.` : fresh.summary);
      }
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const act = async (action: () => Promise<unknown>) => {
    try {
      await action();
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  };

  const body = [
    <>
      <p>
        XXMI Launcher loads mods into the game. GMM finds your games and mods, but the game always starts through XXMI.
      </p>
      <ol className="guide-list">
        <li>Download XXMI Launcher from its official releases page on GitHub.</li>
        <li>
          Install it with the <b>.msi</b>, or unpack the portable <b>.zip</b> somewhere outside Program Files.
        </li>
        <li>Start it once. GMM finds it on its own.</li>
      </ol>
      <div className="flex flex-wrap gap-2">
        <Button icon="download" onClick={() => act(api.openXxmiReleases)}>
          Open the releases page
        </Button>
        <Button icon="folder" onClick={() => act(api.pickXxmiFolder)}>
          It's somewhere else…
        </Button>
      </div>
    </>,
    <>
      <p>XXMI handles several games. Tell it which ones you want mods for.</p>
      <ol className="guide-list">
        <li>Open XXMI Launcher.</li>
        <li>
          Under <b>"Select Games To Mod:"</b>, tick <b>{game}</b> ({importer}).
        </li>
      </ol>
    </>,
    <>
      <p>
        {importer} is the part of XXMI that loads {game} mods.
      </p>
      <ol className="guide-list">
        <li>
          With {game} selected, press <b>"Install"</b>.
        </li>
        <li>Wait until XXMI says it's done.</li>
      </ol>
      {current.state === "importer_broken" && (
        <Notice tone="warn" title={`${importer} looks changed.`}>
          Installing it again over the old one fixes this.
        </Notice>
      )}
    </>,
    <>
      <p>XXMI checks the game's settings and asks for any change mods need. Say yes to its prompts.</p>
      <ol className="guide-list">
        <li>Follow what XXMI asks, for example a different graphics setting or where the game is installed.</li>
        <li>
          Press <b>"Start"</b> once so XXMI applies everything.
        </li>
        <li>From then on, start {game} with GMM's Launch button (or from XXMI) and your mods load.</li>
      </ol>
      {needed === 3 && (
        <Notice tone="warn" title="What GMM sees now">
          {current.summary}
        </Notice>
      )}
    </>,
  ];

  return (
    <GuideDialog
      game={game}
      step={step}
      needed={needed}
      onStep={setStep}
      onClose={onClose}
      busy={busy}
      onRecheck={recheck}
    >
      {body[step]}
    </GuideDialog>
  );
}

function GuideDialog({
  game,
  step,
  needed,
  onStep,
  onClose,
  busy,
  onRecheck,
  children,
}: {
  game: string;
  step: number;
  needed: number;
  onStep: (step: number) => void;
  onClose: () => void;
  busy: boolean;
  onRecheck: () => void;
  children: ReactNode;
}) {
  const ready = needed === 4;
  return (
    <Dialog
      eyebrow="Setup guide"
      title={`Set up ${game}`}
      subtitle={ready ? "Everything is set. Mods load when the game starts through XXMI." : `Step ${step + 1} of 4`}
      onClose={onClose}
      footer={
        <>
          <Button icon="refresh" disabled={busy} onClick={onRecheck}>
            Check again
          </Button>
          <span className="grow" />
          {step > 0 && <Button onClick={() => onStep(step - 1)}>Back</Button>}
          {step < 3 ? (
            <Button variant="primary" onClick={() => onStep(step + 1)}>
              Next
            </Button>
          ) : (
            <Button variant="primary" onClick={onClose}>
              Done
            </Button>
          )}
        </>
      }
    >
      <ol className="stepper" aria-label="Steps">
        {TITLES.map((title, i) => {
          const done = i < needed;
          return (
            <li key={title} className={`stepper-item${i === step ? " is-current" : ""}${done ? " is-done" : ""}`}>
              <button type="button" onClick={() => onStep(i)} aria-current={i === step ? "step" : undefined}>
                <span className="stepper-n" aria-hidden="true">
                  {done ? <Icon name="check" size={13} stroke={4} /> : i + 1}
                </span>
                <span className="stepper-t">
                  {title}
                  <span className="sr-only">{done ? " (done)" : ""}</span>
                </span>
              </button>
            </li>
          );
        })}
      </ol>
      <div className="guide-body">
        <h3 className="sec-h">{TITLES[step]}</h3>
        {children}
      </div>
    </Dialog>
  );
}
