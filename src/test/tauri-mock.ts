import { invoke as coreInvoke } from "@tauri-apps/api/core";
import { vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

// biome-ignore lint/suspicious/noExplicitAny: test double accepts any command signature
export const invoke = vi.mocked(coreInvoke as (...args: any[]) => Promise<any>);
