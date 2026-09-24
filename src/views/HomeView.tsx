import type { RefObject } from "react";
import { api } from "../api";
import { CaptureBox, type CaptureBoxHandle } from "../components/CaptureBox";
import { CaptureCard, SuggestionRow } from "../components/CaptureCard";
import { MomentumChip, TypeChip } from "../components/Chips";
import { ProcessingBanner } from "../components/ProcessingBanner";
import { ResurfacePanel } from "../components/Resurface";
import { relativeDays, useAsync } from "../hooks";
import { useNav } from "../nav";

/** "What from my own brain is useful to me right now?" (PRODUCT.md) */
export function HomeView({ captureRef }: { captureRef: RefObject<CaptureBoxHandle | null> }) {
  const home = useAsync(() => api.home(), []);
  const nav = useNav();
  const d = home.data;

  return (
    <div className="home">
      <CaptureBox ref={captureRef} onSaved={home.reload} />
      {d && <ProcessingBanner status={d.processing} onChanged={home.reload} />}
      {home.error && <p className="error">{home.error}</p>}

      <div className="home-grid">
        <ResurfacePanel />

        {d && d.active_projects.length > 0 && (
          <section className="panel">
            <h2>Right now</h2>
            <ul className="plain">
              {d.active_projects.map((p) => (
                <li key={p.project.id}>
                  <button
                    type="button"
                    className="link"
                    onClick={() => nav.go({ name: "project", id: p.project.id })}
                  >
                    {p.project.name}
                  </button>{" "}
                  <MomentumChip momentum={p.project.momentum} />{" "}
                  <span className="muted">
                    {p.confirmed_atoms + p.suggested_atoms} thoughts
                    {p.last_activity && ` · last ${relativeDays(p.last_activity)}`}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {d && d.recurring.length > 0 && (
          <section className="panel">
            <h2>You keep coming back to this</h2>
            <ul className="plain">
              {d.recurring.map((r) => (
                <li key={r.atom.id}>
                  <TypeChip type={r.atom.atom_type} />{" "}
                  <button
                    type="button"
                    className="link derived-text"
                    onClick={() => nav.go({ name: "capture", id: r.atom.capture_id })}
                  >
                    {r.atom.text}
                  </button>{" "}
                  <span className="muted">· {r.mentions}×</span>
                </li>
              ))}
            </ul>
          </section>
        )}

        {d && d.pending_suggestions.length > 0 && (
          <section className="panel">
            <h2>Possible new projects</h2>
            {d.pending_suggestions.map((s) => (
              <SuggestionRow key={s.id} s={s} onChanged={home.reload} />
            ))}
          </section>
        )}
      </div>

      <section className="recent">
        <div className="panel-head">
          <h2>Recently captured</h2>
          <button type="button" className="link" onClick={() => nav.go({ name: "history" })}>
            All history →
          </button>
        </div>
        {d?.recent.length === 0 && (
          <p className="muted">Nothing yet. Your first thought goes in the box above.</p>
        )}
        {d?.recent.map((v) => (
          <CaptureCard key={v.capture.id} view={v} onChanged={home.reload} />
        ))}
      </section>
    </div>
  );
}
