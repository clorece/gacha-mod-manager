// SPDX-License-Identifier: MIT OR Apache-2.0
// Settings: Appearance and the Mods Store's sensitive-mods
// switches on the stored preferences, the XXMI Launcher folder (found or chosen), each game
// and how it was found (plus folders added by hand), Storage (the caches with Clear, and the
// full breakdown) and About (version, license, disclaimer, credits, third-party notices).
// Every folder comes from a native dialog opened in Rust; the page only passes game ids.

import { useCallback, useEffect, useState } from "react";
import readme from "../../../../README.md?raw";
import notices from "../../../../THIRD-PARTY-NOTICES.md?raw";
import { type AppRelease, api, asCmdError, type CacheStorage, type Settings as SettingsData } from "../api";
import type { AppData } from "../data";
import { bytes, plural } from "../format";
import { setupLabel, sourceLabel } from "../games";
import { Markdown, section } from "../markdown";
import { usePrefs } from "../prefs";
import { Badge, Button, Dialog, Segmented, Spinner, Switch, useToast } from "../ui";
import { APP_VERSION, openRelease, versionLabel } from "../version";
import { StorageBreakdown } from "./Storage";

/** Settings › About's update check. */
type UpdateState =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "done"; release: AppRelease | null }
  | { state: "failed"; message: string };

export function Settings({ data }: { data: AppData }) {
  const [breakdown, setBreakdown] = useState(false);
  if (breakdown) return <StorageBreakdown data={data} onBack={() => setBreakdown(false)} />;
  return <SettingsCards data={data} onBreakdown={() => setBreakdown(true)} />;
}

