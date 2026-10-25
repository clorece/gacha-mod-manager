// SPDX-License-Identifier: MIT OR Apache-2.0
// The app shell (docs/08, "Navigation"): the startup check first, then the sidebar and the
// current page, the install queue and the install confirmation. Dashboard, Downloads and
// Mods Library are built; the other pages arrive in Phase 5 (docs/05-roadmap.md).

import { lazy, type ReactNode, Suspense, useState } from "react";
import type { StartupReport } from "./api";
import { InstallDialog } from "./components/InstallDialog";
import { modCounts, useAppData } from "./data";
import { ALL, type Scope } from "./games";
import { Dashboard } from "./pages/Dashboard";
import { Downloads } from "./pages/Downloads";
import { Library } from "./pages/Library";
import { ComingSoon, ReportBug } from "./pages/Placeholders";
import { Startup } from "./pages/Startup";
import { isActive, useQueue } from "./queue";
import { type Page, Sidebar } from "./shell/Sidebar";
import { useTheme } from "./theme";

// Dev builds only; `import.meta.env.DEV` is false in release builds, so the page is dropped.
const StyleGuide = import.meta.env.DEV ? lazy(() => import("./pages/StyleGuide")) : null;

export default function App() {
  const data = useAppData();
  const { theme, choose } = useTheme();
  const [page, setPage] = useState<Page>("dashboard");
  const [scope, setScope] = useState<Scope>(ALL);
  const [started, setStarted] = useState(false);
  // The startup check's findings (mod updates), for the library and store badges to come.
  const [, setStartup] = useState<StartupReport | null>(null);
  const [installing, setInstalling] = useState<number | null>(null);
  const jobs = useQueue((job) => void data.refreshLibrary(job.game));

  if (!started) {
    return (
      <Startup
        games={data.games}
        onDone={(report) => {
          setStartup(report);
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
      content = <Downloads jobs={jobs} games={data.games} onShowLibrary={showLibrary} />;
      break;
    case "library":
      content = <Library data={data} scope={scope} onScope={setScope} />;
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
      <main className="main">{content}</main>
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
