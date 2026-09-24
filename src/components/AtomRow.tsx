import { useState } from "react";
import { api, errorMessage } from "../api";
import type { AtomView } from "../bindings/AtomView";
import { useNav, useProjects } from "../nav";
import { TypeSelect } from "./Chips";

interface Props {
  view: AtomView;
  onChanged: () => void;
}

/** One AI (or user) interpretation, correctable in place with one click. */
export function AtomRow({ view, onChanged }: Props) {
  const { atom, links } = view;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(atom.text);
  const [error, setError] = useState<string>();

  const act = async (fn: () => Promise<unknown>) => {
    setError(undefined);
    try {
      await fn();
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <li className="atom">
      <div className="atom-main">
        <TypeSelect
          label="Atom type"
          value={atom.atom_type}
          onChange={(t) => act(() => api.setAtomType(atom.id, t))}
        />
        {editing ? (
          <form
            className="atom-edit"
            onSubmit={(e) => {
              e.preventDefault();
              setEditing(false);
              if (draft.trim() && draft !== atom.text)
                void act(() => api.setAtomText(atom.id, draft));
            }}
          >
            <input
              aria-label="Atom text"
              autoFocus
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={() => setEditing(false)}
              onKeyDown={(e) => e.key === "Escape" && setEditing(false)}
            />
          </form>
        ) : (
          <button
            type="button"
            className="atom-text"
            title="Click to edit"
            onClick={() => {
              setDraft(atom.text);
              setEditing(true);
            }}
          >
            {atom.text}
          </button>
        )}
        <span className="atom-meta">
          {atom.origin === "user"
            ? "edited by you"
            : atom.confidence != null
              ? `${Math.round(atom.confidence * 100)}%`
              : ""}
        </span>
        <button
          type="button"
          className="icon"
          title="Not a real thought — reject"
          aria-label="Reject atom"
          onClick={() => act(() => api.rejectAtom(atom.id))}
        >
          ✕
        </button>
      </div>
      <LinkChips atomId={atom.id} links={links} onChanged={onChanged} />
      {error && <div className="error">{error}</div>}
    </li>
  );
}

function LinkChips({
  atomId,
  links,
  onChanged,
}: {
  atomId: string;
  links: AtomView["links"];
  onChanged: () => void;
}) {
  const projects = useProjects();
  const nav = useNav();
  const linked = new Set(links.map((l) => l.project_id));
  const addable = projects.list.filter((p) => !linked.has(p.project.id));
  const run = (p: Promise<unknown>) => p.then(onChanged, onChanged);

  return (
    <div className="links">
      {links.map((l) => (
        <span
          key={l.project_id}
          className={`link-chip ${l.status}`}
          title={l.reason ?? (l.status === "confirmed" ? "Confirmed" : "Suggested")}
        >
          <button
            type="button"
            className="link-name"
            onClick={() => nav.go({ name: "project", id: l.project_id })}
          >
            {l.project_name}
            {l.status === "suggested" ? "?" : ""}
          </button>
          {l.status === "suggested" && (
            <button
              type="button"
              className="icon ok"
              aria-label={`Confirm link to ${l.project_name}`}
              onClick={() => run(api.confirmLink(atomId, l.project_id))}
            >
              ✓
            </button>
          )}
          <button
            type="button"
            className="icon"
            aria-label={`Reject link to ${l.project_name}`}
            onClick={() => run(api.rejectLink(atomId, l.project_id))}
          >
            ✕
          </button>
        </span>
      ))}
      {addable.length > 0 && (
        <select
          aria-label="Link to project"
          className="add-link"
          value=""
          onChange={(e) => e.target.value && run(api.confirmLink(atomId, e.target.value))}
        >
          <option value="">+ project</option>
          {addable.map((p) => (
            <option key={p.project.id} value={p.project.id}>
              {p.project.name}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}
