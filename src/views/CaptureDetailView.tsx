import { api } from "../api";
import { CaptureCard } from "../components/CaptureCard";
import { TypeChip } from "../components/Chips";
import { formatWhen, relativeDays, useAsync } from "../hooks";
import { useNav } from "../nav";

export function CaptureDetailView({ id }: { id: string }) {
  const view = useAsync(() => api.getCapture(id), [id]);
  const related = useAsync(() => api.related(id), [id]);
  const nav = useNav();

  return (
    <div>
      <button type="button" className="link" onClick={nav.back}>
        ← Back
      </button>
      {view.error && <p className="error">{view.error}</p>}
      {view.data?.capture.deleted_at && (
        <p className="banner warn">This capture is in the trash.</p>
      )}
      {view.data && <CaptureCard view={view.data} onChanged={view.reload} expanded />}

      <section className="panel">
        <h2>Possibly related</h2>
        {related.data?.length === 0 && (
          <p className="muted">Nothing similar yet (needs an embedding model, or more captures).</p>
        )}
        <ul className="plain related">
          {related.data?.map((r) => (
            <li key={r.atom.id}>
              <TypeChip type={r.atom.atom_type} />{" "}
              <button
                type="button"
                className="link derived-text"
                onClick={() => nav.go({ name: "capture", id: r.capture.id })}
              >
                {r.atom.text}
              </button>
              <div className="muted small-text">
                “{r.capture.text.slice(0, 140)}
                {r.capture.text.length > 140 ? "…" : ""}” · {formatWhen(r.capture.captured_at)} (
                {relativeDays(r.capture.captured_at)}) · similarity {r.similarity.toFixed(2)}
              </div>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
