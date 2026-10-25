// SPDX-License-Identifier: MIT OR Apache-2.0
// The install confirmation (docs/08, "Install confirmation"). It fetches the chosen file into
// GMM's download cache and looks inside (`prepare_install`), then shows what would happen:
// a file choice when GameBanana lists several, a version choice when the archive holds
// several mods, blocked files, where it installs. Nothing is installed until the user
// confirms; the confirmed install goes to the queue (`queue_install`) and the Downloads page.

import { useEffect, useState } from "react";
import {
  api,
  asCmdError,
  type InstallPlan,
  type InstallProgress,
  type ModFile,
  type ModProfile,
  type VariantPick,
} from "../api";
import { bytes, day, plural } from "../format";
import { Button, Checkbox, Dialog, Notice, ProgressBar, Radio, Switch, useToast } from "../ui";

type Loading = { kind: "loading"; progress: InstallProgress | null } | { kind: "failed"; message: string };

export function InstallDialog({
  modId,
  gameName,
  onClose,
  onQueued,
}: {
  modId: number;
  /** For "where it installs" ("Zenless Zone Zero › ZZMI\Mods\GMM"). */
  gameName: (gameId: string) => string;
  onClose: () => void;
  onQueued: () => void;
}) {
  const toast = useToast();
  const [profile, setProfile] = useState<ModProfile | null>(null);
  const [fileId, setFileId] = useState<number | null>(null);
  const [plan, setPlan] = useState<InstallPlan | null>(null);
  const [status, setStatus] = useState<Loading>({ kind: "loading", progress: null });
  const [picked, setPicked] = useState<string[]>([]);
  const [switchOn, setSwitchOn] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    api
      .catalogMod(modId)
      .then((p) => live && setProfile(p))
      .catch(() => {
        // The header falls back to the plan's name.
      });
    return () => {
      live = false;
    };
  }, [modId]);

  // (Re)look inside whenever the file changes. `null` = GameBanana's newest clean file.
  useEffect(() => {
    let live = true;
    setPlan(null);
    setStatus({ kind: "loading", progress: null });
    api
      .prepareInstall(modId, fileId, (p) => live && setStatus({ kind: "loading", progress: p }))
      .then((p) => {
        if (!live) return;
        setPlan(p);
        // Nothing ticked yet; a single-mod archive needs no choice.
        setPicked([]);
      })
      .catch((e) => live && setStatus({ kind: "failed", message: asCmdError(e).message }));
    return () => {
      live = false;
    };
  }, [modId, fileId]);

  // Kept while another file is being looked at, so the choice doesn't jump away.
  const [files, setFiles] = useState<ModFile[]>([]);
  useEffect(() => {
    if (plan) setFiles([plan.file, ...plan.other_files].sort((a, b) => b.date_added - a.date_added));
  }, [plan]);
  const chosenFile = plan?.file.id ?? fileId;
  const versions = plan?.versions ?? [];
  const multi = versions.length > 1;
  const choosable = versions.filter((v) => v.installed === null);
  const blocked = versions
    .filter((v) => !multi || picked.includes(v.name))
    .flatMap((v) => v.blocked.map((b) => (multi ? `${v.name}: ${b}` : b)));
  const clash = (plan?.version_conflicts.length ?? 0) > 0;
  const unusable = plan !== null && (plan.kind === null || versions.length === 0);
  const nothingNew = plan !== null && !unusable && choosable.length === 0;

  const toggle = (name: string, on: boolean) =>
    setPicked((current) => (on ? [...current, name] : current.filter((n) => n !== name)));
  const allPicked = choosable.length > 0 && choosable.every((v) => picked.includes(v.name));

  let installLabel = "Install";
  if (multi) {
    installLabel = picked.length === 0 ? "Pick a version" : `Install ${plural(picked.length, "version")}`;
  }
  if (blocked.length > 0 && (!multi || picked.length > 0)) installLabel += " without blocked files";
  const canInstall = plan !== null && !unusable && !nothingNew && (!multi || picked.length > 0) && !busy;

  const confirm = async () => {
    if (!plan) return;
    setBusy(true);
    const variants: VariantPick = multi ? { type: "named", names: picked } : { type: "only" };
    try {
      await api.queueInstall({
        mod_id: plan.mod_id,
        file_id: plan.file.id,
        variants,
        skip_blocked: blocked.length > 0,
        start_disabled: !switchOn,
      });
      onQueued();
    } catch (e) {
      toast("bad", asCmdError(e).message);
      setBusy(false);
    }
  };

  const viewOnGameBanana = () => {
    api.openModPage(modId).catch((e) => toast("bad", asCmdError(e).message));
  };

  const summary = profile?.summary;
  const banner = summary?.images[0];
  const thumb = banner?.variants.find((v) => v.size.width >= 100)?.url ?? banner?.url;
  const byline = [
    summary?.submitter.name && `by ${summary.submitter.name}`,
    summary?.category?.name,
    plan ? gameName(plan.game) : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <Dialog
      eyebrow={multi && versions.some((v) => v.installed !== null) ? "Add versions" : "Install"}
      title={summary?.name ?? plan?.mod_name ?? "Looking at the mod…"}
      subtitle={byline || undefined}
      media={thumb ? <img className="thumb cf-thumb" src={thumb} alt="" /> : <div className="thumb cf-thumb art a2" />}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          {status.kind === "failed" || unusable ? (
            <Button variant="primary" onClick={viewOnGameBanana}>
              View on GameBanana
            </Button>
          ) : (
            <Button variant="primary" icon="download" onClick={confirm} disabled={!canInstall}>
              {nothingNew ? "Already installed" : installLabel}
            </Button>
          )}
        </>
      }
    >
      {status.kind === "failed" && (
        <Notice tone="bad" title="GMM can't install this file.">
          {status.message}
        </Notice>
      )}

      {files.length > 1 && (
        <>
          <div className="cf-h-row">
            <h3 className="cf-h">Choose a file</h3>
            <span className="count">{files.length} files on GameBanana</span>
          </div>
          <div className="opt-list">
            {files.map((f) => (
              <Radio
                key={f.id}
                className="opt-row"
                name="file"
                checked={f.id === chosenFile}
                disabled={!plan}
                onChange={() => setFileId(f.id)}
              >
                <span className="grow">
                  <b className="block font-extrabold">{f.description ?? f.name}</b>
                  <span className="sub">
                    {f.description ? `${f.name} · ` : ""}
                    {bytes(f.size)} · {day(f.date_added)}
                  </span>
                </span>
              </Radio>
            ))}
          </div>
        </>
      )}

      {!plan && status.kind === "loading" && <Looking progress={status.progress} />}

      {unusable && (
        <Notice tone="bad" title={`GMM didn't find a ${plan ? gameName(plan.game) : ""} mod in this file.`}>
          Nothing in it looks like a mod XXMI loads (no mod .ini files), so there's nothing to install.
        </Notice>
      )}

      {plan && multi && (
        <>
          <div className="cf-h-row">
            <h3 className="cf-h">Choose versions</h3>
            <span className="count">
              {picked.length} of {choosable.length} selected
            </span>
            <span className="grow" />
            {choosable.length > 0 && (
              <Button variant="link" onClick={() => setPicked(allPicked ? [] : choosable.map((v) => v.name))}>
                {allPicked ? "Clear" : "Select all"}
              </Button>
            )}
          </div>
          <p className="sub">
            This archive holds {versions.length} versions. GMM looked inside after fetching it into its cache (
            {bytes(plan.archive_bytes)}); nothing is installed until you confirm.
          </p>
          <div className="opt-list cf-vlist">
            {versions.map((v) => {
              const done = v.installed !== null;
              const first = picked[0] === v.name;
              return (
                <Checkbox
                  key={v.folder}
                  className="opt-row"
                  checked={done || picked.includes(v.name)}
                  disabled={done}
                  onChange={(on) => toggle(v.name, on)}
                >
                  <span className="grow">
                    <b className="block font-extrabold">{v.name}</b>
                    <span className="sub">
                      {plural(v.files, "file")} · {bytes(v.bytes)}
                    </span>
                  </span>
                  {first && switchOn && <span className="tag-on">On after install</span>}
                  {done && <span className="tag-done">Installed</span>}
                </Checkbox>
              );
            })}
          </div>
          {clash && (
            <p className="hint">
              Some of these versions replace the same things, so only one of those can be on at a time. The first one
              you pick is switched on; switch between them later in the mod's settings.
            </p>
          )}
        </>
      )}

      {blocked.length > 0 && (
        <Notice tone="warn" title={`${plural(blocked.length, "file")} won't be installed.`}>
          GMM never installs programs, scripts or shortcuts from a mod: {blocked.join(", ")}.
        </Notice>
      )}

      {plan && (
        <div className="kv">
          <div className="kv-row">
            <span>File</span>
            <b>
              {plan.file.name} · {bytes(plan.file.size)}
              {plan.from_cache ? " · already in the download cache" : ""}
            </b>
          </div>
          <div className="kv-row">
            <span>Installs to</span>
            <b>
              {gameName(plan.game)} › {plan.install_to}
            </b>
          </div>
          <div className="kv-row">
            <span>Safety</span>
            <b>Scanned clean by GameBanana; GMM checked every file again</b>
          </div>
        </div>
      )}

      {plan && !unusable && !nothingNew && (
        <div className="cf-switch">
          <div className="grow">
            <b>Switch it on after installing</b>
            <div className="sub">Otherwise it's installed switched off.</div>
          </div>
          <Switch checked={switchOn} onChange={setSwitchOn} label="Switch it on after installing" />
        </div>
      )}
    </Dialog>
  );
}

function Looking({ progress }: { progress: InstallProgress | null }) {
  let line = "Getting the mod's files from GameBanana";
  let value: number | undefined;
  if (progress?.type === "using_cache") line = "Found it in GMM's download cache";
  if (progress?.type === "downloading") line = `Fetching ${bytes(progress.size)} into GMM's cache`;
  if (progress?.type === "progress") {
    const total = progress.total;
    line = total
      ? `Fetching into GMM's cache · ${Math.floor((progress.downloaded / total) * 100)}% · ${bytes(progress.downloaded)} of ${bytes(total)}`
      : `Fetching into GMM's cache · ${bytes(progress.downloaded)}`;
    value = total ? progress.downloaded / total : undefined;
  }
  if (progress?.type === "extracting") line = "Looking inside the archive";
  return (
    <div className="cf-looking">
      <p className="sub">
        GMM fetches the file into its download cache and looks inside before you choose. Nothing is installed until you
        confirm.
      </p>
      <div className="dl-state st-run">{line}</div>
      <ProgressBar value={value} label={line} />
    </div>
  );
}
