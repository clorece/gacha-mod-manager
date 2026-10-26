// SPDX-License-Identifier: MIT OR Apache-2.0
// The itemized uninstall confirmation (security rule R8), shared by Mod settings and the
// Library's select mode. Deleting is permanent: no Recycle Bin, and GMM's cached archives
// of a mod go with its last version. Hand-installed folders get a stronger warning, since
// GameBanana can't give them back.

import { useState } from "react";
import { api, asCmdError, type Uninstalled, type UninstallItem } from "../api";
import { bytes, plural } from "../format";
import { Button, Dialog, Notice, useToast } from "../ui";

export function UninstallDialog({
  batches,
  onClose,
  onDone,
}: {
  /** From `uninstall_preview`, per game: exactly what goes. */
  batches: { game: string; items: UninstallItem[] }[];
  onClose: () => void;
  onDone: (done: Uninstalled[]) => Promise<void> | void;
}) {
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const items = batches.flatMap((b) => b.items);
  const hand = items.filter((p) => p.hand_installed);
  const total = items.reduce((sum, p) => sum + p.bytes, 0);
  // Several folders of one mod are its versions.
  const what =
    items.length === 1
      ? items[0].name
      : new Set(items.map((p) => p.name)).size === 1
        ? `${items[0].name} (${items.length} versions)`
        : plural(items.length, "mod");

  const confirm = async () => {
    setBusy(true);
    try {
      const done: Uninstalled[] = [];
      for (const b of batches) {
        done.push(
          ...(await api.uninstall(
            b.game,
            b.items.map((p) => p.key),
          )),
        );
      }
      const deleted = done.filter((d) => d.removed).length;
      toast("ok", items.length === 1 && deleted === 0 ? `${what} removed from GMM's list.` : `${what} deleted.`);
      await onDone(done);
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      eyebrow="Uninstall"
      title={`Delete ${what}?`}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="danger" icon="trash" disabled={busy} onClick={confirm}>
            {items.length === 1 ? "Delete" : `Delete ${items.length}`}
          </Button>
        </>
      }
    >
      <p className="sub">
        Deleted for good, not moved to the Recycle Bin
        {items.length > 1 ? ` · ${bytes(total)} in all` : ""}. GMM's downloaded copy goes too, so reinstalling downloads
        it again.
      </p>
      <div className="uninstall-list">
        {items.map((p) => (
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
      </div>
      {hand.length > 0 && (
        <Notice
          tone="bad"
          title={
            hand.length === 1 && items.length === 1
              ? "This folder wasn't installed by GMM."
              : `${plural(hand.length, "folder")} weren't installed by GMM.`
          }
        >
          They hold files you added yourself. GMM can't download them again, and they won't be in the Recycle Bin.
        </Notice>
      )}
    </Dialog>
  );
}
