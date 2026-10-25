// SPDX-License-Identifier: MIT OR Apache-2.0
// The app shell (docs/08, "Navigation"): the startup check first, the preferences screen on
// the first run, then the sidebar and the current page, the install queue, the install
// confirmation and the local install (a file dropped on the window). The mod page opens
// over the page it came from, which stays mounted (hidden), so Back returns to the Store
// with its filters and scroll position. Settings and Documentation arrive later in Phase 5
// (docs/05-roadmap.md).

import { lazy, type ReactNode, Suspense, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type ModKey, type ModUpdateInfo } from "./api";
import { useOwned } from "./catalog";
import { InstallDialog } from "./components/InstallDialog";
import { LocalInstallDialog } from "./components/LocalInstallDialog";
import { NetworkNotice } from "./components/NetworkNotice";
import { modCounts, useAppData } from "./data";
import { ALL, type Scope } from "./games";
import { Dashboard } from "./pages/Dashboard";
import { Downloads } from "./pages/Downloads";
import { Library } from "./pages/Library";
import { ModPage } from "./pages/ModPage";
import { ComingSoon, ReportBug } from "./pages/Placeholders";
import { Startup } from "./pages/Startup";
import { Store } from "./pages/Store";
import { Welcome } from "./pages/Welcome";
import { usePrefs } from "./prefs";
import { isActive, useQueue } from "./queue";
import { type Page, Sidebar } from "./shell/Sidebar";
import { useAppliedTheme } from "./theme";

// Dev builds only; `import.meta.env.DEV` is false in release builds, so the page is dropped.
const StyleGuide = import.meta.env.DEV ? lazy(() => import("./pages/StyleGuide")) : null;

const pageNames: Partial<Record<Page, string>> = { store: "Mods Store", library: "Mods Library" };

