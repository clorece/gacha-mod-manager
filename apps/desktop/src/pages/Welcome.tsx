// SPDX-License-Identifier: MIT OR Apache-2.0
// First run: the user's preferences, once, right after the first startup check. Theme and
// sensitive mods only (the user asked for nothing else here); each one can be changed later
// in Settings, and sensitive mods also from the Store and Library filters. Not on the design
// canvas; built from the startup screen's layout and the shared controls.

import { useState } from "react";
import type { ThemeChoice } from "../api";
import logo from "../assets/gmm-logo.svg";
import { usePrefs } from "../prefs";
import { Button, Segmented, Switch } from "../ui";

export function Welcome() {
  const { prefs, update } = usePrefs();
  const [busy, setBusy] = useState(false);

  const finish = async () => {
    setBusy(true);
    // The app opens as soon as this is saved (App.tsx checks `onboarded`).
    await update({ onboarded: true });
  };

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
        <section className="su-card" aria-labelledby="welcome-h">
          <h1 className="su-h" id="welcome-h">
            Before you start
          </h1>
          <p className="sub">Two quick choices. Nothing here is sent anywhere.</p>

          <div className="pref-row">
            <div className="grow">
              <b>Theme</b>
              <span className="sub">Light and dark follow the GMM palette.</span>
            </div>
            <Segmented<ThemeChoice>
              label="Theme"
              value={prefs.theme}
              onChange={(theme) => void update({ theme })}
              options={[
                { value: "dark", label: "Dark" },
                { value: "light", label: "Light" },
                { value: "system", label: "Match Windows" },
              ]}
            />
          </div>

          <div className="pref-row">
            <div className="grow">
              <b>Show mods marked as sensitive</b>
              <span className="sub">GameBanana flags these (NSFW). Hidden unless you turn this on.</span>
            </div>
            <Switch
              checked={prefs.show_sensitive}
              onChange={(show_sensitive) => void update({ show_sensitive })}
              label="Show mods marked as sensitive"
              showState
            />
          </div>

          <div className="pref-row">
            <div className="grow">
              <b>Blur their previews until hovered</b>
              <span className="sub">Only applies when sensitive mods are shown.</span>
            </div>
            <Switch
              checked={prefs.blur_sensitive}
              onChange={(blur_sensitive) => void update({ blur_sensitive })}
              label="Blur sensitive previews until hovered"
              disabled={!prefs.show_sensitive}
              showState
            />
          </div>

          <p className="hint">
            You can change these anytime: sensitive mods from the Mods Store and Mods Library filters, and everything in
            Settings.
          </p>
          <Button variant="primary" block disabled={busy} onClick={finish}>
            Continue
          </Button>
        </section>
      </div>
    </div>
  );
}
