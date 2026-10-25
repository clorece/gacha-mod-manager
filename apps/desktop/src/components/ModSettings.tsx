// SPDX-License-Identifier: MIT OR Apache-2.0
// Mod settings: the side panel from a library card (docs/08, "Mod settings"). For now: the
// switch, the details and Uninstall with its itemized confirmation. Versions, Open folder,
// View on GameBanana and Reinstall arrive with Phase 5's commands.

import { useState } from "react";
import { api, asCmdError, type UninstallItem } from "../api";
import { bytes, plural } from "../format";
import type { LibraryItem } from "../pages/Library";
import { Button, Dialog, SidePanel, Switch, useToast } from "../ui";

export function ModSettings({
  item,
  onClose,
  onToggle,
  onRemoved,
}: {
  item: LibraryItem;
  onClose: () => void;
  onToggle: (on: boolean) => void;
  onRemoved: () => Promise<void>;
}) {
  const toast = useToast();
  const [preview, setPreview] = useState<UninstallItem[] | null>(null);
  const [busy, setBusy] = useState(false);

  const askUninstall = async () => {
    try {
      setPreview(await api.uninstallPreview(item.game.id, [item.key]));
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  };

  const uninstall = async () => {
    setBusy(true);
    try {
      const done = await api.uninstall(item.game.id, [item.key]);
      const trashed = done.some((d) => d.trashed);
      toast("ok", trashed ? `${item.name} moved to the Recycle Bin.` : `${item.name} removed from GMM's list.`);
      setPreview(null);
      await onRemoved();
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <SidePanel eyebrow="Mod settings" title={item.name} onClose={onClose}>
        <section className="drawer-sec">
          <p className="sub">
            {item.game.name} · {item.hand ? "Added by hand" : item.added}
          </p>
          <div className="flex items-center gap-3">
            <div className="grow">
              <b>Switched on</b>
              <div className="sub">
                {item.usable ? "XXMI loads it the next time the game starts." : `Can't switch: ${item.problem}.`}
              </div>
            </div>
            <Switch
              checked={item.enabled}
              onChange={onToggle}
              label={`${item.name} on or off`}
              disabled={!item.usable}
              showState
            />
          </div>
        </section>
        <section className="drawer-sec">
          <h3 className="sec-h">Details</h3>
          <div className="kv">
            {item.version && (
              <div className="kv-row">
                <span>Version</span>
                <b>{item.version}</b>
              </div>
            )}
            {item.size !== null && (
              <div className="kv-row">
                <span>Size</span>
                <b>{bytes(item.size)}</b>
              </div>
            )}
            <div className="kv-row">
              <span>Folder</span>
              <b className="mono">
                {item.game.importer}\Mods\{item.folder.replaceAll("/", "\\")}
              </b>
            </div>
          </div>
        </section>
        <section className="drawer-sec">
          <Button variant="danger" block icon="trash" onClick={askUninstall}>
            Uninstall
          </Button>
          <p className="sub">
            {item.hand
              ? "Moves the folder to the Recycle Bin. GMM shows exactly what goes first."
              : "Moves the mod to the Recycle Bin. The downloaded archive stays in GMM's cache."}
          </p>
        </section>
      </SidePanel>

      {preview && (
        <Dialog
          eyebrow="Uninstall"
          title={`Move ${plural(preview.length, "mod")} to the Recycle Bin?`}
          onClose={() => setPreview(null)}
          footer={
            <>
              <Button onClick={() => setPreview(null)}>Cancel</Button>
              <Button variant="danger" disabled={busy} onClick={uninstall}>
                Move to Recycle Bin
              </Button>
            </>
          }
        >
          {preview.map((p) => (
            <div key={p.folder} className="kv">
              <div className="kv-row">
                <span>Mod</span>
                <b>{p.name}</b>
              </div>
              <div className="kv-row">
                <span>Folder</span>
                <b className="mono">{p.folder}</b>
              </div>
              <div className="kv-row">
                <span>Size</span>
                <b>
                  {p.truncated ? "at least " : ""}
                  {bytes(p.bytes)}
                  {p.files !== null ? ` · ${plural(p.files, "file")}` : ""}
                </b>
              </div>
            </div>
          ))}
          {preview.some((p) => p.hand_installed) && (
            <p className="hint">
              This folder wasn't installed by GMM: it holds your own files. You can restore it from the Recycle Bin.
            </p>
          )}
        </Dialog>
      )}
    </>
  );
}
