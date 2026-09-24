import { useEffect, useState } from "react";
import { api, errorMessage } from "../api";
import type { CaptureView } from "../bindings/CaptureView";
import { CaptureCard } from "../components/CaptureCard";
import { useAsync } from "../hooks";

const PAGE = 30;

/** Chronological raw captures (UX.md "Capture history"). */
export function HistoryView() {
  const first = useAsync(() => api.listCaptures(undefined, PAGE), []);
  const [more, setMore] = useState<CaptureView[]>([]);
  const [done, setDone] = useState(false);
  const [error, setError] = useState<string>();
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset extra pages whenever page one reloads
  useEffect(() => {
    setMore([]);
    setDone(false);
  }, [first.data]);

  const items = [...(first.data ?? []), ...more];
  async function loadMore() {
    const last = items[items.length - 1];
    if (!last) return;
    try {
      const page = await api.listCaptures(last.capture.id, PAGE);
      setMore((m) => [...m, ...page]);
      if (page.length < PAGE) setDone(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <div>
      <h1>History</h1>
      {first.error && <p className="error">{first.error}</p>}
      {items.length === 0 && !first.loading && <p className="muted">No captures yet.</p>}
      {items.map((v) => (
        <CaptureCard key={v.capture.id} view={v} onChanged={first.reload} />
      ))}
      {items.length >= PAGE && !done && (
        <button type="button" onClick={loadMore}>
          Load older
        </button>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}
