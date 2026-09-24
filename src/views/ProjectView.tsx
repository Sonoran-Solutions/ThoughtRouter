import { useEffect, useState } from "react";
import { api, errorMessage, MOMENTA } from "../api";
import type { Momentum } from "../bindings/Momentum";
import type { ProjectPage } from "../bindings/ProjectPage";
import { AtomRow } from "../components/AtomRow";
import { formatWhen, useAsync } from "../hooks";
import { useNav, useProjects } from "../nav";

/** Project / thought-thread page: synthesis is convenience, the timeline is evidence. */
export function ProjectView({ id }: { id: string }) {
  const page = useAsync(() => api.projectPage(id), [id]);
  const nav = useNav();
  const p = page.data;

  return (
    <div>
      <button type="button" className="link" onClick={nav.back}>
        ← Back
      </button>
      {page.error && <p className="error">{page.error}</p>}
      {p && (
        <>
          <ProjectHeader page={p} onChanged={page.reload} />
          <Synthesis page={p} onChanged={page.reload} />
          <section>
            <h2>Timeline</h2>
            {p.timeline.length === 0 && (
              <p className="muted">
                No linked thoughts yet. Links appear as new captures are processed.
              </p>
            )}
            <ol className="timeline">
              {p.timeline.map((e) => (
                <li key={e.capture.id} id={`cap-${e.capture.id}`}>
                  <button
                    type="button"
                    className="link when"
                    onClick={() => nav.go({ name: "capture", id: e.capture.id })}
                  >
                    {formatWhen(e.capture.captured_at)}
                  </button>
                  <div className="raw">{e.capture.text}</div>
                  <ul className="atoms">
                    {e.atoms.map((a) => (
                      <AtomRow key={a.atom.id} view={a} onChanged={page.reload} />
                    ))}
                  </ul>
                </li>
              ))}
            </ol>
          </section>
        </>
      )}
    </div>
  );
}

function ProjectHeader({ page, onChanged }: { page: ProjectPage; onChanged: () => void }) {
  const { project } = page;
  const projects = useProjects();
  const nav = useNav();
  const [name, setName] = useState(project.name);
  const [description, setDescription] = useState(project.description);
  const [momentum, setMomentum] = useState<Momentum>(project.momentum);
  const [error, setError] = useState<string>();
  useEffect(() => {
    setName(project.name);
    setDescription(project.description);
    setMomentum(project.momentum);
  }, [project]);

  const dirty =
    name !== project.name || description !== project.description || momentum !== project.momentum;
  async function save() {
    try {
      await api.updateProject(project.id, name, description, momentum);
      projects.reload();
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }
  async function remove() {
    if (
      !confirm(
        `Delete project “${project.name}”? Thoughts stay; only the project and its links go.`,
      )
    )
      return;
    await api.deleteProject(project.id);
    projects.reload();
    nav.go({ name: "projects" });
  }

  return (
    <form
      className="project-header"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <input
        className="title-input"
        aria-label="Project name"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <input
        aria-label="Project description"
        placeholder="Description"
        value={description}
        onChange={(e) => setDescription(e.target.value)}
      />
      <label>
        Momentum{" "}
        <select value={momentum} onChange={(e) => setMomentum(e.target.value as Momentum)}>
          {MOMENTA.map((m) => (
            <option key={m}>{m}</option>
          ))}
        </select>
      </label>
      {dirty && (
        <button type="submit" className="small primary">
          Save
        </button>
      )}
      <button type="button" className="small danger" onClick={remove}>
        Delete project
      </button>
      {error && <p className="error">{error}</p>}
    </form>
  );
}

function Synthesis({ page, onChanged }: { page: ProjectPage; onChanged: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const s = page.synthesis;

  async function summarize() {
    setBusy(true);
    setError(undefined);
    try {
      await api.summarizeProject(page.project.id);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const list = (title: string, items: string[]) =>
    items.length > 0 && (
      <>
        <h3>{title}</h3>
        <ul>
          {items.map((x) => (
            <li key={x}>{x}</li>
          ))}
        </ul>
      </>
    );

  return (
    <section className="panel synthesis">
      <div className="panel-head">
        <h2>Current understanding</h2>
        <button type="button" onClick={summarize} disabled={busy || page.timeline.length === 0}>
          {busy ? "Summarizing…" : s ? "Re-summarize" : "Summarize"}
        </button>
      </div>
      {error && <p className="error">{error}</p>}
      {!s && (
        <p className="muted">
          Generate a summary of where this project stands, from its timeline below.
        </p>
      )}
      {s && (
        <div className="derived">
          <p className="derived-label">
            Generated {formatWhen(s.created_at)}
            {s.model && ` by ${s.model}`} from {s.source_capture_ids.length} capture
            {s.source_capture_ids.length === 1 ? "" : "s"}.
            {s.stale && <strong> Newer thoughts exist — re-summarize to include them.</strong>}
          </p>
          <p>{s.body.summary}</p>
          {list("Open questions", s.body.open_questions)}
          {list("Possible next actions", s.body.possible_next_actions)}
          {list("Decisions", s.body.decisions)}
          {list("Changed assumptions", s.body.changed_assumptions)}
        </div>
      )}
    </section>
  );
}
