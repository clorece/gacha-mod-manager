// SPDX-License-Identifier: MIT OR Apache-2.0
// Installing from an archive on this computer (Phase 6): opened by a file dropped on the
// window or picked with "Install from a file". Rust holds the file; this dialog only knows
// its name. GMM copies it into its cache and looks inside for the chosen game, then shows the
// same choices as the install confirmation: versions, blocked files, on after installing.

import { useEffect, useState } from "react";
import { api, asCmdError, type GameInfo, type LocalPlan, type VariantPick } from "../api";
import { bytes, plural } from "../format";
import { Button, Checkbox, Dialog, Notice, ProgressBar, Radio, Switch, useToast } from "../ui";

export function LocalInstallDialog({
  fileName,
  games,
  defaultGame,
  onClose,
  onInstalled,
}: {
  fileName: string;
  games: GameInfo[];
  defaultGame: string;
  onClose: () => void;
  onInstalled: (game: string) => Promise<void>;
}) {
  const toast = useToast();
  const [game, setGame] = useState(defaultGame);
  const [plan, setPlan] = useState<LocalPlan | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [picked, setPicked] = useState<string[]>([]);
  const [switchOn, setSwitchOn] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    setPlan(null);
    setFailed(null);
    api
      .prepareLocal(game)
      .then((p) => {
        if (!live) return;
        setPlan(p);
        setPicked([]);
      })
      .catch((e) => live && setFailed(asCmdError(e).message));
    return () => {
      live = false;
    };
  }, [game]);

  const close = () => {
    void api.discardLocal().catch(() => {});
    onClose();
  };

  const versions = plan?.versions ?? [];
  const multi = versions.length > 1;
  const unusable = plan !== null && (plan.kind === null || versions.length === 0);
  const blocked = versions
    .filter((v) => !multi || picked.includes(v.name))
    .flatMap((v) => v.blocked.map((b) => (multi ? `${v.name}: ${b}` : b)));
  const taken = versions.filter((v) => v.installed !== null).map((v) => v.folder);
  const canInstall = plan !== null && !unusable && (!multi || picked.length > 0) && !busy;

  let label = multi ? (picked.length ? `Install ${plural(picked.length, "version")}` : "Pick a version") : "Install";
  if (blocked.length > 0 && (!multi || picked.length > 0)) label += " without blocked files";

  const install = async () => {
    setBusy(true);
    const variants: VariantPick = multi ? { type: "named", names: picked } : { type: "only" };
    try {
      const done = await api.installLocal(variants, blocked.length > 0, !switchOn);
      toast("ok", `Installed ${plural(done.length, "folder")} from ${fileName}.`);
      await onInstalled(game);
    } catch (e) {
      toast("bad", asCmdError(e).message);
      setBusy(false);
    }
  };

  return (
    <Dialog
      eyebrow="Install from a file"
      title={fileName}
      subtitle="Same checks as a download: GMM copies it into its cache, then extracts and scans it."
      onClose={close}
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" icon="download" disabled={!canInstall} onClick={install}>
            {label}
          </Button>
        </>
      }
    >
      {games.length > 1 && (
        <>
          <h3 className="cf-h">Which game is it for?</h3>
          <div className="opt-list">
            {games.map((g) => (
              <Radio
                key={g.id}
                className="opt-row"
                name="local-game"
                checked={g.id === game}
                onChange={() => setGame(g.id)}
              >
                <b className="grow font-extrabold">{g.name}</b>
              </Radio>
            ))}
          </div>
        </>
      )}

      {!plan && !failed && (
        <div className="cf-looking">
          <div className="dl-state st-run">Copying, extracting and scanning</div>
          <ProgressBar label="Looking inside the archive" />
        </div>
      )}
      {failed && (
        <Notice tone="bad" title="GMM can't install from this file.">
          {failed}
        </Notice>
      )}
      {unusable && (
        <Notice tone="bad" title="GMM didn't find a mod for this game in it.">
          Nothing in it looks like a mod XXMI loads for {games.find((g) => g.id === game)?.name}.
        </Notice>
      )}

      {plan && multi && (
        <>
          <div className="cf-h-row">
            <h3 className="cf-h">Choose versions</h3>
            <span className="count">
              {picked.length} of {versions.length} selected
            </span>
          </div>
          <div className="opt-list cf-vlist">
            {versions.map((v) => (
              <Checkbox
                key={v.folder}
                className="opt-row"
                checked={picked.includes(v.name)}
                onChange={(on) => setPicked((cur) => (on ? [...cur, v.name] : cur.filter((n) => n !== v.name)))}
              >
                <span className="grow">
                  <b className="block font-extrabold">{v.name}</b>
                  <span className="sub">
                    {plural(v.files, "file")} · {bytes(v.bytes)}
                  </span>
                </span>
                {picked[0] === v.name && switchOn && <span className="tag-on">On after install</span>}
              </Checkbox>
            ))}
          </div>
          {plan.version_conflicts.length > 0 && (
            <p className="hint">
              Some versions replace the same things; the first one you pick is switched on and the clashing ones off.
            </p>
          )}
        </>
      )}

      {blocked.length > 0 && (
        <Notice tone="warn" title={`${plural(blocked.length, "file")} won't be installed.`}>
          GMM never installs programs, scripts or shortcuts from a mod: {blocked.join(", ")}.
        </Notice>
      )}
      {taken.length > 0 && (
        <Notice tone="warn" title="A folder with this name is already installed.">
          {taken.join(", ")}. Uninstall it first, or rename the archive.
        </Notice>
      )}

      {plan && !unusable && (
        <>
          <div className="kv">
            <div className="kv-row">
              <span>File</span>
              <b>
                {plan.file_name} · {bytes(plan.archive_bytes)}
              </b>
            </div>
            <div className="kv-row">
              <span>Installs to</span>
              <b>
                {games.find((g) => g.id === plan.game)?.name} › {plan.install_to}
              </b>
            </div>
            <div className="kv-row">
              <span>Safety</span>
              <b>Not from GameBanana: GMM's own checks only</b>
            </div>
          </div>
          <div className="cf-switch">
            <div className="grow">
              <b>Switch it on after installing</b>
              <div className="sub">Otherwise it's installed switched off.</div>
            </div>
            <Switch checked={switchOn} onChange={setSwitchOn} label="Switch it on after installing" />
          </div>
        </>
      )}
    </Dialog>
  );
}