function SettingsCards({ data, onBreakdown }: { data: AppData; onBreakdown: () => void }) {
  const toast = useToast();
  const [appUpdate, setAppUpdate] = useState<UpdateState>({ state: "idle" });
  const checkUpdate = () => {
    setAppUpdate({ state: "checking" });
    api
      .appUpdateCheck()
      .then((release) => setAppUpdate({ state: "done", release }))
      .catch((e) => setAppUpdate({ state: "failed", message: asCmdError(e).message }));
  };
  const { prefs, update } = usePrefs();
  const [saved, setSaved] = useState<SettingsData | null>(null);
  const [cache, setCache] = useState<CacheStorage | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [showNotices, setShowNotices] = useState(false);

  const reload = useCallback(async () => {
    try {
      const [s, storage] = await Promise.all([api.settings(), api.storage()]);
      setSaved(s);
      setCache(storage.cache);
    } catch (e) {
      toast("bad", asCmdError(e).message);
    }
  }, [toast]);

  useEffect(() => {
    void reload();
  }, [reload]);

  /** Runs one action (named for its busy state), then reloads what it changed. */
  const run = async (name: string, action: () => Promise<string | null>, refreshGames = false) => {
    setBusy(name);
    try {
      const message = await action();
      if (message) toast("ok", message);
      if (refreshGames) await data.refresh();
      await reload();
    } catch (e) {
      toast("bad", asCmdError(e).message);
    } finally {
      setBusy(null);
    }
  };

  const xxmi = data.statuses.find((s) => s.xxmi_root)?.xxmi_root ?? null;
  const xxmiVersion = data.statuses.find((s) => s.xxmi_version)?.xxmi_version ?? null;

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Settings</h1>
          <p>Stored on this computer only. Changes apply at once.</p>
        </div>
      </div>

      <div className="settings">
        <section className="card panel" aria-labelledby="set-look">
          <h2 id="set-look" className="card-title">
            Appearance
          </h2>
          <div className="set-row">
            <div className="grow">
              <b>Theme</b>
              <div className="sub">Match Windows follows your system's dark or light mode.</div>
            </div>
            <Segmented
              label="Theme"
              value={prefs.theme}
              onChange={(theme) => void update({ theme })}
              options={[
                { value: "dark", label: "Dark" },
                { value: "light", label: "Light" },
                { value: "system", label: "Match Windows" },
              ]}
            />
          </div>
        </section>

        <section className="card panel" aria-labelledby="set-store">
          <h2 id="set-store" className="card-title">
            Mods Store
          </h2>
          <div className="set-row">
            <div className="grow">
              <b>Show mods marked as sensitive</b>
              <div className="sub">GameBanana's own flag. Also the Show sensitive chip in the Store and Library.</div>
            </div>
            <Switch
              checked={prefs.show_sensitive}
              onChange={(show_sensitive) => void update({ show_sensitive })}
              label="Show mods marked as sensitive"
              showState
            />
          </div>
          <div className="set-row">
            <div className="grow">
              <b>Blur their previews until hovered</b>
              <div className="sub">
                {prefs.show_sensitive ? "Hover or focus a card to see it." : "Sensitive mods are hidden right now."}
              </div>
            </div>
            <Switch
              checked={prefs.blur_sensitive}
              onChange={(blur_sensitive) => void update({ blur_sensitive })}
              label="Blur sensitive previews"
              disabled={!prefs.show_sensitive}
              showState
            />
          </div>
        </section>

        <section className="card panel" aria-labelledby="set-xxmi">
          <h2 id="set-xxmi" className="card-title">
            XXMI Launcher
          </h2>
          <div className="set-row">
            <div className="grow">
              <b>{xxmi ? "Folder" : "Not found"}</b>
              <div className="sub mono">
                {xxmi ?? "Install XXMI Launcher, or choose its folder if it's somewhere unusual."}
              </div>
              {xxmi && (
                <div className="sub">
                  {saved?.xxmi_root ? "Chosen by you" : "Found automatically"}
                  {xxmiVersion ? ` · version ${xxmiVersion}` : ""}
                </div>
              )}
            </div>
            <div className="set-actions">
              <Button
                icon="folder"
                disabled={busy !== null}
                onClick={() =>
                  run("xxmi", async () => ((await api.pickXxmiFolder()) ? "XXMI folder saved." : null), true)
                }
              >
                Choose folder…
              </Button>
              {saved?.xxmi_root && (
                <Button
                  disabled={busy !== null}
                  onClick={() =>
                    run(
                      "xxmi",
                      async () => {
                        await api.clearXxmiFolder();
                        return "GMM finds XXMI on its own again.";
                      },
                      true,
                    )
                  }
                >
                  Find automatically
                </Button>
              )}
            </div>
          </div>
        </section>

        <section className="card panel" aria-labelledby="set-games">
          <h2 id="set-games" className="card-title">
            Games
          </h2>
          {data.games.map((g) => {
            const status = data.statuses.find((s) => s.game_id === g.id);
            const state = status ? setupLabel(status.state) : null;
            const added = saved?.manual_folders.filter((f) => f.game === g.id) ?? [];
            return (
              <div className="set-game" key={g.id}>
                <div className="set-row">
                  <div className="grow">
                    <b>{g.name}</b>
                    <div className="sub">
                      {status && status.detected_installs.length > 0
                        ? `Found through ${[...new Set(status.detected_installs.map((i) => sourceLabel(i.source)))].join(", ")}`
                        : "Not found on this computer"}
                    </div>
                  </div>
                  {state && <Badge tone={state.tone}>{state.label}</Badge>}
                  <Button
                    icon="plus"
                    size="sm"
                    disabled={busy !== null}
                    onClick={() =>
                      run(
                        `game-${g.id}`,
                        async () => ((await api.pickGameFolder(g.id)) ? `${g.name} folder added.` : null),
                        true,
                      )
                    }
                  >
                    Add a folder…
                  </Button>
                </div>
                {status?.detected_installs.map((i) => (
                  <div className="set-sub mono" key={i.folder}>
                    {i.folder} <span className="sub">· {sourceLabel(i.source)}</span>
                  </div>
                ))}
                {added.map((f) => (
                  <div className="set-sub" key={f.folder}>
                    <span className="mono grow">{f.folder}</span>
                    <Button
                      variant="link"
                      disabled={busy !== null}
                      onClick={() =>
                        run(
                          `game-${g.id}`,
                          async () => {
                            await api.removeGameFolder(g.id, f.folder);
                            return "Folder forgotten. Nothing on disk was touched.";
                          },
                          true,
                        )
                      }
                    >
                      Forget
                    </Button>
                  </div>
                ))}
              </div>
            );
          })}
        </section>

        <section className="card panel" aria-labelledby="set-storage">
          <h2 id="set-storage" className="card-title">
            Storage
          </h2>
          {!cache || !saved ? (
            <p className="sub">
              <Spinner /> Measuring
            </p>
          ) : (
            <>
              <CacheRow
                title="Downloaded archives"
                detail={`${bytes(cache.downloads.bytes)} · ${plural(saved.download_cache_entries, "archive")}. Lets you reinstall, add versions and roll back without downloading again.`}
                busy={busy === "downloads"}
                disabled={busy !== null || cache.downloads.bytes === 0}
                onClear={() =>
                  run("downloads", async () => {
                    const cleared = await api.clearDownloadCache();
                    return `${plural(cleared.archives, "archive")} deleted (${bytes(cleared.bytes)}). Installed mods stay.`;
                  })
                }
              />
              <CacheRow
                title="Preview images"
                detail={`${bytes(cache.images.bytes)} · ${plural(cache.images.files, "image")}. Kept under 512 MiB; the oldest go first.`}
                busy={busy === "images"}
                disabled={busy !== null || cache.images.files === 0}
                onClear={() =>
                  run("images", async () => {
                    const cleared = await api.clearImageCache();
                    return `${plural(cleared.files, "image")} deleted. They download again when shown.`;
                  })
                }
              />
              <CacheRow
                title="GameBanana answers"
                detail={`${bytes(cache.http_bytes)} · ${plural(cache.http_entries, "page")}. Shown when GameBanana is slow or can't be reached.`}
                busy={busy === "http"}
                disabled={busy !== null || cache.http_entries === 0}
                onClear={() =>
                  run("http", async () => {
                    const n = await api.clearHttpCache();
                    return `${plural(n, "saved page")} cleared. The next pages load fresh.`;
                  })
                }
              />
              <div className="set-row">
                <div className="grow">
                  <b>Mods on disk</b>
                  <div className="sub">Space per game and per mod; switch off or delete several at once.</div>
                </div>
                <Button icon="layers" onClick={onBreakdown}>
                  Show breakdown
                </Button>
              </div>
              {cache.free_bytes !== null && (
                <p className="sub">{bytes(cache.free_bytes)} free on the drive that holds GMM's cache.</p>
              )}
            </>
          )}
        </section>

        <section className="card panel" aria-labelledby="set-about">
          <h2 id="set-about" className="card-title">
            About
          </h2>
          <div className="kv">
            <div className="kv-row">
              <span>Version</span>
              <b>Gacha Mod Manager {APP_VERSION}</b>
            </div>
            <div className="kv-row">
              <span>Updates</span>
              <span className="about-update">
                <b>
                  {appUpdate.state === "idle"
                    ? "GMM looks for a newer version on GitHub each time it starts."
                    : appUpdate.state === "checking"
                      ? "Looking on GitHub…"
                      : appUpdate.state === "failed"
                        ? `Couldn't check: ${appUpdate.message}`
                        : appUpdate.release
                          ? `GMM ${versionLabel(appUpdate.release.version)} is out.`
                          : "You have the newest version."}
                </b>
                {appUpdate.state === "done" && appUpdate.release ? (
                  <Button
                    variant="primary"
                    icon="download"
                    onClick={() => {
                      const release = appUpdate.state === "done" ? appUpdate.release : null;
                      if (release) openRelease(release.version, (m) => toast("bad", m));
                    }}
                  >
                    Get the update
                  </Button>
                ) : (
                  <Button onClick={checkUpdate} disabled={appUpdate.state === "checking"}>
                    Check for updates
                  </Button>
                )}
              </span>
            </div>
            <div className="kv-row">
              <span>License</span>
              <b>MIT or Apache-2.0, at your option. Free and open source.</b>
            </div>
            {saved && (
              <div className="kv-row">
                <span>Database</span>
                <b className="mono">{saved.database}</b>
              </div>
            )}
          </div>
          <h3 className="sec-h">Disclaimer</h3>
          <Markdown source={section(readme, "Disclaimer")} />
          <h3 className="sec-h">Credits</h3>
          <Markdown source={section(readme, "Credits")} />
          <div>
            <Button icon="book" onClick={() => setShowNotices(true)}>
              Third-party notices
            </Button>
          </div>
        </section>
      </div>

      {showNotices && (
        <Dialog
          eyebrow="About"
          title="Third-party notices"
          onClose={() => setShowNotices(false)}
          footer={<Button onClick={() => setShowNotices(false)}>Close</Button>}
        >
          <Markdown source={notices} skipTitle />
        </Dialog>
      )}
    </>
  );
}

function CacheRow({
  title,
  detail,
  busy,
  disabled,
  onClear,
}: {
  title: string;
  detail: string;
  busy: boolean;
  disabled: boolean;
  onClear: () => void;
}) {
  return (
    <div className="set-row">
      <div className="grow">
        <b>{title}</b>
        <div className="sub">{detail}</div>
      </div>
      <Button icon="trash" disabled={disabled} onClick={onClear}>
        {busy ? "Clearing…" : "Clear"}
      </Button>
    </div>
  );
}
