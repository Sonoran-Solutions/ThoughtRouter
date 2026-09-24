import { listen } from "@tauri-apps/api/event";
import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { errorMessage } from "./api";

/** Bumps whenever background processing changes data; views refetch on it. */
export const DataVersion = createContext(0);

/** Listens for backend processing events and returns a debounced counter. */
export function useProcessingVersion(): number {
  const [version, setVersion] = useState(0);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen("processing-updated", () => {
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setVersion((v) => v + 1), 250);
    })
      .then((u) => {
        if (cancelled) u();
        else unlisten = u;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
      clearTimeout(timer.current);
    };
  }, []);
  return version;
}

export interface AsyncState<T> {
  data?: T;
  error?: string;
  loading: boolean;
  reload: () => void;
}

/**
 * Runs `fn` on mount, when `deps` change, and when background processing
 * reports new data. Keeps showing previous data while reloading.
 */
export function useAsync<T>(fn: () => Promise<T>, deps: unknown[]): AsyncState<T> {
  const version = useContext(DataVersion);
  const [tick, setTick] = useState(0);
  const [state, setState] = useState<{ data?: T; error?: string; loading: boolean }>({
    loading: true,
  });
  const fnRef = useRef(fn);
  fnRef.current = fn;
  // biome-ignore lint/correctness/useExhaustiveDependencies: deps are forwarded by the caller
  useEffect(() => {
    let live = true;
    setState((s) => ({ ...s, loading: true }));
    fnRef.current().then(
      (data) => live && setState({ data, loading: false }),
      (e) => live && setState((s) => ({ ...s, error: errorMessage(e), loading: false })),
    );
    return () => {
      live = false;
    };
  }, [...deps, version, tick]);
  const reload = useCallback(() => setTick((t) => t + 1), []);
  return { ...state, reload };
}

export function formatWhen(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function relativeDays(iso: string): string {
  const days = Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 45) return `${days} days ago`;
  const months = Math.round(days / 30);
  if (months < 18) return `${months} months ago`;
  return `${Math.round(days / 365)} years ago`;
}
