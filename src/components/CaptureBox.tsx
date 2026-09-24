import { forwardRef, useImperativeHandle, useRef, useState } from "react";
import { api, errorMessage } from "../api";
import type { CaptureView } from "../bindings/CaptureView";

export interface CaptureBoxHandle {
  focus: () => void;
}

interface Props {
  onSaved?: (c: CaptureView) => void;
}

/** The one obvious action (UX.md): keyboard-first, no metadata, instant confirmation. */
export const CaptureBox = forwardRef<CaptureBoxHandle, Props>(function CaptureBox(
  { onSaved },
  ref,
) {
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();
  const [savedAt, setSavedAt] = useState<number>();
  const area = useRef<HTMLTextAreaElement>(null);
  useImperativeHandle(ref, () => ({ focus: () => area.current?.focus() }));

  const empty = text.trim().length === 0;

  async function save() {
    if (empty || saving) return;
    setSaving(true);
    setError(undefined);
    try {
      const view = await api.saveCapture(text);
      // Clear only after the durable write succeeded.
      setText("");
      setSavedAt(Date.now());
      onSaved?.(view);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
      area.current?.focus();
    }
  }

  return (
    <form
      className="capture-box"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <label htmlFor="capture-text" className="capture-prompt">
        What's on your mind?
      </label>
      <textarea
        id="capture-text"
        ref={area}
        autoFocus
        value={text}
        rows={5}
        placeholder="Dump it here. Messy is fine."
        onChange={(e) => {
          setText(e.target.value);
          setSavedAt(undefined);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            void save();
          }
        }}
      />
      <div className="capture-actions">
        <span className="hint">Ctrl/⌘ + Enter to save</span>
        {savedAt && !error && (
          <span className="saved" role="status">
            Saved ✓
          </span>
        )}
        {error && (
          <span className="error" role="alert">
            Not saved: {error}
          </span>
        )}
        <button type="submit" className="primary" disabled={empty || saving}>
          {saving ? "Saving…" : "Save Thought"}
        </button>
      </div>
    </form>
  );
});
