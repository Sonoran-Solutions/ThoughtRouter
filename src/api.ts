// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs.
// Types come from src/bindings (generated from Rust by ts-rs).
import { invoke } from "@tauri-apps/api/core";
import type { Atom } from "./bindings/Atom";
import type { AtomType } from "./bindings/AtomType";
import type { Capture } from "./bindings/Capture";
import type { CaptureView } from "./bindings/CaptureView";
import type { ExportResult } from "./bindings/ExportResult";
import type { HomeData } from "./bindings/HomeData";
import type { Momentum } from "./bindings/Momentum";
import type { ProcessingStatus } from "./bindings/ProcessingStatus";
import type { Project } from "./bindings/Project";
import type { ProjectPage } from "./bindings/ProjectPage";
import type { ProjectSummary } from "./bindings/ProjectSummary";
import type { RelatedAtom } from "./bindings/RelatedAtom";
import type { RespondOutcome } from "./bindings/RespondOutcome";
import type { ResurfaceCard } from "./bindings/ResurfaceCard";
import type { ResurfaceResponse } from "./bindings/ResurfaceResponse";
import type { SearchResponse } from "./bindings/SearchResponse";
import type { Settings } from "./bindings/Settings";
import type { SettingsView } from "./bindings/SettingsView";
import type { Stats } from "./bindings/Stats";
import type { SynthesisView } from "./bindings/SynthesisView";

export const api = {
  saveCapture: (text: string) => invoke<CaptureView>("save_capture", { text }),
  listCaptures: (before?: string, limit = 30) =>
    invoke<CaptureView[]>("list_captures", { before: before ?? null, limit }),
  getCapture: (id: string) => invoke<CaptureView>("get_capture", { id }),
  trashCapture: (id: string) => invoke<void>("trash_capture", { id }),
  restoreCapture: (id: string) => invoke<void>("restore_capture", { id }),
  purgeCapture: (id: string) => invoke<void>("purge_capture", { id }),
  listTrash: () => invoke<Capture[]>("list_trash"),
  reprocessCapture: (id: string) => invoke<void>("reprocess_capture", { id }),
  reprocessAll: () => invoke<number>("reprocess_all"),
  retryFailed: (captureId?: string) =>
    invoke<number>("retry_failed", { captureId: captureId ?? null }),

  setAtomType: (atomId: string, atomType: AtomType) =>
    invoke<void>("set_atom_type", { atomId, atomType }),
  setAtomText: (atomId: string, text: string) => invoke<void>("set_atom_text", { atomId, text }),
  rejectAtom: (atomId: string) => invoke<void>("reject_atom", { atomId }),
  addAtom: (captureId: string, text: string, atomType: AtomType) =>
    invoke<Atom>("add_atom", { captureId, text, atomType }),

  search: (query: string) => invoke<SearchResponse>("search", { query }),
  related: (captureId: string) => invoke<RelatedAtom[]>("related", { captureId }),

  listProjects: () => invoke<ProjectSummary[]>("list_projects"),
  createProject: (name: string, description: string, momentum: Momentum) =>
    invoke<Project>("create_project", { name, description, momentum }),
  updateProject: (id: string, name: string, description: string, momentum: Momentum) =>
    invoke<Project>("update_project", { id, name, description, momentum }),
  deleteProject: (id: string) => invoke<void>("delete_project", { id }),
  projectPage: (id: string) => invoke<ProjectPage>("project_page", { id }),
  summarizeProject: (id: string) => invoke<SynthesisView>("summarize_project", { id }),
  confirmLink: (atomId: string, projectId: string) =>
    invoke<void>("confirm_link", { atomId, projectId }),
  rejectLink: (atomId: string, projectId: string) =>
    invoke<void>("reject_link", { atomId, projectId }),
  acceptSuggestion: (id: string, name?: string) =>
    invoke<Project>("accept_suggestion", { id, name: name ?? null }),
  dismissSuggestion: (id: string) => invoke<void>("dismiss_suggestion", { id }),

  resurface: () => invoke<ResurfaceCard | null>("resurface"),
  respondResurface: (eventId: string, response: ResurfaceResponse) =>
    invoke<RespondOutcome>("respond_resurface", { eventId, response }),
  createProjectFromAtom: (atomId: string, name: string) =>
    invoke<Project>("create_project_from_atom", { atomId, name }),
  home: () => invoke<HomeData>("home"),
  stats: () => invoke<Stats>("stats"),
  processingStatus: () => invoke<ProcessingStatus>("processing_status"),

  getSettings: () => invoke<SettingsView>("get_settings"),
  saveSettings: (next: Settings) => invoke<SettingsView>("save_settings", { next }),
  setApiKey: (key: string) => invoke<void>("set_api_key", { key }),
  clearApiKey: () => invoke<void>("clear_api_key"),

  exportMarkdown: () => invoke<ExportResult>("export_markdown"),
  exportJson: () => invoke<ExportResult>("export_json"),
  backupNow: () => invoke<string>("backup_now"),
  importJson: () => invoke<string | null>("import_json"),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
};

export const ATOM_TYPES: AtomType[] = [
  "spark",
  "project",
  "feature",
  "task",
  "question",
  "research",
  "reference",
  "decision",
  "problem",
  "someday",
];

export const MOMENTA: Momentum[] = [
  "spark",
  "exploring",
  "ready",
  "active",
  "blocked",
  "dormant",
  "finished",
];

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
