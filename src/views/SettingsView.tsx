import { useEffect, useState } from "react";
import { api, errorMessage } from "../api";
import type { ResurfaceWeights } from "../bindings/ResurfaceWeights";
import type { Settings } from "../bindings/Settings";
import { formatWhen, useAsync } from "../hooks";

const WEIGHT_LABELS: Record<keyof ResurfaceWeights, string> = {
  dormancy: "Dormancy (time untouched)",
  recurrence: "Recurrence (mentioned again)",
  active_project: "Active-project relevance",
  open_question: "Open question bonus",
  exploration: "Random exploration",
  not_now_penalty: "“Not now” penalty",
};

export function SettingsView() {
  const view = useAsync(() => api.getSettings(), []);
  const [draft, setDraft] = useState<Settings>();
  const [key, setKey] = useState("");
  const [msg, setMsg] = useState<string>();
  const [error, setError] = useState<string>();
  useEffect(() => {
    if (view.data) setDraft(view.data.settings);
  }, [view.data]);

  const run = async (fn: () => Promise<unknown>) => {
    setError(undefined);
    setMsg(undefined);
    try {
      const m = await fn();
      if (typeof m === "string") setMsg(m);
      view.reload();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  if (!draft || !view.data) return <h1>Settings</h1>;
  const v = view.data;
  const set = <K extends keyof Settings>(k: K, val: Settings[K]) =>
    setDraft({ ...draft, [k]: val });
  const dirty = JSON.stringify(draft) !== JSON.stringify(v.settings);

  return (
    <div className="settings">
      <h1>Settings</h1>
      {msg && <p className="banner info">{msg}</p>}
      {error && <p className="banner warn">{error}</p>}

      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            await api.saveSettings(draft);
            return "Settings saved.";
          });
        }}
      >
        <section className="panel">
          <h2>AI processing</h2>
          <label className="check">
            <input
              type="checkbox"
              checked={draft.processing_enabled}
              onChange={(e) => set("processing_enabled", e.target.checked)}
            />
            Process captures (off ⇒ thoughts are saved and searchable by words, nothing is sent
            anywhere)
          </label>
          <label>
            Provider
            <select
              value={draft.provider}
              onChange={(e) => set("provider", e.target.value as Settings["provider"])}
            >
              <option value="openrouter">OpenRouter</option>
              <option value="mock">Mock (offline heuristics, for trying the app)</option>
            </select>
          </label>
          {draft.provider === "openrouter" && (
            <>
              <label>
                Analyzer model id
                <input
                  placeholder="vendor/model-name (must support structured outputs)"
                  value={draft.analyzer_model}
                  onChange={(e) => set("analyzer_model", e.target.value)}
                />
              </label>
              <label>
                Embedding model id
                <input
                  placeholder="vendor/embedding-model (leave empty to disable meaning-based search)"
                  value={draft.embedding_model}
                  onChange={(e) => set("embedding_model", e.target.value)}
                />
              </label>
              <label className="check">
                <input
                  type="checkbox"
                  checked={draft.openrouter_zdr}
                  onChange={(e) => set("openrouter_zdr", e.target.checked)}
                />
                Zero-data-retention endpoints only (providers that train on or store prompts are
                always excluded)
              </label>
              <p className="muted small-text">
                Text is sent to OpenRouter only when processing is on. Embeddings in use:{" "}
                <code>{v.embedding_model_id ?? "none"}</code>
              </p>
            </>
          )}
        </section>

        <section className="panel">
          <h2>Resurfacing</h2>
          <label>
            Similarity threshold for “same idea again”
            <input
              type="number"
              step="0.01"
              min="0"
              max="1"
              value={draft.similarity_threshold}
              onChange={(e) => set("similarity_threshold", Number(e.target.value))}
            />
          </label>
          <label>
            Rest an item for (days) after showing it
            <input
              type="number"
              min="0"
              value={draft.resurface_suppress_days}
              onChange={(e) => set("resurface_suppress_days", Number(e.target.value))}
            />
          </label>
          <details>
            <summary>Scoring weights</summary>
            {(Object.keys(WEIGHT_LABELS) as (keyof ResurfaceWeights)[]).map((k) => (
              <label key={k}>
                {WEIGHT_LABELS[k]}
                <input
                  type="number"
                  step="0.1"
                  value={draft.resurface_weights[k]}
                  onChange={(e) =>
                    set("resurface_weights", {
                      ...draft.resurface_weights,
                      [k]: Number(e.target.value),
                    })
                  }
                />
              </label>
            ))}
          </details>
        </section>

        <section className="panel">
          <h2>Capture</h2>
          <label>
            Global shortcut
            <input
              value={draft.global_shortcut}
              onChange={(e) => set("global_shortcut", e.target.value)}
            />
          </label>
        </section>

        <div className="sticky-save">
          <button type="submit" className="primary" disabled={!dirty}>
            Save settings
          </button>
        </div>
      </form>

      <section className="panel">
        <h2>OpenRouter API key</h2>
        <p className="muted">
          {v.api_key_source === "env"
            ? "Using OPENROUTER_API_KEY from the environment."
            : v.api_key_source === "keychain"
              ? "A key is stored in your OS keychain."
              : "No key set. Captures are saved and wait in the queue until one is added."}
        </p>
        <form
          className="inline"
          onSubmit={(e) => {
            e.preventDefault();
            void run(async () => {
              await api.setApiKey(key);
              setKey("");
              return "API key saved to the OS keychain.";
            });
          }}
        >
          <input
            type="password"
            aria-label="OpenRouter API key"
            placeholder="sk-or-…"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            autoComplete="off"
          />
          <button type="submit" disabled={!key.trim()}>
            Save key
          </button>
          {v.api_key_source === "keychain" && (
            <button
              type="button"
              className="danger"
              onClick={() => run(async () => void (await api.clearApiKey()))}
            >
              Remove key
            </button>
          )}
        </form>
      </section>

      <DataSection dataDir={v.data_dir} run={run} />
    </div>
  );
}

