// SPDX-License-Identifier: MIT OR Apache-2.0
// GMM's version as people read it ("0.0.1 beta"; the build's version is 0.0.1-beta) and the
// update notice's action. GMM never installs updates itself: it opens the release page on
// GitHub (after a native confirmation), where the new installer is.

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
