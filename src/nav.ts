import { createContext, useContext } from "react";
import type { ProjectSummary } from "./bindings/ProjectSummary";

export type Route =
  | { name: "home" }
  | { name: "history" }
  | { name: "search"; q?: string }
  | { name: "projects" }
  | { name: "project"; id: string }
  | { name: "capture"; id: string }
  | { name: "stats" }
  | { name: "settings" };

export interface NavApi {
  route: Route;
  go: (r: Route) => void;
  back: () => void;
}

export const Nav = createContext<NavApi>({ route: { name: "home" }, go: () => {}, back: () => {} });
export const useNav = () => useContext(Nav);

/** All projects, for link pickers. Reloaded with processing events. */
export const Projects = createContext<{ list: ProjectSummary[]; reload: () => void }>({
  list: [],
  reload: () => {},
});
export const useProjects = () => useContext(Projects);
