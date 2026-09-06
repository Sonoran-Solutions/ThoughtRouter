# Decision Log

This file records important project decisions so they do not have to be reconstructed from memory later.

Use short entries. If a decision becomes complex enough to deserve a full ADR, link one from here.

---

## D-001 — Build for one brain first

**Status:** Accepted

ThoughtRouter is a personal tool first. Public release may happen later, but current design decisions should optimize for actual personal usefulness rather than generic market requirements.

**Consequences:**

- no account system or team model in MVP;
- no billing/SaaS architecture;
- weirdly specific workflows are allowed if they genuinely help the primary user;
- productization waits until dogfooding proves value.

---

## D-002 — Capture first, structure later

**Status:** Accepted

The capture flow will not require project, tag, category, priority, status, or due-date selection.

**Reason:** Organizational ceremony is part of the failure mode this project is meant to solve.

---

## D-003 — Raw captures are immutable source data

**Status:** Accepted

AI-generated atoms, summaries, classifications, and relationships must be stored separately from raw captures.

**Consequences:**

- AI processing can be rerun safely;
- model mistakes are reversible;
- provenance stays clear;
- future model changes do not rewrite history.

---

## D-004 — Projects are clusters, not folders

**Status:** Accepted

Project membership is many-to-many and represented as relationships.

A thought may belong to several projects or no project at all.

---

## D-005 — Dormant is a valid state

**Status:** Accepted

An idea can be intentionally inactive without being treated as failed, overdue, or abandoned.

The system should preserve dormant context and occasionally resurface it when useful.

---

## D-006 — Local-first architecture

**Status:** Accepted for initial implementation

SQLite is the proposed source database, with a lightweight desktop client and provider-agnostic AI integration.

**Reason:** Personal thought history should remain usable without running a server or depending on a hosted service.

---

## D-007 — Tauri + React + TypeScript is the initial stack direction

**Status:** Proposed / accepted unless implementation friction says otherwise

The initial desktop app will target Tauri with a React + TypeScript frontend.

This is not ideological. If the stack creates more friction than it removes, replace it.

---

## D-008 — SQLite-native semantic retrieval first

**Status:** Proposed

Use SQLite FTS plus `sqlite-vec` (or equivalent SQLite-native vector support) before considering a separate search/vector service.

**Reason:** The expected personal dataset does not justify extra infrastructure.

---

## D-009 — AI providers live behind an interface

**Status:** Accepted

The application core depends on a `ThoughtProcessor` abstraction.

Initial implementations may include a remote provider, a local provider later, and a deterministic mock for tests.

No domain logic should depend directly on one vendor SDK.

---

## D-010 — AI processing cannot block capture

**Status:** Accepted

The raw capture must be durably stored before model processing begins.

A model outage is a processing failure, not a capture failure.

---

## D-011 — Resurfacing is core, not optional polish

**Status:** Accepted

The project is not successful merely because it stores and categorizes notes.

It must help recover useful forgotten thoughts.

---

## D-012 — Avoid conventional backlog guilt

**Status:** Accepted

ThoughtRouter should not create concepts like "37 overdue thoughts."

The home screen should prioritize relevance, recurrence, forgotten context, and useful connections over an ever-growing task count.

---

## D-013 — Integrations wait until the core loop works

**Status:** Accepted

GitHub, Android capture surfaces, voice, ChatGPT ingestion, and agent handoffs are deliberately parked until the desktop capture/understand/connect/resurface loop proves itself through real use.

**Reason:** Integrations are attractive scope-creep magnets and do not solve a broken core model.

---

## How to add a decision

Copy this template:

```md
## D-XXX — Decision title

**Status:** Proposed | Accepted | Rejected | Superseded

Decision and context.

**Reason:** Why this choice was made.

**Consequences:**

- consequence one;
- consequence two.
```
