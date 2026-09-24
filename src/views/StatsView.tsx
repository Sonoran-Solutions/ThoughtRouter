import { api } from "../api";
import type { CountRow } from "../bindings/CountRow";
import { useAsync } from "../hooks";

const pct = (x: number | null | undefined) => (x == null ? "—" : `${Math.round(x * 100)}%`);

function Bars({ rows }: { rows: CountRow[] }) {
  const max = Math.max(1, ...rows.map((r) => r.count));
  return (
    <ul className="bars">
      {rows.map((r) => (
        <li key={r.label}>
          <span className="bar-label">{r.label}</span>
          <span className="bar" style={{ width: `${(r.count / max) * 100}%` }} />
          <span className="bar-value">{r.count}</span>
        </li>
      ))}
    </ul>
  );
}

/** Dogfooding metrics against the MVP success criteria. */
export function StatsView() {
  const s = useAsync(() => api.stats(), []).data;
  if (!s) return <h1>Stats</h1>;
  return (
    <div>
      <h1>Stats</h1>
      <div className="tiles">
        <div className="tile">
          <span className="tile-value">{s.total_captures}</span>
          <span className="tile-label">captures</span>
        </div>
        <div className="tile">
          <span className="tile-value">{pct(s.atom_correction_rate)}</span>
          <span className="tile-label">atoms corrected (target ≤ 20%)</span>
        </div>
        <div className="tile">
          <span className="tile-value">{pct(s.link_confirm_rate)}</span>
          <span className="tile-label">AI links confirmed (target ≥ 70%)</span>
        </div>
      </div>
      <section className="panel">
        <h2>Captures per week</h2>
        <Bars rows={s.captures_per_week} />
      </section>
      <section className="panel">
        <h2>Project links</h2>
        {s.links_by_status.length ? (
          <Bars rows={s.links_by_status} />
        ) : (
          <p className="muted">No links yet.</p>
        )}
      </section>
      <section className="panel">
        <h2>Resurfacing responses</h2>
        {s.resurfacing_responses.length ? (
          <Bars rows={s.resurfacing_responses} />
        ) : (
          <p className="muted">Nothing resurfaced yet.</p>
        )}
      </section>
    </div>
  );
}
