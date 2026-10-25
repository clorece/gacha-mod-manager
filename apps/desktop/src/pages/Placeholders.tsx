// SPDX-License-Identifier: MIT OR Apache-2.0
// Pages whose screens are built in Phase 5 (docs/05-roadmap.md). Each says what it will hold,
// so the sidebar is complete now; Report a Bug is the designed work-in-progress page.

import logo from "../assets/gmm-logo.svg";
import { Button } from "../ui";

const upcoming = {
  downloads: {
    title: "Downloads",
    line: "Active and recent installs, grouped by game. Arrives with the install queue.",
  },
  settings: {
    title: "Settings",
    line: "Appearance, the Mods Store, the XXMI folder, your games and storage. Stored on this computer only.",
  },
  store: {
    title: "Mods Store",
    line: "Trending mods and the GameBanana catalogue for the selected game, with search and filters.",
  },
  docs: {
    title: "Documentation",
    line: "How GMM works, and what to do when something doesn't.",
  },
} as const;

export function ComingSoon({ page }: { page: keyof typeof upcoming }) {
  const { title, line } = upcoming[page];
  return (
    <>
      <div className="page-head">
        <div>
          <h1>{title}</h1>
          <p>{line}</p>
        </div>
      </div>
      <div className="card empty">
        This screen is being built next. Everything here works from the gmm command line today.
      </div>
    </>
  );
}

export function ReportBug() {
  return (
    <div className="wip-wrap">
      <img className="wip-logo" src={logo} alt="" />
      <h1>Report a Bug</h1>
      <span className="wip-chip">IN DEVELOPMENT</span>
      <p>
        Coming after the first beta. You'll describe what went wrong and can attach an error or crash log, which you'll
        see in full before anything is sent.
      </p>
      <div className="card panel text-left" aria-hidden="true">
        <span className="fake-field">What happened?</span>
        <span className="fake-field">Attach a log (optional)</span>
        <div className="mt-3.5 flex justify-end">
          <Button variant="primary" disabled tabIndex={-1}>
            Send report
          </Button>
        </div>
      </div>
    </div>
  );
}
