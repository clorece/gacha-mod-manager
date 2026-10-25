// SPDX-License-Identifier: MIT OR Apache-2.0
// The app shell (docs/08, "Navigation"): the startup check first, the preferences screen on
// the first run, then the sidebar and the current page, the install queue, the install confirmation and the local install (a file
// dropped on the window). Dashboard, Downloads and Mods Library are built; the other pages
// arrive in Phase 5 (docs/05-roadmap.md).

import { lazy, type ReactNode, Suspense, useEffect, useState } from "react";
import { api, type ModUpdateInfo } from "./api";
import { InstallDialog } from "./components/InstallDialog";
import { LocalInstallDialog } from "./components/LocalInstallDialog";
import { NetworkNotice } from "./components/NetworkNotice";
import { modCounts, useAppData } from "./data";
import { ALL, type Scope } from "./games";
import { Dashboard } from "./pages/Dashboard";
import { Downloads } from "./pages/Downloads";
import { Library } from "./pages/Library";
import { ComingSoon, ReportBug } from "./pages/Placeholders";
import { Startup } from "./pages/Startup";
import { Welcome } from "./pages/Welcome";
import { usePrefs } from "./prefs";
import { isActive, useQueue } from "./queue";
import { type Page, Sidebar } from "./shell/Sidebar";
import { useAppliedTheme } from "./theme";

// Dev builds only; `import.meta.env.DEV` is false in release builds, so the page is dropped.
const StyleGuide = import.meta.env.DEV ? lazy(() => import("./pages/StyleGuide")) : null;

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
  const [installing, setInstalling] = useState<number | null>(null);
  /** The name of an archive dropped on the window or picked, for the local install. */
  const [localFile, setLocalFile] = useState<string | null>(null);
  const jobs = useQueue((job) => {
    void data.refreshLibrary(job.game);
    // An update that finished is no longer one to offer.
    if (job.action === "change_file") setUpdates((all) => all.filter((u) => u.gb_mod_id !== job.mod_id));
  });

  useEffect(() => {
    api.dropWatch((name) => setLocalFile(name)).catch(() => {});
  }, []);

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
  const showLibrary = (game: string) => {
    setScope(game);
    setPage("library");
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
        <Library data={data} scope={scope} onScope={setScope} updates={updates} onQueued={() => setPage("downloads")} />
      );
      break;
    case "bug":
      content = <ReportBug />;
      break;
    case "styleguide":
      content = StyleGuide && (
        <Suspense fallback={null}>
          <StyleGuide theme={theme} onTheme={choose} onInstall={setInstalling} />
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
        onGo={setPage}
        games={pickerGames}
        scope={scope}
        onScope={setScope}
        theme={theme}
        onToggleTheme={() => choose(theme === "dark" ? "light" : "dark")}
        activeDownloads={jobs.filter(isActive).length}
      />
      <main className="main">
        <NetworkNotice />
        {content}
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
          modId={installing}
          gameName={gameName}
          onClose={() => setInstalling(null)}
          onQueued={() => {
            setInstalling(null);
            setPage("downloads");
          }}
        />
      )}
    </div>
  );
}
