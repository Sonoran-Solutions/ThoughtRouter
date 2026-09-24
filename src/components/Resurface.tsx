import { useState } from "react";
import { api, errorMessage } from "../api";
import type { ResurfaceCard } from "../bindings/ResurfaceCard";
import type { ResurfaceResponse } from "../bindings/ResurfaceResponse";
import { formatWhen, relativeDays } from "../hooks";
import { useNav, useProjects } from "../nav";
import { TypeChip } from "./Chips";

const RESPONSES: { value: ResurfaceResponse; label: string }[] = [
  { value: "interesting", label: "Interesting" },
  { value: "not_now", label: "Not now" },
  { value: "make_active", label: "Make active" },
  { value: "dismiss", label: "Dismiss" },
];

/** One forgotten thought, on demand, with the reason it was chosen. */
export function ResurfacePanel() {
  const [card, setCard] = useState<ResurfaceCard | null>();
  const [message, setMessage] = useState<string>();
  const [needsProject, setNeedsProject] = useState(false);
  const [projectName, setProjectName] = useState("");
  const [error, setError] = useState<string>();
  const nav = useNav();
  const projects = useProjects();

  async function pick() {
    setError(undefined);
    setMessage(undefined);
    setNeedsProject(false);
    try {
      const c = await api.resurface();
      setCard(c);
      if (!c)
        setMessage(
          "Nothing to resurface yet — keep capturing. Items shown recently are rested for a while.",
        );
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function respond(r: ResurfaceResponse) {
    if (!card) return;
    try {
      const out = await api.respondResurface(card.event_id, r);
      if (out.needs_project) {
        setNeedsProject(true);
        setProjectName(
          card.atom.text
            .split(/\s+/)
            .slice(0, 5)
            .join(" ")
            .replace(/[.,!?]$/, ""),
        );
        return;
      }
      setMessage(
        out.activated_project
          ? `“${out.activated_project.name}” is now active.`
          : r === "dismiss"
            ? "Won't show that again."
            : r === "not_now"
              ? "OK — it'll rest for a while."
              : "Noted.",
      );
      setCard(null);
      projects.reload();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function createProject() {
    if (!card || !projectName.trim()) return;
    try {
      const p = await api.createProjectFromAtom(card.atom.id, projectName);
      projects.reload();
      setCard(null);
      setNeedsProject(false);
      setMessage(`Created “${p.name}” as an active project.`);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <section className="panel resurface">
      <div className="panel-head">
        <h2>From the vault</h2>
        <button type="button" onClick={pick}>
          Resurface something
        </button>
      </div>
      {card && (
        <div className="resurface-card">
          <p className="reason">{card.reason}</p>
          <div className="atom-line">
            <TypeChip type={card.atom.atom_type} />{" "}
            <span className="derived-text">{card.atom.text}</span>
          </div>
          <blockquote className="raw small">
            {card.capture.text}
            <footer>
              <button
                type="button"
                className="link"
                onClick={() => nav.go({ name: "capture", id: card.capture.id })}
              >
                {formatWhen(card.capture.captured_at)} · {relativeDays(card.capture.captured_at)}
              </button>
            </footer>
          </blockquote>
          {card.projects.length > 0 && (
            <p className="muted">
              Related to {card.projects.map((p) => p.project_name).join(", ")}
            </p>
          )}
          <details className="why">
            <summary>Why this?</summary>
            <ul>
              {card.terms.map((t) => (
                <li key={t.name}>
                  {t.explanation}: <code>{t.value.toFixed(2)}</code>
                </li>
              ))}
            </ul>
          </details>
          {needsProject ? (
            <form
              className="suggestion"
              onSubmit={(e) => {
                e.preventDefault();
                void createProject();
              }}
            >
              <span>New active project:</span>
              <input
                aria-label="Project name"
                value={projectName}
                onChange={(e) => setProjectName(e.target.value)}
              />
              <button type="submit" className="small primary">
                Create
              </button>
            </form>
          ) : (
            <div className="row-actions">
              {RESPONSES.map((r) => (
                <button key={r.value} type="button" onClick={() => respond(r.value)}>
                  {r.label}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
      {message && <p className="muted">{message}</p>}
      {error && <p className="error">{error}</p>}
    </section>
  );
}
