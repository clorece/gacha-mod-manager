// SPDX-License-Identifier: MIT OR Apache-2.0
// Adopting a hand-installed mod: GMM suggests the GameBanana mods it could be (its own
// gmm.json, an id in the folder name, then a search), the user picks one, and GMM takes the
// folder over (`adopt`). Nothing is adopted without that pick.

import { useEffect, useState } from "react";
import { type AdoptCandidate, api, asCmdError } from "../api";
import type { LibraryItem } from "../pages/Library";
import { Button, Dialog, Notice, Radio, Spinner, useToast } from "../ui";

const REASON: Record<AdoptCandidate["reason"], string> = {
  sidecar: "Named in the folder's gmm.json",
  id_in_name: "Its id is in the folder name",
  search: "Found by searching its name",
};

export function AdoptDialog({
  item,
  onClose,
  onAdopted,
}: {
  item: LibraryItem;
  onClose: () => void;
  onAdopted: () => Promise<void>;
}) {
  const toast = useToast();
  const [found, setFound] = useState<AdoptCandidate[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [picked, setPicked] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const path = item.key.type === "unmanaged" ? item.key.path : "";
  const insideGmm = path.startsWith("GMM/");

  useEffect(() => {
    let live = true;
    api
      .adoptCandidates(item.game.id, path)
      .then((c) => {
        if (!live) return;
        setFound(c);
        setPicked(c[0]?.summary.id ?? null);
      })
      .catch((e) => live && setFailed(asCmdError(e).message));
    return () => {
      live = false;
    };
  }, [item.game.id, path]);

  const choice = found?.find((c) => c.summary.id === picked);
  const adopt = async () => {
    if (!choice) return;
    setBusy(true);
    try {
      const done = await api.adopt(item.game.id, path, choice.summary.id, choice.file_id);
      toast(
        "ok",
        done.moved
          ? `${choice.summary.name} is now GMM's. It moved into Mods\\GMM, so its in-game toggles start over.`
          : `${choice.summary.name} is now GMM's.`,
      );
      await onAdopted();
    } catch (e) {
      toast("bad", asCmdError(e).message);
      setBusy(false);
    }
  };

  return (
    <Dialog
      eyebrow="Find on GameBanana"
      title={item.name}
      subtitle="Which GameBanana mod is this folder?"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!choice || busy} onClick={adopt}>
            This is it
          </Button>
        </>
      }
    >
      {failed && (
        <Notice tone="bad" title="Couldn't look it up.">
          {failed}
        </Notice>
      )}
      {!found && !failed && (
        <p className="sub">
          <Spinner /> Asking GameBanana
        </p>
      )}
      {found && found.length === 0 && (
        <Notice tone="info" title="Nothing on GameBanana looks like it.">
          Rename the folder to the mod's name, or keep it as a hand-installed mod.
        </Notice>
      )}
      {found && found.length > 0 && (
        <div className="opt-list cf-vlist">
          {found.map((c) => {
            // Flagged mods are common in search results: no preview for them here, like the
            // store with sensitive mods hidden.
            const flagged = c.summary.visibility !== "show" || c.summary.has_content_ratings;
            const thumb = flagged ? undefined : (c.summary.images[0]?.variants[0]?.url ?? c.summary.images[0]?.url);
            return (
              <Radio
                key={c.summary.id}
                className="opt-row"
                name="adopt"
                checked={picked === c.summary.id}
                onChange={() => setPicked(c.summary.id)}
              >
                {thumb ? (
                  <img className="thumb xr-thumb" src={thumb} alt="" />
                ) : (
                  <span className="thumb art a3 xr-thumb" />
                )}
                <span className="grow">
                  <b className="block font-extrabold">{c.summary.name}</b>
                  <span className="sub">
                    by {c.summary.submitter.name} · {REASON[c.reason]}
                    {flagged ? " · flagged sensitive, preview hidden" : ""}
                  </span>
                </span>
              </Radio>
            );
          })}
        </div>
      )}
      <p className="hint">
        GMM keeps the folder's name and files, and from then on checks GameBanana for updates. Updating replaces its
        files with the GameBanana ones.
        {insideGmm ? "" : " It moves into Mods\\GMM, so 3DMigoto's saved in-game toggles for it start over."}
      </p>
    </Dialog>
  );
}