export default function App() {
  const data = useAppData();
  const { prefs, loaded, update } = usePrefs();
  const theme = useAppliedTheme(prefs.theme);
  const choose = (next: typeof prefs.theme) => void update({ theme: next });
  const [page, setPage] = useState<Page>("dashboard");
  const [scope, setScope] = useState<Scope>(ALL);
  const [started, setStarted] = useState(false);
  // Newer files the startup check found; the Mod settings panel offers Update for them.
  const [updates, setUpdates] = useState<ModUpdateInfo[]>([]);
  const owned = useOwned(data, updates);
  const [installing, setInstalling] = useState<{ modId: number; fileId: number | null } | null>(null);
  /** The mod page, over the page it was opened from. */
  const [modPage, setModPage] = useState<number | null>(null);
  /** A Library mod to open the settings of (from the Store or a mod page). */
  const [libraryFocus, setLibraryFocus] = useState<{ game: string; key: ModKey } | null>(null);
  /** The name of an archive dropped on the window or picked, for the local install. */
  const [localFile, setLocalFile] = useState<string | null>(null);
  const main = useRef<HTMLElement>(null);
  const savedScroll = useRef(0);
  const restoreScroll = useRef(false);
  const jobs = useQueue((job) => {
    void data.refreshLibrary(job.game);
    // An update that finished is no longer one to offer.
    if (job.action === "change_file") setUpdates((all) => all.filter((u) => u.gb_mod_id !== job.mod_id));
  });

  useEffect(() => {
    api.dropWatch((name) => setLocalFile(name)).catch(() => {});
  }, []);

  // Back from a mod page lands where the user was.
  useLayoutEffect(() => {
    if (!main.current) return;
    if (modPage !== null) {
      main.current.scrollTop = 0;
    } else if (restoreScroll.current) {
      main.current.scrollTop = savedScroll.current;
      restoreScroll.current = false;
    }
  }, [modPage]);

  if (started && loaded && !prefs.onboarded) {
    return <Welcome />;
  }

  if (!started) {
    return (
      <Startup
        games={data.games}
        onDone={(report) => {
          setUpdates(report?.updates ?? []);
          setStarted(true);
        }}
      />
    );
  }

  const gameName = (id: string) => data.games.find((g) => g.id === id)?.name ?? id;
  const go = (next: Page) => {
    setModPage(null);
    setPage(next);
  };
  const showLibrary = (game: string) => {
    setScope(game);
    go("library");
  };
  const openMod = (modId: number) => {
    savedScroll.current = main.current?.scrollTop ?? 0;
    setModPage(modId);
  };
  const closeMod = () => {
    restoreScroll.current = true;
    setModPage(null);
  };
  /** An installed mod's settings in the Library. */
  const manage = (modId: number) => {
    const mine = owned.get(modId);
    if (!mine) return;
    setLibraryFocus({ game: mine.game, key: { type: "managed", id: mine.rows[0] } });
    showLibrary(mine.game);
  };
  /** Every Download button: the install confirmation, or for an installed mod with one
   *  version its settings (several versions: the confirmation in "Add versions" mode). */
  const download = (mod: { id: number }, fileId?: number) => {
    const mine = owned.get(mod.id);
    if (mine && !mine.versions && fileId === undefined) manage(mod.id);
    else setInstalling({ modId: mod.id, fileId: fileId ?? null });
  };

  const pickerGames = data.games.map((g) => ({
    id: g.id,
    name: g.name,
    short: g.short_name,
    mods: modCounts(data.libraries[g.id]).total,
  }));

  let content: ReactNode;
  switch (page) {
    case "dashboard":
      content = <Dashboard data={data} />;
      break;
    case "downloads":
      content = (
        <Downloads
          jobs={jobs}
          games={data.games}
          onShowLibrary={showLibrary}
          onInstallFile={async () => {
            const name = await api.pickArchive();
            if (name) setLocalFile(name);
          }}
        />
      );
      break;
    case "library":
      content = (
        <Library
          data={data}
          scope={scope}
          onScope={setScope}
          updates={updates}
          onQueued={() => go("downloads")}
          focus={libraryFocus}
          onFocusDone={() => setLibraryFocus(null)}
          onOpenMod={openMod}
        />
      );
      break;
    case "store":
      content = <Store data={data} scope={scope} owned={owned} onOpenMod={openMod} onDownload={download} />;
      break;
    case "bug":
      content = <ReportBug />;
      break;
    case "styleguide":
      content = StyleGuide && (
        <Suspense fallback={null}>
          <StyleGuide theme={theme} onTheme={choose} />
        </Suspense>
      );
      break;
    default:
      content = <ComingSoon page={page} />;
  }

  return (
    <div className="app">
      <Sidebar
        page={page}
        onGo={go}
        games={pickerGames}
        scope={scope}
        onScope={setScope}
        theme={theme}
        onToggleTheme={() => choose(theme === "dark" ? "light" : "dark")}
        activeDownloads={jobs.filter(isActive).length}
      />
      <main className="main" ref={main}>
        <NetworkNotice />
        <div hidden={modPage !== null}>{content}</div>
        {modPage !== null && (
          <ModPage
            key={modPage}
            modId={modPage}
            owned={owned.get(modPage)}
            backLabel={pageNames[page] ?? "Back"}
            onBack={closeMod}
            onDownload={download}
            onManage={() => manage(modPage)}
          />
        )}
      </main>
      {localFile !== null && data.games.length > 0 && (
        <LocalInstallDialog
          fileName={localFile}
          games={data.games}
          defaultGame={scope === ALL ? (data.games[0]?.id ?? "") : scope}
          onClose={() => setLocalFile(null)}
          onInstalled={async (game) => {
            setLocalFile(null);
            await data.refreshLibrary(game);
            showLibrary(game);
          }}
        />
      )}
      {installing !== null && (
        <InstallDialog
          modId={installing.modId}
          initialFileId={installing.fileId}
          gameName={gameName}
          onClose={() => setInstalling(null)}
          onQueued={() => {
            setInstalling(null);
            go("downloads");
          }}
        />
      )}
    </div>
  );
}
