// SPDX-License-Identifier: MIT OR Apache-2.0
// The Phase 6 parts of the Mod settings panel: updating and rolling back a GameBanana mod,
// its key bindings, and the actions (open folder, view on GameBanana, adopt a hand-installed
// mod). Each talks to its own command; nothing here names a path or an address.

import { useEffect, useState } from "react";
import { api, asCmdError, type KeyBindings, type ModUpdateInfo, type RollbackFile } from "../api";
import { day } from "../format";
import type { LibraryItem } from "../pages/Library";
import { Button, Spinner, useToast } from "../ui";
import { AdoptDialog } from "./AdoptDialog";

/** Update to the newest file, or roll back to one still in the download cache. */
export function UpdateSection({
  item,
  update,
  onQueued,
}: {
  item: LibraryItem;
  update: ModUpdateInfo | undefined;
  onQueued: () => void;
}) {
  const toast = useToast();
  const [earlier, setEarlier] = useState<RollbackFile[] | null>(null);
  const [busy, setBusy] = useState(false);
  const gbModId = item.gbModId;
  if (gbModId === null) return null;

  const queue = async (fileId: number | null, what: string) => {
    setBusy(true);
    try {
      await api.queueChangeFile(gbModId, fileId);
      toast("ok", `${what} added to Downloads. Each version keeps its folder and its on/off state.`);
      onQueued();
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const showEarlier = async () => {
    try {
      setEarlier(await api.rollbackFiles(item.game.id, gbModId));
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  };

  return (
    <section className="drawer-sec">
      <h3 className="sec-h">Updates</h3>
      {update ? (
        <div className="xr-row">
          <div className="grow">
            <b>A newer file is on GameBanana</b>
            <div className="sub">
              {update.newest.name} · {day(update.newest.date_added)}
            </div>
          </div>
          <Button variant="primary" icon="download" disabled={busy} onClick={() => queue(update.newest.id, "Update")}>
            Update
          </Button>
        </div>
      ) : (
        <p className="sub">Up to date as of the last startup check.</p>
      )}
      {earlier === null ? (
        <Button variant="link" onClick={showEarlier}>
          Roll back to an earlier file…
        </Button>
      ) : earlier.length === 0 ? (
        <p className="sub">No earlier file of this mod is in the download cache.</p>
      ) : (
        earlier.map((f) => (
          <div className="xr-row" key={f.file_id}>
            <div className="grow">
              <b>{f.name}</b>
              <div className="sub">
                {f.version ? `Version ${f.version} · ` : ""}
                {f.date_added ? day(f.date_added) : "No longer listed on GameBanana"} · in the cache
              </div>
            </div>
            <Button disabled={busy} onClick={() => queue(f.file_id, "Roll back")}>
              Roll back
            </Button>
          </div>
        ))
      )}
    </section>
  );
}

/** Pretty key names: `no_modifiers ctrl VK_LEFT` -> `Ctrl + Left`. */
function keyLabel(raw: string): string {
  return raw
    .split(/\s+/)
    .filter((part) => part && !/^no_/i.test(part))
    .map((part) => {
      const k = part.replace(/^VK_/i, "");
      return k.length <= 1 ? k.toUpperCase() : k.charAt(0).toUpperCase() + k.slice(1).toLowerCase();
    })
    .join(" + ");
}

/** The mod's `[Key*]` sections: which keys switch its parts in game. */
export function KeySection({ item }: { item: LibraryItem }) {
  const [keys, setKeys] = useState<KeyBindings | null>(null);
  const [failed, setFailed] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    api
      .modKeybindings(item.game.id, item.key)
      .then((k) => live && setKeys(k))
      .catch((e) => live && setFailed(asCmdError(e).message));
    return () => {
      live = false;
    };
  }, [item.game.id, item.key]);

  return (
    <section className="drawer-sec">
      <h3 className="sec-h">Key bindings</h3>
      {failed && <p className="sub">{failed}</p>}
      {!keys && !failed && (
        <p className="sub">
          <Spinner /> Reading the mod's files
        </p>
      )}
      {keys && keys.bindings.length === 0 && <p className="sub">This mod has no key bindings.</p>}
      {keys && keys.bindings.length > 0 && (
        <div className="kv">
          {keys.bindings.map((k) => (
            <div className="kv-row" key={`${k.ini}|${k.name}`}>
              <span title={k.ini}>{k.name}</span>
              <b>
                {k.keys.map(keyLabel).join(" or ")}
                {k.back.length > 0 ? ` · back: ${k.back.map(keyLabel).join(" or ")}` : ""}
                {k.kind ? ` · ${k.kind}` : ""}
              </b>
            </div>
          ))}
        </div>
      )}
      {keys && keys.notes.length > 0 && <p className="sub">{keys.notes.join(" · ")}</p>}
    </section>
  );
}

/** Open folder, View or Find on GameBanana. */
export function ActionsSection({
  item,
  onAdopted,
  onOpenMod,
}: {
  item: LibraryItem;
  onAdopted: () => Promise<void>;
  onOpenMod: (gbModId: number) => void;
}) {
  const toast = useToast();
  const [adopting, setAdopting] = useState(false);
  const fail = (e: unknown) => toast("bad", asCmdError(e).message);
  const gbModId = item.gbModId;

  return (
    <section className="drawer-sec">
      <h3 className="sec-h">Actions</h3>
      <div className="action-grid">
        <button type="button" className="action" onClick={() => api.openModFolder(item.game.id, item.key).catch(fail)}>
          <b>Open folder</b>
          <span className="sub">In File Explorer</span>
        </button>
        {gbModId !== null ? (
          <button type="button" className="action" onClick={() => onOpenMod(gbModId)}>
            <b>View mod page</b>
            <span className="sub">Details, files and updates, with a link to GameBanana</span>
          </button>
        ) : (
          item.hand &&
          item.isDir && (
            <button type="button" className="action" onClick={() => setAdopting(true)}>
              <b>Find on GameBanana</b>
              <span className="sub">Let GMM manage and update it</span>
            </button>
          )
        )}
      </div>
      {adopting && (
        <AdoptDialog
          item={item}
          onClose={() => setAdopting(false)}
          onAdopted={async () => {
            setAdopting(false);
            await onAdopted();
          }}
        />
      )}
    </section>
  );
}
