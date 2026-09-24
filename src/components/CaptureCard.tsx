import { useState } from "react";
import { ATOM_TYPES, api, errorMessage } from "../api";
import type { AtomType } from "../bindings/AtomType";
import type { CaptureView } from "../bindings/CaptureView";
import type { ProjectSuggestion } from "../bindings/ProjectSuggestion";
import { formatWhen } from "../hooks";
import { useNav, useProjects } from "../nav";
import { AtomRow } from "./AtomRow";
import { StateBadge } from "./Chips";

interface Props {
  view: CaptureView;
  onChanged: () => void;
  /** Show interpretation expanded (detail view) rather than collapsed. */
  expanded?: boolean;
}

/** What the user wrote (authoritative) above what the system inferred (derived). */
export function CaptureCard({ view, onChanged, expanded = false }: Props) {
  const { capture, atoms, state, last_error } = view;
  const [open, setOpen] = useState(expanded);
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string>();
  const nav = useNav();

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
    <article className="capture-card">
      <header>
        <button
          type="button"
          className="link when"
          onClick={() => nav.go({ name: "capture", id: capture.id })}
          title={capture.captured_at}
        >
          {formatWhen(capture.captured_at)}
        </button>
        <StateBadge state={state} error={last_error} />
        <span className="spacer" />
        {state === "needs_retry" && (
          <button
            type="button"
            className="small"
            onClick={() => act(() => api.retryFailed(capture.id))}
          >
            Retry
          </button>
        )}
      </header>
      <div className="raw" title="What you wrote">
        {capture.text}
      </div>
      {last_error && state !== "processed" && (
        <div className="muted small-text">Last processing error: {last_error}</div>
      )}

      <section className="interpretation">
        <button type="button" className="link" onClick={() => setOpen(!open)} aria-expanded={open}>
          {open ? "▾" : "▸"} Interpretation · {atoms.length} atom{atoms.length === 1 ? "" : "s"}
          {view.pending_suggestions.length > 0 && " · project suggestion"}
        </button>
        {open && (
          <>
            <p className="derived-label">
              Generated{view.interpreted_by ? ` by ${view.interpreted_by}` : ""} — correct anything
              that's wrong.
            </p>
            {atoms.length === 0 && (
              <p className="muted">
                {state === "processed" ? "No atoms extracted." : "Not processed yet."}
              </p>
            )}
            <ul className="atoms">
              {atoms.map((a) => (
                <AtomRow key={a.atom.id} view={a} onChanged={onChanged} />
              ))}
            </ul>
            {view.pending_suggestions.map((s) => (
              <SuggestionRow key={s.id} s={s} onChanged={onChanged} />
            ))}
            {adding ? (
              <AddAtomForm
                onCancel={() => setAdding(false)}
                onAdd={(text, t) =>
                  act(async () => {
                    await api.addAtom(capture.id, text, t);
                    setAdding(false);
                  })
                }
              />
            ) : null}
            <div className="row-actions">
              <button type="button" className="small" onClick={() => setAdding(true)}>
                + Add missing atom
              </button>
              <button
                type="button"
                className="small"
                onClick={() => act(() => api.reprocessCapture(capture.id))}
              >
                Reprocess
              </button>
              <button
                type="button"
                className="small danger"
                onClick={() => act(() => api.trashCapture(capture.id))}
              >
                Delete
              </button>
            </div>
          </>
        )}
      </section>
      {error && <div className="error">{error}</div>}
    </article>
  );
}

export function SuggestionRow({ s, onChanged }: { s: ProjectSuggestion; onChanged: () => void }) {
  const projects = useProjects();
  const [name, setName] = useState(s.name);
  const done = () => {
    projects.reload();
    onChanged();
  };
  return (
    <div className="suggestion">
      <span>Create project</span>
      <input
        aria-label="Suggested project name"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <span>?</span>
      <button
        type="button"
        className="small primary"
        onClick={() => api.acceptSuggestion(s.id, name).then(done, done)}
      >
        Create
      </button>
      <button
        type="button"
        className="small"
        onClick={() => api.dismissSuggestion(s.id).then(done, done)}
      >
        Dismiss
      </button>
    </div>
  );
}

function AddAtomForm({
  onAdd,
  onCancel,
}: {
  onAdd: (text: string, t: AtomType) => void;
  onCancel: () => void;
}) {
  const [text, setText] = useState("");
  const [t, setT] = useState<AtomType>("spark");
  return (
    <form
      className="add-atom"
      onSubmit={(e) => {
        e.preventDefault();
        if (text.trim()) onAdd(text, t);
      }}
    >
      <select
        aria-label="New atom type"
        value={t}
        onChange={(e) => setT(e.target.value as AtomType)}
      >
        {ATOM_TYPES.map((x) => (
          <option key={x}>{x}</option>
        ))}
      </select>
      <input
        aria-label="New atom text"
        autoFocus
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <button type="submit" className="small primary">
        Add
      </button>
      <button type="button" className="small" onClick={onCancel}>
        Cancel
      </button>
    </form>
  );
}
