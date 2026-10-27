// SPDX-License-Identifier: MIT OR Apache-2.0
// GMM's version as people read it ("0.0.1 beta"; the build's version is 0.0.1-beta) and getting
// an update. On Windows GMM installs it itself: Rust downloads the installer, checks its
// signature, starts Windows Installer and closes GMM, which reopens when it's done
// (`useAppUpdate`). Elsewhere the release page opens on GitHub (after a native confirmation).

import { useCallback, useState } from "react";
import { api, asCmdError } from "./api";

/** "0.0.1-beta" → "0.0.1 beta". */
export function versionLabel(version: string): string {
  return version.replace("-", " ");
}

/** This build's version, as people read it. */
export const APP_VERSION = versionLabel(__APP_VERSION__);

/** Opens the found release's page; reports a failure through `onError`. */
export function openRelease(version: string, onError: (message: string) => void): void {
  api.openAppRelease(version).catch((e) => onError(asCmdError(e).message));
}

/** Where an update GMM installs itself stands. */
export type UpdateRun =
  | { state: "idle" }
  | { state: "downloading"; downloaded: number; total: number | null }
  | { state: "installing" }
  | { state: "failed"; message: string };

/** How far along an update is, 0–100 (installing counts as done downloading). */
export function updatePercent(run: UpdateRun): number {
  if (run.state === "installing") return 100;
  if (run.state !== "downloading" || !run.total) return 0;
  return Math.min(100, Math.round((run.downloaded / run.total) * 100));
}

/** Installs `version` (the one the last check found) and tracks it. GMM closes once Windows
 *  Installer starts, so success is never reported back: the app is gone by then. */
export function useAppUpdate(): [UpdateRun, (version: string) => void] {
  const [run, setRun] = useState<UpdateRun>({ state: "idle" });
  const start = useCallback((version: string) => {
    setRun({ state: "downloading", downloaded: 0, total: null });
    api
      .appUpdateInstall(version, (p) =>
        setRun(
          p.phase === "installing"
            ? { state: "installing" }
            : { state: "downloading", downloaded: p.downloaded, total: p.total },
        ),
      )
      .catch((e) => setRun({ state: "failed", message: asCmdError(e).message }));
  }, []);
  return [run, start];
}
