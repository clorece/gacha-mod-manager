// SPDX-License-Identifier: MIT OR Apache-2.0
// Mod settings, the side panel from a library card: the preview and
// source, the switch, versions (switch which one is on, remove one, add more from the cached
// archive), updates and roll back, key bindings, actions (open folder, GameBanana, adopt),
// the details and Uninstall with its itemized confirmation.

import { useState } from "react";
import { api, asCmdError, type ModKey, type ModUpdateInfo, type UninstallItem } from "../api";
import { bytes, plural } from "../format";
import { itemKeys, type LibraryItem, type LibraryVersion } from "../pages/Library";
import { Button, IconButton, SidePanel, Switch, useToast } from "../ui";
import { ActionsSection, KeySection, UpdateSection } from "./ModExtras";
import { Preview } from "./Preview";
import { UninstallDialog } from "./UninstallDialog";

export function ModSettings({
  item,
  onClose,
  onToggle,
  onRefresh,
  onRemoved,
  update,
  onQueued,
  onChanged,
  onOpenMod,
  onAddVersions,
}: {
  item: LibraryItem;
  onClose: () => void;
  onToggle: (on: boolean) => void;
  /** A version was switched or removed: reload the library (the panel stays). */
  onRefresh: () => Promise<void>;
  onRemoved: () => Promise<void>;
  /** A newer file found by the startup check, for GMM's GameBanana mods. */
  update: ModUpdateInfo | undefined;
  /** An update or roll back went to the queue. */
  onQueued: () => void;
  /** The mod changed kind (adopted): reload the library. */
  onChanged: () => Promise<void>;
  /** GMM's mod page for its GameBanana mod. */
  onOpenMod: (gbModId: number) => void;
  /** The install confirmation for more versions of this file (from the download cache). */
  onAddVersions: (gbModId: number, gbFileId: number | null) => void;
}) {
  const toast = useToast();
  /** What the confirmation lists, and whether it was the whole mod (the panel closes). */
  const [preview, setPreview] = useState<{ items: UninstallItem[]; whole: boolean } | null>(null);
  const askUninstall = async (keys: ModKey[], whole: boolean) => {
    try {
      setPreview({ items: await api.uninstallPreview(item.game.id, keys), whole });
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  };
  const source = item.hand
    ? "Added by hand"
    : [item.gbModId !== null ? `GameBanana #${item.gbModId}` : "Installed by GMM", item.category]
        .filter(Boolean)
        .join(" · ");
  const folders = Math.max(item.versions.length, 1);

  return (
    <>
      <SidePanel eyebrow="Mod settings" title={item.name} onClose={onClose}>
        <section className="drawer-sec">
          <div className="ms-head">
            <Preview
              className="ms-preview"
              image={item.preview ? { url: item.preview, variants: [] } : undefined}
              width={420}
              sensitive={item.sensitive}
              alt={item.name}
              eager
            />
            <p className="sub">
              {item.game.name} · {source}
            </p>
          </div>
          <div className="flex items-center gap-3">
            <div className="grow">
              <b>Switched on</b>
              <div className="sub">
                {!item.usable
                  ? `Can't switch: ${item.problem}.`
                  : item.versions.length > 0
                    ? "Switches the current version. XXMI loads it the next time the game starts."
                    : "XXMI loads it the next time the game starts."}
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
        {item.gbModId !== null && (
          <VersionsSection
            item={item}
            onRefresh={onRefresh}
            onRemove={(v) => askUninstall([v.key], item.versions.length <= 1)}
            onAdd={() => item.gbModId !== null && onAddVersions(item.gbModId, item.gbFileId)}
          />
        )}
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
            <div className="kv-row">
              <span>Added</span>
              <b>{item.added.replace(/^Installed /, "")}</b>
            </div>
            {item.size !== null && (
              <div className="kv-row">
                <span>Size on disk</span>
                <b>
                  {bytes(item.size)} · {plural(folders, "folder")}
                </b>
              </div>
            )}
            {!item.hand && item.gbModId !== null && (
              <div className="kv-row">
                <span>File</span>
                <b>Kept in the download cache until you clear it (Settings › Storage)</b>
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
          <Button variant="danger" block icon="trash" onClick={() => askUninstall(itemKeys(item), true)}>
            Uninstall
          </Button>
          <p className="sub">
            {item.hand
              ? "Deletes the folder for good. GMM shows exactly what goes first."
              : item.versions.length > 1
                ? "Deletes every version and GMM's downloaded copy for good. You can always install it again."
                : "Deletes the mod and GMM's downloaded copy for good. You can always install it again."}
          </p>
        </section>
      </SidePanel>

      {preview && (
        <UninstallDialog
          batches={[{ game: item.game.id, items: preview.items }]}
          onClose={() => setPreview(null)}
          onDone={async () => {
            const whole = preview.whole;
            setPreview(null);
            await (whole ? onRemoved() : onRefresh());
          }}
        />
      )}
    </>
  );
}

/** The installed versions of a GameBanana file: which one is on, remove one, add more. */
function VersionsSection({
  item,
  onRefresh,
  onRemove,
  onAdd,
}: {
  item: LibraryItem;
  onRefresh: () => Promise<void>;
  onRemove: (v: LibraryVersion) => void;
  onAdd: () => void;
}) {
  const toast = useToast();
  const [busy, setBusy] = useState(false);

  const turn = async (v: LibraryVersion, on: boolean) => {
    if (v.key.type !== "managed") return;
    setBusy(true);
    try {
      if (on) {
        const outcome = await api.switchVersion(item.game.id, v.key.id);
        const off = outcome.filter((o) => o.id !== v.id && o.changed && !o.enabled);
        if (off.length > 0) toast("info", `${v.name} is on. ${plural(off.length, "version")} that clash went off.`);
      } else {
        await api.setEnabled(item.game.id, v.key, false);
      }
      await onRefresh();
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="drawer-sec">
      <h3 className="sec-h">Versions</h3>
      {item.versions.length === 0 ? (
        <p className="sub">One version installed. The archive may hold others to choose from.</p>
      ) : (
        <>
          <p className="sub">Switching one on switches off the others that replace the same things in game.</p>
          <div className="ver-list">
            {item.versions.map((v) => (
              <div className="ver-row" key={v.id}>
                <div className="grow">
                  <b>{v.name}</b>
                  <div className="sub">
                    {bytes(v.size)}
                    {!v.usable && " · folder missing"}
                  </div>
                </div>
                <Switch
                  checked={v.enabled}
                  onChange={(on) => void turn(v, on)}
                  label={`${v.name} on or off`}
                  disabled={busy || !v.usable}
                  showState
                />
                <IconButton icon="trash" label={`Remove ${v.name}`} size="sm" onClick={() => onRemove(v)} />
              </div>
            ))}
          </div>
        </>
      )}
      <Button icon="plus" onClick={onAdd}>
        {item.versions.length === 0 ? "Look for other versions" : "Add versions"}
      </Button>
      <p className="sub">Reads the archive from GMM's download cache when it's still there.</p>
    </section>
  );
}
