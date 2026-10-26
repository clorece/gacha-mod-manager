// SPDX-License-Identifier: MIT OR Apache-2.0
// The notice for when GameBanana is limiting GMM (docs/gamebanana-api.md): it asked GMM
// to slow down, or answers couldn't be had and saved ones are showing. `network_status` is a
// local question (no request to GameBanana), asked every few seconds.

import { useEffect, useState } from "react";
import { api, type NetworkStatus } from "../api";
import { Notice } from "../ui";

const EVERY_MS = 5000;

export function NetworkNotice() {
  const [status, setStatus] = useState<NetworkStatus | null>(null);

  useEffect(() => {
    let live = true;
    const ask = () =>
      api
        .networkStatus()
        .then((s) => live && setStatus(s))
        .catch(() => {});
    void ask();
    const timer = setInterval(ask, EVERY_MS);
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, []);

  if (!status) return null;
  if (status.cooldown_secs !== null) {
    const secs = status.cooldown_secs;
    const when = secs >= 120 ? `about ${Math.ceil(secs / 60)} minutes` : `${secs} s`;
    return (
      <Notice tone="warn" title="GameBanana asked GMM to slow down.">
        Too many requests came from this network. GMM shows what it saved and tries again in {when}; installs in the
        queue carry on by themselves.
      </Notice>
    );
  }
  if (status.showing_saved) {
    return (
      <Notice tone="info" title="Showing saved GameBanana data.">
        GameBanana couldn't be reached just now, so some pages show what GMM saved earlier.
      </Notice>
    );
  }
  return null;
}
