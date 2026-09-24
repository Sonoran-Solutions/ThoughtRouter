import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api";
import type { ProjectSummary } from "./bindings/ProjectSummary";
import type { CaptureBoxHandle } from "./components/CaptureBox";
import { DataVersion, useAsync, useProcessingVersion } from "./hooks";
import { Nav, Projects, type Route } from "./nav";
import { CaptureDetailView } from "./views/CaptureDetailView";
import { HistoryView } from "./views/HistoryView";
import { HomeView } from "./views/HomeView";
import { ProjectsView } from "./views/ProjectsView";
import { ProjectView } from "./views/ProjectView";
import { SearchView } from "./views/SearchView";
import { SettingsView } from "./views/SettingsView";
import { StatsView } from "./views/StatsView";

const TABS: { route: Route; label: string; key: string }[] = [
  { route: { name: "home" }, label: "Home", key: "1" },
  { route: { name: "history" }, label: "History", key: "2" },
  { route: { name: "search" }, label: "Search", key: "3" },
  { route: { name: "projects" }, label: "Projects", key: "4" },
  { route: { name: "stats" }, label: "Stats", key: "5" },
  { route: { name: "settings" }, label: "Settings", key: "6" },
];

function ProjectsProvider({ children }: { children: React.ReactNode }) {
  const projects = useAsync(() => api.listProjects(), []);
  const value = useMemo(
    () => ({ list: projects.data ?? ([] as ProjectSummary[]), reload: projects.reload }),
    [projects.data, projects.reload],
  );
  return <Projects.Provider value={value}>{children}</Projects.Provider>;
}

export function App() {
  const [stack, setStack] = useState<Route[]>([{ name: "home" }]);
  const route = stack[stack.length - 1];
  const version = useProcessingVersion();
  const captureRef = useRef<CaptureBoxHandle>(null);

  const go = useCallback((r: Route) => {
    setStack((s) => [...s.slice(-20), r]);
    window.scrollTo(0, 0);
  }, []);
  const back = useCallback(() => setStack((s) => (s.length > 1 ? s.slice(0, -1) : s)), []);
  const nav = useMemo(() => ({ route, go, back }), [route, go, back]);

  // Global hotkey → show the capture box.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("focus-capture", () => {
      go({ name: "home" });
      setTimeout(() => captureRef.current?.focus(), 50);
    })
      .then((u) => {
        unlisten = u;
      })
      .catch(() => {});
    return () => unlisten?.();
  }, [go]);

  // Alt+1..6 switches tabs; Ctrl/⌘+K opens search.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && !e.ctrlKey && !e.metaKey) {
        const tab = TABS.find((t) => t.key === e.key);
        if (tab) {
          e.preventDefault();
          go(tab.route);
        }
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        go({ name: "search" });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go]);

  let view: React.ReactNode;
  switch (route.name) {
    case "home":
      view = <HomeView captureRef={captureRef} />;
      break;
    case "history":
      view = <HistoryView />;
      break;
    case "search":
      view = <SearchView initial={route.q} />;
      break;
    case "projects":
      view = <ProjectsView />;
      break;
    case "project":
      view = <ProjectView id={route.id} />;
      break;
    case "capture":
      view = <CaptureDetailView id={route.id} />;
      break;
    case "stats":
      view = <StatsView />;
      break;
    case "settings":
      view = <SettingsView />;
      break;
  }

  return (
    <DataVersion.Provider value={version}>
      <Nav.Provider value={nav}>
        <ProjectsProvider>
          <div className="shell">
            <nav className="sidebar" aria-label="Main">
              <div className="brand">ThoughtRouter</div>
              {TABS.map((t) => (
                <button
                  key={t.label}
                  type="button"
                  className={route.name === t.route.name ? "tab active" : "tab"}
                  aria-current={route.name === t.route.name ? "page" : undefined}
                  title={`Alt+${t.key}`}
                  onClick={() => go(t.route)}
                >
                  {t.label}
                </button>
              ))}
            </nav>
            <main>{view}</main>
          </div>
        </ProjectsProvider>
      </Nav.Provider>
    </DataVersion.Provider>
  );
}
