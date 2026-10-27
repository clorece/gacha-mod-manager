// SPDX-License-Identifier: MIT OR Apache-2.0
// Report a Bug: the user writes what went wrong, and the log file to attach is chosen for
// them (the newest; Change… picks another from the log folder in a native dialog). Rust
// writes the GitHub issue (their text, versions and setup states, never a path) and opens it
// in the browser after a native confirmation; GMM sends nothing itself. A link can't carry
// files, so Rust then shows the log folder for the user to drag the log into the issue.

import { useEffect, useId, useState } from "react";
import { api, asCmdError, type LogFile } from "../api";
import { ago, bytes } from "../format";
import { Button, Icon, Spinner, useToast } from "../ui";

export function ReportBug() {
  const toast = useToast();
  const titleId = useId();
  const textId = useId();
  const [title, setTitle] = useState("");
  const [text, setText] = useState("");
  /** `undefined` while it loads; `null` when nothing has been logged. */
  const [log, setLog] = useState<LogFile | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    api
      .reportLog()
      .then((f) => live && setLog(f))
      .catch(() => live && setLog(null));
    return () => {
      live = false;
    };
  }, []);

  const change = async () => {
    try {
      const picked = await api.pickReportLog();
      if (picked) setLog(picked);
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  };

  const send = async () => {
    setBusy(true);
    try {
      if (await api.openBugReport(title, text)) {
        toast(
          "ok",
          log
            ? `Opened in your browser. Drag ${log.name} from the log folder into the issue, then submit it.`
            : "Opened in your browser. Check it and submit it there.",
        );
      }
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Report a Bug</h1>
          <p>
            Tell us what went wrong. GMM fills in an issue on its GitHub page for you to check and submit; nothing is
            sent from GMM.
          </p>
        </div>
      </div>

      <div className="report">
        <section className="card panel report-form" aria-labelledby="rb-write">
          <h2 id="rb-write" className="card-title">
            Your report
          </h2>
          <label className="field" htmlFor={titleId}>
            <span>Title</span>
            <input
              id={titleId}
              className="field-input"
              value={title}
              maxLength={120}
              placeholder="Installing a RAR mod fails"
              onChange={(e) => setTitle(e.target.value)}
            />
          </label>
          <label className="field" htmlFor={textId}>
            <span>What happened?</span>
            <textarea
              id={textId}
              className="field-input"
              rows={7}
              value={text}
              placeholder={"What you did, what you expected, and what happened instead.\n1. …\n2. …"}
              onChange={(e) => setText(e.target.value)}
            />
          </label>
          <fieldset className="field report-field">
            <legend>Log file</legend>
            <div className="report-log">
              <Icon name="file" size={22} stroke={2.2} />
              <div className="report-log-text">
                {log === undefined ? (
                  <span className="sub">
                    <Spinner /> Finding the newest log
                  </span>
                ) : log === null ? (
                  <>
                    <b>No log yet</b>
                    <span className="sub">Nothing has been logged, so there's nothing to attach.</span>
                  </>
                ) : (
                  <>
                    <b>{log.name}</b>
                    <span className="sub">
                      {bytes(log.size)} · written {ago(log.modified)}
                    </span>
                  </>
                )}
              </div>
              <Button icon="folder" onClick={change}>
                Change…
              </Button>
            </div>
          </fieldset>
          <div className="report-actions">
            <Button variant="primary" icon="external" disabled={busy || title.trim() === ""} onClick={send}>
              Open on GitHub…
            </Button>
            <span className="sub">
              {title.trim() === "" ? "Add a title first. " : ""}GMM asks before your browser opens
              {log ? ", then shows the log folder so you can drag the log into the issue" : ""}. Submitting needs a
              GitHub account.
            </span>
          </div>
        </section>
      </div>
    </>
  );
}
