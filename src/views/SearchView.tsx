import { useEffect, useState } from "react";
import { api, errorMessage } from "../api";
import type { SearchResponse } from "../bindings/SearchResponse";
import { MatchBadge, Snippet, TypeChip } from "../components/Chips";
import { formatWhen } from "../hooks";
import { useNav } from "../nav";

/** Natural-language search over captures and atoms (words + meaning). */
export function SearchView({ initial = "" }: { initial?: string }) {
  const [q, setQ] = useState(initial);
  const [res, setRes] = useState<SearchResponse>();
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const nav = useNav();

  useEffect(() => {
    if (!q.trim()) {
      setRes(undefined);
      return;
    }
    let live = true;
    const t = setTimeout(async () => {
      setBusy(true);
      try {
        const r = await api.search(q);
        if (live) {
          setRes(r);
          setError(undefined);
        }
      } catch (e) {
        if (live) setError(errorMessage(e));
      } finally {
        if (live) setBusy(false);
      }
    }, 300);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [q]);

  return (
    <div>
      <h1>Search</h1>
      <input
        className="search-input"
        type="search"
        autoFocus
        aria-label="Search your thoughts"
        placeholder="e.g. that idea about a linux control app for the handheld"
        value={q}
        onChange={(e) => setQ(e.target.value)}
      />
      {busy && <p className="muted">Searching…</p>}
      {res?.semantic_note && <p className="muted small-text">{res.semantic_note}</p>}
      {error && <p className="error">{error}</p>}
      {res && res.results.length === 0 && <p className="muted">No matches.</p>}
      <ul className="results">
        {res?.results.map((r) => (
          <li key={`${r.kind}-${r.id}`}>
            <button
              type="button"
              className="result"
              onClick={() => nav.go({ name: "capture", id: r.capture_id })}
            >
              <div className="result-head">
                {r.kind === "atom" && r.atom_type ? (
                  <TypeChip type={r.atom_type} />
                ) : (
                  <span className="badge">capture</span>
                )}
                <MatchBadge matched={r.matched} />
                <span className="muted small-text">{formatWhen(r.captured_at)}</span>
              </div>
              <div className={r.kind === "capture" ? "raw small" : "derived-text"}>
                {r.snippet ? <Snippet text={r.snippet} /> : r.text.slice(0, 280)}
              </div>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
