import { api } from "../api";
import type { ProcessingStatus } from "../bindings/ProcessingStatus";
import { useNav } from "../nav";

/** Makes processing state visible without ever implying a capture was lost. */
export function ProcessingBanner({
  status,
  onChanged,
}: {
  status: ProcessingStatus;
  onChanged: () => void;
}) {
  const nav = useNav();
  if (status.blocked_reason) {
    return (
      <div className="banner warn" role="status">
        Your thoughts are saved. {status.blocked_reason}
        {status.queued > 0 && ` (${status.queued} waiting)`}{" "}
        <button type="button" className="link" onClick={() => nav.go({ name: "settings" })}>
          Open Settings
        </button>
      </div>
    );
  }
  if (status.failed > 0) {
    return (
      <div className="banner warn" role="status">
        {status.failed} processing step{status.failed === 1 ? "" : "s"} failed.{" "}
        <button type="button" className="link" onClick={() => api.retryFailed().then(onChanged)}>
          Retry all
        </button>
      </div>
    );
  }
  if (status.queued + status.running > 0) {
    return (
      <div className="banner info" role="status">
        Processing{status.provider === "openrouter" ? " via OpenRouter" : " (mock)"}…{" "}
        {status.queued + status.running} step{status.queued + status.running === 1 ? "" : "s"} left
      </div>
    );
  }
  return null;
}