function DataSection({
  dataDir,
  run,
}: {
  dataDir: string;
  run: (fn: () => Promise<unknown>) => Promise<void>;
}) {
  const trash = useAsync(() => api.listTrash(), []);
  const [lastPath, setLastPath] = useState<string>();

  return (
    <section className="panel">
      <h2>Your data</h2>
      <p className="muted small-text">
        Stored locally in <code>{dataDir}</code>. A backup is written on every start.
      </p>
      <div className="row-actions">
        <button
          type="button"
          onClick={() =>
            run(async () => {
              const p = await api.backupNow();
              setLastPath(p);
              return `Backup written: ${p}`;
            })
          }
        >
          Back up now
        </button>
        <button
          type="button"
          onClick={() =>
            run(async () => {
              const r = await api.exportMarkdown();
              setLastPath(r.directory);
              return `Exported ${r.files.length} Markdown file(s) to ${r.directory}`;
            })
          }
        >
          Export Markdown
        </button>
        <button
          type="button"
          onClick={() =>
            run(async () => {
              const r = await api.exportJson();
              setLastPath(r.files[0]);
              return `Exported JSON to ${r.files[0]}`;
            })
          }
        >
          Export JSON
        </button>
        <button
          type="button"
          onClick={() => run(async () => (await api.importJson()) ?? undefined)}
        >
          Import JSON…
        </button>
        {lastPath && (
          <button
            type="button"
            className="link"
            onClick={() => run(() => api.revealPath(lastPath))}
          >
            Show in folder
          </button>
        )}
      </div>
      <div className="row-actions">
        <button
          type="button"
          onClick={() =>
            run(async () => `Queued ${await api.reprocessAll()} captures for reprocessing.`)
          }
        >
          Reprocess all captures
        </button>
        <button
          type="button"
          onClick={() => run(async () => `Retrying ${await api.retryFailed()} failed step(s).`)}
        >
          Retry failed processing
        </button>
      </div>

      <h3>Trash</h3>
      {trash.data?.length === 0 && (
        <p className="muted">Empty. Trashed thoughts are purged after 30 days.</p>
      )}
      <ul className="plain">
        {trash.data?.map((c) => (
          <li key={c.id} className="trash-item">
            <span className="raw small">{c.text.slice(0, 160)}</span>
            <span className="muted small-text">
              deleted {c.deleted_at && formatWhen(c.deleted_at)}
            </span>
            <button
              type="button"
              className="small"
              onClick={() => run(() => api.restoreCapture(c.id)).then(trash.reload)}
            >
              Restore
            </button>
            <button
              type="button"
              className="small danger"
              onClick={() => {
                if (confirm("Permanently delete this thought and everything derived from it?"))
                  void run(() => api.purgeCapture(c.id)).then(trash.reload);
              }}
            >
              Delete forever
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
