// SPDX-License-Identifier: MIT OR Apache-2.0
// Mod settings: the side panel from a library card (docs/08, "Mod settings"): the switch,
// updates and roll back, key bindings, actions (open folder, GameBanana, adopt), the details
// and Uninstall with its itemized confirmation. Switching between versions comes with the
// Library's version groups.

import { useState } from "react";
import { api, asCmdError, type ModUpdateInfo, type UninstallItem } from "../api";
import { bytes } from "../format";
import type { LibraryItem } from "../pages/Library";
import { Button, SidePanel, Switch, useToast } from "../ui";
import { ActionsSection, KeySection, UpdateSection } from "./ModExtras";
import { UninstallDialog } from "./UninstallDialog";

export function ModSettings({
  item,
  onClose,
  onToggle,
  onRemoved,
  update,
  onQueued,
  onChanged,
  onOpenMod,
}: {
  item: LibraryItem;
  onClose: () => void;
  onToggle: (on: boolean) => void;
  onRemoved: () => Promise<void>;
  /** A newer file found by the startup check, for GMM's GameBanana mods. */
  update: ModUpdateInfo | undefined;
  /** An update or roll back went to the queue. */
  onQueued: () => void;
  /** The mod changed kind (adopted): reload the library. */
  onChanged: () => Promise<void>;
  /** GMM's mod page for its GameBanana mod. */
  onOpenMod: (gbModId: number) => void;
}) {
  const toast = useToast();
  const [preview, setPreview] = useState<UninstallItem[] | null>(null);
  const askUninstall = async () => {
    try {
      setPreview(await api.uninstallPreview(item.game.id, [item.key]));
    } catch (e) {
      toast("bad", asCmdError(e).message);
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
        <UpdateSection item={item} update={update} onQueued={onQueued} />
        <KeySection item={item} />
        <ActionsSection item={item} onAdopted={onChanged} onOpenMod={onOpenMod} />
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
              ? "Deletes the folder for good. GMM shows exactly what goes first."
              : "Deletes the mod and GMM's downloaded copy for good. You can always install it again."}
          </p>
        </section>
      </SidePanel>

      {preview && (
        <UninstallDialog
          game={item.game.id}
          items={preview}
          onClose={() => setPreview(null)}
          onDone={async () => {
            setPreview(null);
            await onRemoved();
          }}
        />
      )}
    </>
  );
}
