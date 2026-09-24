import { useState } from "react";
import { api, errorMessage, MOMENTA } from "../api";
import type { Momentum } from "../bindings/Momentum";
import { MomentumChip } from "../components/Chips";
import { relativeDays } from "../hooks";
import { useNav, useProjects } from "../nav";

/** Projects are clusters you confirm, not folders you file into (D-017). */
export function ProjectsView() {
  const projects = useProjects();
  const nav = useNav();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [momentum, setMomentum] = useState<Momentum>("exploring");
  const [error, setError] = useState<string>();

  async function create() {
    setError(undefined);
    try {
      await api.createProject(name, description, momentum);
      setName("");
      setDescription("");
      projects.reload();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <div>
      <h1>Projects</h1>
      <p className="muted">
        Seed the projects you already know about. New thoughts get linked to them automatically; you
        confirm or reject each link.
      </p>
      <form
        className="panel project-form"
        onSubmit={(e) => {
          e.preventDefault();
          void create();
        }}
      >
        <input
          aria-label="Project name"
          placeholder="Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <input
          aria-label="Project description"
          placeholder="One line: what is it?"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
        <select
          aria-label="Momentum"
          value={momentum}
          onChange={(e) => setMomentum(e.target.value as Momentum)}
        >
          {MOMENTA.map((m) => (
            <option key={m}>{m}</option>
          ))}
        </select>
        <button type="submit" className="primary" disabled={!name.trim()}>
          Add project
        </button>
        {error && <p className="error">{error}</p>}
      </form>
      {projects.list.length === 0 && <p className="muted">No projects yet.</p>}
      <ul className="project-list">
        {projects.list.map((p) => (
          <li key={p.project.id}>
            <button
              type="button"
              className="result"
              onClick={() => nav.go({ name: "project", id: p.project.id })}
            >
              <div className="result-head">
                <strong>{p.project.name}</strong> <MomentumChip momentum={p.project.momentum} />
              </div>
              {p.project.description && <div className="muted">{p.project.description}</div>}
              <div className="muted small-text">
                {p.confirmed_atoms} confirmed · {p.suggested_atoms} suggested
                {p.last_activity && ` · last thought ${relativeDays(p.last_activity)}`}
              </div>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
