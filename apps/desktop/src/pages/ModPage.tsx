// SPDX-License-Identifier: MIT OR Apache-2.0
// GMM's own mod page: a full page with Back, opened from a Store card,
// the featured gallery or the Library's mod settings. The slideshow, the header (author,
// category and character, version, dates, counters), the license checklist, and tabs for
// the description, the files (archived ones folded away), the update notes and the credits.
// Every Install button opens the install confirmation; "View on GameBanana" opens the
// mod's page in the browser after the native confirmation. Built from 07's sketch and the
// design tokens (no canvas round, decided 2026-10-25).

import { useCallback, useEffect, useState } from "react";
import { api, asCmdError, type ModFile, type ModProfile, type ModUpdate } from "../api";
import { isSensitive, type Owned } from "../catalog";
import { RichText } from "../components/RichText";
import { Slideshow } from "../components/Slideshow";
import { bytes, compact, day, plural } from "../format";
import { Badge, Button, Dialog, Icon, Notice, Spinner, useToast } from "../ui";

type Tab = "description" | "files" | "updates" | "credits";

export function ModPage({
  modId,
  owned,
  backLabel,
  onBack,
  onDownload,
  onManage,
}: {
  modId: number;
  owned: Owned | undefined;
  /** Where Back goes ("Mods Store"). */
  backLabel: string;
  onBack: () => void;
  /** The install confirmation, for GameBanana's newest file or a chosen one. */
  onDownload: (mod: { id: number; name: string }, fileId?: number) => void;
  /** The installed mod's settings in the Library. */
  onManage: () => void;
}) {
  const toast = useToast();
  const [profile, setProfile] = useState<ModProfile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("description");
  const [license, setLicense] = useState(false);

  const load = useCallback(() => {
    setError(null);
    api
      .catalogMod(modId)
      .then(setProfile)
      .catch((e) => setError(asCmdError(e).message));
  }, [modId]);

  useEffect(() => {
    setProfile(null);
    setTab("description");
    load();
  }, [load]);

  const back = (
    <button type="button" className="back-link" onClick={onBack}>
      <Icon name="arrowLeft" size={18} stroke={2.6} />
      {backLabel}
    </button>
  );

  if (error) {
    return (
      <>
        {back}
        <Notice
          tone="bad"
          title="Couldn't load this mod from GameBanana."
          actions={<Button onClick={load}>Try again</Button>}
        >
          {error}
        </Notice>
      </>
    );
  }
  if (!profile) {
    return (
      <>
        {back}
        <div className="card empty mp-loading">
          <Spinner /> Loading the mod page…
        </div>
      </>
    );
  }

  const m = profile.summary;
  const installable = !(profile.is_trashed || profile.is_withheld || m.obsolete);
  const why = profile.is_trashed
    ? "GameBanana removed this mod."
    : profile.is_withheld
      ? "GameBanana is holding this mod back for review."
      : m.obsolete
        ? "Its author marked this mod obsolete."
        : null;
  const place = [m.root_category?.name, m.category?.name].filter(Boolean).join(" › ");
  const fileCount = profile.files.files.length + profile.files.archived_files.length;
  const creditCount = profile.credits.reduce((n, g) => n + g.authors.length, 0);
  const openGb = () => api.openModPage(m.id).catch((e) => toast("bad", asCmdError(e).message));
  const lic = profile.license;

  return (
    <article className="mp" aria-labelledby="mp-title">
      {back}
      <Slideshow images={m.images} name={m.name} sensitive={isSensitive(m)} />

      <header className="mp-head">
        <div className="grow">
          {place && <div className="eyebrow">{place}</div>}
          <h1 className="mp-title" id="mp-title">
            {m.name}
          </h1>
          <div className="mp-by">
            {m.submitter.avatar_url ? (
              <img className="av" src={m.submitter.avatar_url} alt="" />
            ) : (
              <span className="av" aria-hidden="true">
                {m.submitter.name.slice(0, 1).toUpperCase()}
              </span>
            )}
            <span>
              by <b>{m.submitter.name}</b>
              {m.version && ` · v${m.version}`} · Added {day(m.date_added)}
              {m.date_updated !== null && ` · Updated ${day(m.date_updated)}`}
            </span>
          </div>
          <div className="mp-stats">
            <span title="Likes">
              <Icon name="heart" size={16} stroke={2.4} />
              {compact(m.stats.likes)} likes
            </span>
            {m.stats.downloads !== null && (
              <span title="Downloads">
                <Icon name="download" size={16} stroke={2.4} />
                {compact(m.stats.downloads)} downloads
              </span>
            )}
            <span title="Views">
              <Icon name="eye" size={16} stroke={2.4} />
              {compact(m.stats.views)} views
            </span>
            {isSensitive(m) && <Badge tone="warn">Marked sensitive</Badge>}
          </div>
        </div>
        <div className="mp-actions">
          {owned ? (
            <>
              <Badge tone={owned.on ? "ok" : "quiet"}>{owned.on ? "Installed" : "Installed · off"}</Badge>
              {owned.versions && (
                <Button variant="primary" icon="plus" disabled={!installable} onClick={() => onDownload(m)}>
                  Add versions
                </Button>
              )}
              <Button icon="sliders" onClick={onManage}>
                Mod settings
              </Button>
            </>
          ) : (
            <Button
              variant="primary"
              icon="download"
              disabled={!installable || profile.files.files.length === 0}
              title={why ?? (profile.files.files.length === 0 ? "No files to download" : undefined)}
              onClick={() => onDownload(m)}
            >
              Download
            </Button>
          )}
          <Button variant="link" icon="external" onClick={openGb}>
            View on GameBanana
          </Button>
        </div>
      </header>

      <div className="notices">
        {why && (
          <Notice tone="warn" title="Can't be installed.">
            {why}
          </Notice>
        )}
        {owned?.update && (
          <Notice
            tone="info"
            title="A newer file is out."
            actions={<Button onClick={onManage}>Update in Mod settings</Button>}
          >
            The startup check found it. Updating keeps your folder names and on/off choices.
          </Notice>
        )}
      </div>

      {(lic.yes.length > 0 || lic.ask.length > 0 || lic.no.length > 0 || profile.license_html.trim() !== "") && (
        <section className="mp-license" aria-label="License">
          <span className="eyebrow">License</span>
          {lic.yes.map((t) => (
            <Badge key={`y${t}`} tone="ok">
              ✓ {t}
            </Badge>
          ))}
          {lic.ask.map((t) => (
            <Badge key={`a${t}`} tone="warn">
              ? {t}
            </Badge>
          ))}
          {lic.no.map((t) => (
            <Badge key={`n${t}`} tone="bad">
              ✗ {t}
            </Badge>
          ))}
          {profile.license_html.trim() !== "" && (
            <Button variant="link" size="sm" onClick={() => setLicense(true)}>
              Read the license
            </Button>
          )}
        </section>
      )}

      <div className="tabs" role="tablist" aria-label="Mod details">
        {(
          [
            ["description", "Description"],
            ["files", `Files (${fileCount})`],
            ["updates", `Updates (${profile.updates_count})`],
            ["credits", `Credits (${creditCount})`],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            className="tab"
            id={`tab-${id}`}
            aria-selected={tab === id}
            aria-controls={`panel-${id}`}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      <section className="card panel mp-panel" role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`}>
        {tab === "description" &&
          (profile.description_html.trim() !== "" ? (
            <RichText html={profile.description_html} />
          ) : (
            <p className="sub">The author didn't write a description.</p>
          ))}
        {tab === "files" && (
          <FilesTab profile={profile} installable={installable} onInstall={(f) => onDownload(m, f.id)} />
        )}
        {tab === "updates" && <UpdatesTab modId={m.id} />}
        {tab === "credits" &&
          (profile.credits.length === 0 ? (
            <p className="sub">No credits listed beyond the author.</p>
          ) : (
            <div className="mp-credits">
              {profile.credits.map((g) => (
                <div key={g.name}>
                  <h3 className="sec-h">{g.name}</h3>
                  <ul>
                    {g.authors.map((a) => (
                      <li key={`${a.name}${a.role ?? ""}`}>
                        <b>{a.name}</b>
                        {a.role && <span className="sub"> · {a.role}</span>}
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          ))}
      </section>

      {license && (
        <Dialog eyebrow="License" title={m.name} onClose={() => setLicense(false)}>
          <RichText html={profile.license_html} />
        </Dialog>
      )}
    </article>
  );
}

function FilesTab({
  profile,
  installable,
  onInstall,
}: {
  profile: ModProfile;
  installable: boolean;
  onInstall: (f: ModFile) => void;
}) {
  const { files, archived_files } = profile.files;
  if (files.length + archived_files.length === 0) return <p className="sub">GameBanana lists no files.</p>;
  return (
    <div className="mp-files">
      {[...files]
        .sort((a, b) => b.date_added - a.date_added)
        .map((f) => (
          <FileRow key={f.id} file={f} installable={installable} onInstall={() => onInstall(f)} />
        ))}
      {archived_files.length > 0 && (
        <details className="mp-archived">
          <summary>Archived files ({archived_files.length})</summary>
          {archived_files.map((f) => (
            <FileRow key={f.id} file={f} installable={installable} onInstall={() => onInstall(f)} />
          ))}
        </details>
      )}
    </div>
  );
}

function FileRow({ file, installable, onInstall }: { file: ModFile; installable: boolean; onInstall: () => void }) {
  const clean = file.av_result === "clean";
  return (
    <div className="mp-file">
      <div className="grow">
        <div className="mp-file-name">
          <b className="mono">{file.name}</b>
          {file.version && <Badge tone="quiet">v{file.version}</Badge>}
          {clean ? (
            <Badge tone="ok">Scanned clean</Badge>
          ) : (
            <Badge tone="warn">{file.av_result ? `Scan: ${file.av_result}` : "Not scanned yet"}</Badge>
          )}
        </div>
        <div className="sub">
          {bytes(file.size)} · {day(file.date_added)} · {plural(file.download_count, "download")}
        </div>
        {file.description && <p className="mp-file-desc">{file.description}</p>}
      </div>
      <Button
        size="sm"
        icon="download"
        disabled={!installable || !clean}
        title={!clean ? "Only files GameBanana scanned as clean can be installed" : undefined}
        onClick={onInstall}
      >
        Install
      </Button>
    </div>
  );
}

function UpdatesTab({ modId }: { modId: number }) {
  const [items, setItems] = useState<ModUpdate[]>([]);
  const [page, setPage] = useState(0);
  const [last, setLast] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const more = useCallback(
    (next: number) => {
      setBusy(true);
      setError(null);
      api
        .catalogUpdates(modId, next)
        .then((p) => {
          setItems((all) => (next === 1 ? p.items : [...all, ...p.items]));
          setPage(next);
          setLast(p.is_last || p.items.length === 0);
        })
        .catch((e) => setError(asCmdError(e).message))
        .finally(() => setBusy(false));
    },
    [modId],
  );

  useEffect(() => more(1), [more]);

  if (error) {
    return (
      <Notice
        tone="bad"
        title="Couldn't load the update notes."
        actions={<Button onClick={() => more(page + 1)}>Try again</Button>}
      >
        {error}
      </Notice>
    );
  }
  if (page === 0) {
    return (
      <p className="sub">
        <Spinner /> Loading…
      </p>
    );
  }
  if (items.length === 0) return <p className="sub">No update notes yet.</p>;
  return (
    <div className="mp-updates">
      {items.map((u) => (
        <div key={u.id} className="mp-update">
          <div className="mp-file-name">
            <b>{u.title}</b>
            {u.version && <Badge tone="quiet">v{u.version}</Badge>}
            <span className="sub">{day(u.date_added)}</span>
          </div>
          {u.changelog.length > 0 && (
            <ul className="mp-changes">
              {u.changelog.map((c, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: changelog lines have no ids
                <li key={i}>
                  {c.category && <Badge tone="quiet">{c.category}</Badge>} {c.text}
                </li>
              ))}
            </ul>
          )}
          {u.text_html.trim() !== "" && <RichText html={u.text_html} />}
        </div>
      ))}
      {!last && (
        <Button disabled={busy} onClick={() => more(page + 1)}>
          {busy ? "Loading…" : "Show older updates"}
        </Button>
      )}
    </div>
  );
}
