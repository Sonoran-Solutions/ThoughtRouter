import { ATOM_TYPES } from "../api";
import type { AtomType } from "../bindings/AtomType";
import type { MatchKind } from "../bindings/MatchKind";
import type { Momentum } from "../bindings/Momentum";
import type { ProcessingState } from "../bindings/ProcessingState";

const STATE_LABEL: Record<ProcessingState, string> = {
  saved: "Saved",
  analyzing: "Analyzing…",
  processed: "Processed",
  needs_retry: "Needs retry",
};

export function StateBadge({ state, error }: { state: ProcessingState; error?: string | null }) {
  return (
    <span className={`badge state-${state}`} title={error ?? undefined}>
      {STATE_LABEL[state]}
    </span>
  );
}

export function TypeChip({ type }: { type: AtomType }) {
  return <span className={`type-chip t-${type}`}>{type}</span>;
}

export function TypeSelect({
  value,
  onChange,
  label,
}: {
  value: AtomType;
  onChange: (t: AtomType) => void;
  label: string;
}) {
  return (
    <select
      aria-label={label}
      className={`type-chip t-${value} type-select`}
      value={value}
      onChange={(e) => onChange(e.target.value as AtomType)}
    >
      {ATOM_TYPES.map((t) => (
        <option key={t} value={t}>
          {t}
        </option>
      ))}
    </select>
  );
}

export function MomentumChip({ momentum }: { momentum: Momentum }) {
  return <span className={`momentum m-${momentum}`}>{momentum}</span>;
}

export function MatchBadge({ matched }: { matched: MatchKind }) {
  const label =
    matched === "both" ? "words + meaning" : matched === "lexical" ? "words" : "meaning";
  return <span className="badge subtle">{label}</span>;
}

/** Renders FTS snippets, turning [hit] markers into <mark>. */
export function Snippet({ text }: { text: string }) {
  const parts = text.split(/(\[[^\]]*\])/g);
  return (
    <>
      {parts.map((p, i) =>
        p.startsWith("[") && p.endsWith("]") ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: static split of one string
          <mark key={i}>{p.slice(1, -1)}</mark>
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: static split of one string
          <span key={i}>{p}</span>
        ),
      )}
    </>
  );
}
