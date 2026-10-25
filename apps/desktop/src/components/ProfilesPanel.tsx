// SPDX-License-Identifier: MIT OR Apache-2.0
// Profiles for one game (Phase 6): save which mods are on, switch to a saved set, share a set
// as a code (GameBanana ids only; hand-installed mods stay private), or import someone's code.

import { useCallback, useEffect, useState } from "react";
import { api, asCmdError, type GameInfo, type ProfileInfo, type SharedMod } from "../api";
import { day, plural } from "../format";
import { Button, Notice, SidePanel, useToast } from "../ui";

export function ProfilesPanel({
  game,
  onClose,
  onApplied,
}: {
  game: GameInfo;
  onClose: () => void;
  onApplied: () => Promise<void>;
}) {
  const toast = useToast();
  const [list, setList] = useState<ProfileInfo[] | null>(null);
  const [name, setName] = useState("");
  const [code, setCode] = useState("");
  const [missing, setMissing] = useState<{ title: string; names: string[] } | null>(null);
  const [busy, setBusy] = useState(false);
  const fail = (e: unknown) => toast("bad", asCmdError(e).message);

  const reload = useCallback(async () => {
    try {
      setList(await api.profiles(game.id));
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  }, [game.id, toast]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const run = async (work: () => Promise<void>) => {
    setBusy(true);
    try {
      await work();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const save = () =>
    run(async () => {
      const saved = await api.profileSave(game.id, name);
      toast("ok", `Saved “${saved.name}” (${plural(saved.mods, "mod")} on).`);
      setName("");
      await reload();
    });

  const apply = (p: ProfileInfo) =>
    run(async () => {
      const done = await api.profileApply(game.id, p.id);
      toast(
        "ok",
        `“${p.name}” applied: ${done.switched_on.length} switched on, ${done.switched_off.length} switched off.`,
      );
      setMissing(done.missing.length > 0 ? { title: `Not in the Mods folder any more:`, names: done.missing } : null);
      await onApplied();
    });

  const share = (p: ProfileInfo) =>
    run(async () => {
      const text = await api.profileExport(game.id, p.id);
      await navigator.clipboard.writeText(text);
      toast("ok", `The code for “${p.name}” is on your clipboard. Hand-installed mods aren't in it.`);
    });

  const remove = (p: ProfileInfo) =>
    run(async () => {
      await api.profileDelete(game.id, p.id);
      toast("ok", `Deleted “${p.name}”. No mod was touched.`);
      await reload();
    });

  const importCode = () =>
    run(async () => {
      const done = await api.profileImport(game.id, code, null);
      toast("ok", `Imported “${done.profile.name}” (${plural(done.profile.mods, "mod")} you have).`);
      setMissing(
        done.missing.length > 0
          ? {
              title: "Not installed here (install them, then import again):",
              names: done.missing.map((m: SharedMod) => `${m.name} (GameBanana ${m.gb_mod_id})`),
            }
          : null,
      );
      setCode("");
      await reload();
    });

  return (
    <SidePanel eyebrow="Profiles" title={game.name} onClose={onClose}>
      <section className="drawer-sec">
        <p className="sub">
          A profile remembers which mods are on. Applying one switches exactly those on and every other mod of{" "}
          {game.name} off.
        </p>
        <form
          className="xr-row"
          onSubmit={(e) => {
            e.preventDefault();
            if (name.trim()) void save();
          }}
        >
          <input
            className="xr-input grow"
            placeholder="Name, e.g. Casual"
            aria-label="Profile name"
            maxLength={60}
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <Button type="submit" variant="primary" disabled={busy || !name.trim()}>
            Save current
          </Button>
        </form>
      </section>

      {missing && (
        <Notice tone="warn" title={missing.title}>
          {missing.names.join(", ")}
        </Notice>
      )}

      <section className="drawer-sec">
        <h3 className="sec-h">Saved</h3>
        {list === null && <p className="sub">Loading…</p>}
        {list?.length === 0 && <p className="sub">No profiles for {game.name} yet.</p>}
        {list?.map((p) => (
          <div className="xr-row" key={p.id}>
            <div className="grow">
              <b>{p.name}</b>
              <div className="sub">
                {plural(p.mods, "mod")} on · saved {day(p.updated_at)}
              </div>
            </div>
            <Button size="sm" variant="primary" disabled={busy} onClick={() => apply(p)}>
              Apply
            </Button>
            <Button size="sm" disabled={busy} onClick={() => share(p)}>
              Copy code
            </Button>
            <Button size="sm" variant="link" disabled={busy} onClick={() => remove(p)}>
              Delete
            </Button>
          </div>
        ))}
      </section>

      <section className="drawer-sec">
        <h3 className="sec-h">Import a code</h3>
        <textarea
          className="xr-input xr-code"
          placeholder='Paste a code that starts with {"gmm_profile":1'
          aria-label="Profile code"
          value={code}
          onChange={(e) => setCode(e.target.value)}
        />
        <div>
          <Button disabled={busy || !code.trim()} onClick={importCode}>
            Import
          </Button>
        </div>
      </section>
    </SidePanel>
  );
}
