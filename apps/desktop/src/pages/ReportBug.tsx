// SPDX-License-Identifier: MIT OR Apache-2.0
// Report a Bug (Phase 6, docs/07 "Report a Bug"): the user writes what went wrong, sees
// exactly what the GitHub issue will say (Rust writes it: their text, versions and setup
// states, never a path), and opens it in the browser after a native confirmation. GMM sends
// nothing itself. Below, the local error log (errors only, home folder shown as ~), to read
// and attach by hand: Open log folder, Clear log.

import { useCallback, useEffect, useId, useState } from "react";
import { api, asCmdError, type BugReport, type LogEntry } from "../api";
import { plural } from "../format";
import { Badge, Button, Notice, Spinner, useToast } from "../ui";

/** Entries the page shows (the file holds more). */
const SHOWN = 50;

function useDebounced<T>(value: T, ms: number): T {
  const [out, setOut] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setOut(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return out;
}

/** `25 Oct 2026, 21:03` in the user's time zone, from the log's UTC time. */
function when(utc: string): string {
  const d = new Date(utc);
  return Number.isNaN(d.getTime())
    ? utc
    : d.toLocaleString("en-GB", {
        day: "numeric",
        month: "short",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
}

export function ReportBug() {
  const toast = useToast();
  const titleId = useId();
  const textId = useId();
  const [title, setTitle] = useState("");
  const [text, setText] = useState("");
  const [draft, setDraft] = useState<BugReport | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);
  const [entries, setEntries] = useState<LogEntry[] | null>(null);
  const [busy, setBusy] = useState(false);
  const typedTitle = useDebounced(title, 350);
  const typedText = useDebounced(text, 350);

  useEffect(() => {
    let live = true;
    api
      .bugReportDraft(typedTitle, typedText)
      .then((d) => {
        if (!live) return;
        setDraft(d);
        setDraftError(null);
      })
      .catch((e) => live && setDraftError(asCmdError(e).message));
    return () => {
      live = false;
    };
  }, [typedTitle, typedText]);

  const loadLog = useCallback(async () => {
    try {
      setEntries(await api.errorLog(SHOWN));
    } catch (e) {
      toast("bad", asCmdError(e).message);
      setEntries([]);
    }
  }, [toast]);

  useEffect(() => {
    void loadLog();
  }, [loadLog]);

  const send = async () => {
    setBusy(true);
    try {
      if (await api.openBugReport(title, text)) toast("ok", "Opened in your browser. Check it and submit it there.");
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const clear = async () => {
    try {
      const n = await api.clearErrorLog();
      toast("ok", `${plural(n, "entry", "entries")} deleted.`);
      await loadLog();
    } catch (e) {
      toast("bad", asCmdError(e).message);
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
          <div className="report-actions">
            <Button variant="primary" icon="external" disabled={busy || title.trim() === ""} onClick={send}>
              Open on GitHub…
            </Button>
            <span className="sub">
              {title.trim() === "" ? "Add a title first. " : ""}GMM asks before your browser opens. Submitting needs a
              GitHub account.
            </span>
          </div>
        </section>

        <section className="card panel" aria-labelledby="rb-preview">
          <h2 id="rb-preview" className="card-title">
            What GitHub will get
          </h2>
          {draftError ? (
            <Notice tone="bad" title="Couldn't prepare the report.">
              {draftError}
            </Notice>
          ) : !draft ? (
            <p className="sub">
              <Spinner /> Preparing
            </p>
          ) : (
            <>
              {draft.shortened && (
                <Notice tone="warn" title="Your description was shortened to fit the link.">
                  Paste the rest into the issue on GitHub.
                </Notice>
              )}
              <div className="report-preview">
                <b>{draft.title}</b>
                <pre>{draft.body}</pre>
              </div>
              <p className="sub">
                Only this text goes into the link: what you wrote, GMM's and XXMI's versions and each game's setup
                state. Your home folder shows as ~. The error log isn't included; attach it yourself if it helps.
              </p>
            </>
          )}
        </section>

        <section className="card panel" aria-labelledby="rb-log">
          <div className="set-row">
            <h2 id="rb-log" className="card-title grow">
              Error log
            </h2>
            <Button icon="folder" onClick={() => api.openLogFolder().catch((e) => toast("bad", asCmdError(e).message))}>
              Open log folder
            </Button>
            <Button icon="trash" disabled={!entries || entries.length === 0} onClick={clear}>
              Clear log
            </Button>
          </div>
          <p className="sub">
            Errors only, kept on this computer, with your home folder shown as ~. To include it, attach errors.log from
            the log folder to the issue.
          </p>
          {entries === null ? (
            <p className="sub">
              <Spinner /> Reading the log
            </p>
          ) : entries.length === 0 ? (
            <p className="empty-line">No errors logged.</p>
          ) : (
            <ol className="log-list" aria-label={`The newest ${plural(entries.length, "error")}`}>
              {[...entries].reverse().map((e, i) => (
                <li key={`${e.time}-${i.toString()}`} className="log-row">
                  <span className="log-time">{when(e.time)}</span>
                  <Badge tone="quiet">{e.source}</Badge>
                  <span className="log-msg">{e.message}</span>
                </li>
              ))}
            </ol>
          )}
        </section>
      </div>
    </>
  );
}
