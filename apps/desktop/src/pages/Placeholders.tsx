// SPDX-License-Identifier: MIT OR Apache-2.0
// Report a Bug: the designed work-in-progress page (docs/08). The real one (a pre-filled
// GitHub issue and a local error log) is Phase 6.

import logo from "../assets/gmm-logo.svg";
import { Button } from "../ui";

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
