// SPDX-License-Identifier: MIT OR Apache-2.0
// The app shell (docs/08, "Navigation"): the sidebar and the current page. Dashboard and
// Mods Library are built; the other pages arrive in Phase 5 (docs/05-roadmap.md).

import { lazy, type ReactNode, Suspense, useState } from "react";
import { modCounts, useAppData } from "./data";
import { ALL, type Scope } from "./games";
import { Dashboard } from "./pages/Dashboard";
import { Library } from "./pages/Library";
import { ComingSoon, ReportBug } from "./pages/Placeholders";
import { type Page, Sidebar } from "./shell/Sidebar";
import { useTheme } from "./theme";

// Dev builds only; `import.meta.env.DEV` is false in release builds, so the page is dropped.
const StyleGuide = import.meta.env.DEV ? lazy(() => import("./pages/StyleGuide")) : null;

export default function App() {
  const data = useAppData();
  const { theme, choose } = useTheme();
  const [page, setPage] = useState<Page>("dashboard");
  const [scope, setScope] = useState<Scope>(ALL);

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
    case "library":
      content = <Library data={data} scope={scope} onScope={setScope} />;
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
        onGo={setPage}
        games={pickerGames}
        scope={scope}
        onScope={setScope}
        theme={theme}
        onToggleTheme={() => choose(theme === "dark" ? "light" : "dark")}
      />
      <main className="main">{content}</main>
    </div>
  );
}
