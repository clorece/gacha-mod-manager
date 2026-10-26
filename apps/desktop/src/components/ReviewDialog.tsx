// SPDX-License-Identifier: MIT OR Apache-2.0
// The review of an install that needs action (docs/08, "Downloads"). The background check
// stopped it and says why: the file holds several versions to choose from, it has files GMM
// never installs, odd files worth a look, or it's an outdated file. The user goes ahead
// (`queue_proceed`, with the versions they picked) or cancels the download.

import { useState } from "react";
import { api, asCmdError, type CheckIssue, type Job, type Review, type VersionPlan } from "../api";
import { bytes, plural } from "../format";
import { Button, Checkbox, Dialog, Notice, useToast } from "../ui";

/** One short phrase per issue, for the Downloads row. */
export function issueSummary(review: Review): string {
  const words: Record<CheckIssue["type"], string> = {
    choose_versions: "choose versions",
    blocked: "unsafe files",
    warnings: "odd files",
    outdated: "outdated file",
  };
  return review.issues.map((i) => words[i.type]).join(", ");
}

/** For each picked version, in list order: on, or off because it clashes with an earlier
 *  version that's on (as the install settles them). */
export function versionTags(review: Review, picked: string[], switchOn: boolean): Map<string, string> {
  const on: number[] = [];
  const tags = new Map<string, string>();
  review.versions.forEach((v, i) => {
    if (!picked.includes(v.name)) return;
    const clash = on.find((j) =>
      review.version_conflicts.some((c) => (c.a === i && c.b === j) || (c.a === j && c.b === i)),
    );
    if (clash !== undefined) {
      tags.set(v.name, `Off · clashes with ${review.versions[clash].name}`);
    } else {
      on.push(i);
      if (switchOn) tags.set(v.name, "On after install");
    }
  });
  return tags;
}

export function ReviewDialog({ job, review, onClose }: { job: Job; review: Review; onClose: () => void }) {
  const toast = useToast();
  const choose = review.issues.some((i) => i.type === "choose_versions");
  const choosable = review.versions.filter((v) => v.installed === null);
  const [picked, setPicked] = useState<string[]>(() => choosable.map((v) => v.name));
  const [busy, setBusy] = useState(false);

  const chosen: VersionPlan[] = choose ? review.versions.filter((v) => picked.includes(v.name)) : review.versions;
  const label = (v: VersionPlan, item: string) => (review.versions.length > 1 ? `${v.name}: ${item}` : item);
  const blocked = chosen.flatMap((v) => v.blocked.map((b) => label(v, b)));
  const warnings = chosen.flatMap((v) => v.warnings.map((w) => label(v, w)));
  const outdated = review.issues.find((i) => i.type === "outdated");
  const tags = versionTags(review, picked, job.switch_on);
  const allPicked = choosable.length > 0 && choosable.every((v) => picked.includes(v.name));

  const act = async (what: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await what();
      onClose();
    } catch (e) {
      toast("bad", asCmdError(e).message);
      setBusy(false);
    }
  };
  const proceed = () => act(() => api.queueProceed(job.id, choose ? { type: "named", names: picked } : null));
  const cancel = () => act(() => api.queueCancel(job.id));

  return (
    <Dialog
      eyebrow="Needs your attention"
      title={job.mod_name}
      subtitle={[job.author && `by ${job.author}`, job.file_name].filter(Boolean).join(" · ") || undefined}
      media={job.image ? <img className="thumb cf-thumb" src={job.image} alt="" /> : undefined}
      onClose={onClose}
      footer={
        <>
          <Button variant="danger" onClick={cancel} disabled={busy}>
            Cancel download
          </Button>
          <Button
            variant="primary"
            icon="download"
            onClick={proceed}
            disabled={busy || (choose && picked.length === 0)}
          >
            {choose
              ? picked.length === 0
                ? "Pick a version"
                : `Install ${plural(picked.length, "version")}`
              : "Install anyway"}
          </Button>
        </>
      }
    >
      <p className="sub">
        GMM downloaded this file and checked every file in it. Nothing is installed yet: look at what it found, then go
        ahead or cancel.
      </p>

      {outdated?.type === "outdated" && (
        <Notice tone="warn" title="Outdated file.">
          This file is in the mod's old files on GameBanana
          {outdated.newest ? `; the newest is ${outdated.newest}` : ""}. It may not work with the game as it is now.
        </Notice>
      )}

      {blocked.length > 0 && (
        <Notice tone="warn" title={`Unsafe files: ${plural(blocked.length, "file")} won't be installed.`}>
          GMM never installs programs, scripts or shortcuts from a mod. Installing goes on without them; they stay only
          in GMM's download cache and never run.
          <ul className="rv-list">
            {blocked.map((b) => (
              <li key={b}>{b}</li>
            ))}
          </ul>
        </Notice>
      )}

      {warnings.length > 0 && (
        <Notice tone="info" title={`GMM noticed ${plural(warnings.length, "odd file")}.`}>
          They're installed with the mod. Look at them if it doesn't show up right in game.
          <ul className="rv-list">
            {warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        </Notice>
      )}

      {choose && (
        <>
          <div className="cf-h-row">
            <h3 className="cf-h">Choose versions</h3>
            <span className="count">
              {picked.length} of {choosable.length} selected
            </span>
            <span className="grow" />
            {choosable.length > 1 && (
              <Button variant="link" onClick={() => setPicked(allPicked ? [] : choosable.map((v) => v.name))}>
                {allPicked ? "Clear" : "Select all"}
              </Button>
            )}
          </div>
          <p className="sub">
            This file holds {review.versions.length} versions. Some mods come in pieces that work together, so every
            version you pick is switched on, except one that replaces the same things as another.
          </p>
          <div className="opt-list cf-vlist">
            {review.versions.map((v) => {
              const done = v.installed !== null;
              const tag = tags.get(v.name);
              return (
                <Checkbox
                  key={v.folder}
                  className="opt-row"
                  checked={done || picked.includes(v.name)}
                  disabled={done || busy}
                  onChange={(on) => setPicked((now) => (on ? [...now, v.name] : now.filter((n) => n !== v.name)))}
                >
                  <span className="grow">
                    <b className="block font-extrabold">{v.name}</b>
                    <span className="sub">
                      {plural(v.files, "file")} · {bytes(v.bytes)}
                    </span>
                  </span>
                  {tag && <span className={tag.startsWith("On") ? "tag-on" : "tag-off"}>{tag}</span>}
                  {done && <span className="tag-done">Installed</span>}
                </Checkbox>
              );
            })}
          </div>
        </>
      )}
    </Dialog>
  );
}
