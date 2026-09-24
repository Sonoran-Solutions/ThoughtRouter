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

## D-014 — Rust owns the application core

**Status:** Accepted

Persistence (`rusqlite`), the background job worker, AI adapters, search, and resurfacing scoring live in the Tauri Rust backend. React is a view layer that calls typed Tauri commands; TypeScript types are generated from Rust structs.

**Reason:** Background jobs must survive window close/reload, SQLite extensions and FTS5 are simplest from Rust, and API keys can live in the OS keychain instead of webview memory.

**Consequences:**

- two languages, mitigated by generated types;
- the AI abstraction is a pair of Rust traits (`Analyzer`, `Embedder`), not a TS interface.

---

## D-015 — Brute-force vectors before `sqlite-vec`

**Status:** Proposed (amends D-008)

Store embeddings as BLOBs keyed by `(object_type, object_id, model)` and compute cosine similarity in Rust. Adopt `sqlite-vec` only when measurement shows it is needed.

**Reason:** A personal corpus of a few thousand vectors is milliseconds to scan, and fixed-dimension vector tables make embedding-model changes painful.

---

## D-016 — Dogfooding starts at M1

**Status:** Proposed

Daily use begins as soon as raw capture + search + backup work. AI processing is applied to the accumulated backlog later.

**Reason:** Resurfacing needs a real corpus, and immutable captures make later reprocessing free.

---

## D-017 — Projects are user-confirmed entities

**Status:** Proposed

The user seeds projects. The AI suggests atom→project links and new projects, but never creates a project without a click.

**Reason:** Avoids project sprawl and the cold-start problem of clustering over an empty database.

---

## D-018 — Captures are non-editable but deletable

**Status:** Proposed

Captures cannot be edited by anyone (enforced by a SQLite trigger). The user can delete a capture (tombstone, then purge), which cascades to its derived data.

**Reason:** Immutability protects against AI rewriting history; it must not trap accidentally captured secrets.

---

## D-019 — User corrections are source data

**Status:** Proposed

User-created or user-edited atoms carry `origin = 'user'`; rejected links persist as `status = 'rejected'`. Reprocessing supersedes only AI-originated rows and never re-suggests rejected links.

**Reason:** Reconciles "derived data is regenerable" with "user corrections outrank AI".

---

## D-020 — OpenRouter is the first AI provider

**Status:** Accepted

The first real `Analyzer` and `Embedder` adapters target OpenRouter (OpenAI-compatible `/api/v1/chat/completions` and `/api/v1/embeddings`). Model IDs are configuration, not code.

**Reason:** One API key and one adapter give access to many models, so model choice can be iterated on without new adapters.

**Consequences:**

- structured output requests use `response_format: { type: "json_schema", json_schema: { strict: true, ... } }` with `provider.require_parameters = true` so requests are only routed to upstreams that honor the schema;
- privacy defaults: `provider.data_collection = "deny"` (and `zdr = true` where the chosen model supports it), exposed as settings;
- the processor still validates every response against the schema; routing is not trusted blindly;
- `processor_runs` records the model id OpenRouter reports actually serving the request;
- embeddings are keyed by model id, so switching embedding models means a re-embed job, not a migration;
- the `Analyzer`/`Embedder` split is kept so a local embedder or direct provider can replace either side later.

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
