# MVP Review & Plan

Written 2026-09-24 against commit `a0189fe` (docs only, no code yet).

This document has two parts:

1. **Review:** what is strong about the current design, and what needs fixing before any code is written.
2. **Plan:** a narrower MVP, cut into four milestones you can use day to day, with a concrete schema and exit criteria.

> **Status (2026-09-24): M1–M4 are implemented.** See "Implementation notes" at the end for where the build differs from this plan. Next step: choose default models, then dogfood.

Decisions raised here are recorded in `DECISIONS.md`. **D-014 (Rust core)** and **D-020 (OpenRouter)** are accepted. D-015 to D-019 are still *Proposed*.

---

# Part 1 — Review

## Current state

| Area | State |
|---|---|
| Product vision (`README`, `PRODUCT`, `UX`) | Strong, coherent, opinionated |
| Architecture / data model / AI pipeline | Good principles, several unresolved seams (below) |
| Roadmap / tasks | Thorough, but the MVP is defined in three slightly different ways |
| Fixtures (`EXAMPLES`) | Good cases, not machine-readable yet |
| Code, tooling, CI | None |

## What is already right (keep it)

- **Raw captures are immutable, and derived data can be thrown away and rebuilt.** This is the most important decision in the repo. It also opens a door that isn't used yet: *captures can be collected long before the AI exists, and processed later.*
- AI is never allowed to block a save (D-010).
- AI vendors sit behind an interface, with a mock for tests (D-009).
- Resurfacing is treated as a core feature (D-011), with deterministic scoring rather than asking an LLM what to show.
- Fixtures assert structure, not exact wording.
- The anti-goals and the scope-control rule are explicit.

## Issues, by severity

### A. Architectural gaps to settle before writing code

**A1. The docs never say where the application core runs.**
`ARCHITECTURE.md` sketches `ThoughtProcessor` as a TypeScript interface. It also picks Tauri, whose backend is Rust. The job queue, SQLite access, secret storage and vector extension all behave differently depending on the answer:

| Core in the webview (TS + `tauri-plugin-sql`) | Core in Rust (`rusqlite`), UI calls Tauri commands |
|---|---|
| Jobs die when the window closes or reloads | Jobs run in a background task, independent of the UI |
| Loading SQLite extensions is awkward | FTS5 comes bundled; `sqlite-vec` has a Rust crate |
| API keys live in JS memory | OS keychain via the `keyring` crate |
| One language | Two languages (mitigated by generating TS types from Rust) |

→ **D-014 (accepted):** Rust owns persistence, jobs, AI adapters and scoring. React is a thin view layer. Generate TS types from the Rust structs (`tauri-specta` or `ts-rs`) so the two sides can't drift apart.

**A2. User corrections conflict with "derived data can be regenerated".**
Suppose the user changes an atom's type and later reprocesses the capture. Today the atoms get deleted and regenerated, and the correction is lost. That breaks the "user outranks AI" rule.
→ Record a correction as *source* data: `atoms.origin = 'user'` for user-created or user-edited atoms. Reprocessing marks old AI atoms `superseded` and never touches user-originated rows. Rejected links stay stored as `rejected`, so the AI can't suggest them again.

**A3. "Projects are clusters", but nothing describes a clustering mechanism.**
In practice the pipeline does LLM classification against named projects. With zero projects it has nothing to link to (a cold-start problem).
→ For the MVP, a project is a **named entity the user confirms**. The user seeds 5–10 projects by hand (name plus one line). The AI links atoms to them. An atom of type `project` produces a *"Create project?"* suggestion, which never happens automatically (this matches Fixture 6). Real embedding-based clustering waits until after the MVP.

**A4. There is no way to delete a capture.**
"Immutable" should mean the AI can't rewrite a capture. It should not mean the user can't remove one. Sooner or later someone pastes a password or something private.
→ Captures are *append-only and non-editable*, and the user can delete one (tombstone first, then a real purge that cascades to the derived rows).

### B. Scope and sequencing

**B1. The MVP is defined three different ways.** The README's MVP (8 features) spans Roadmap Phases 0–5. The TASKS focus is Phases 0–1. The roadmap puts dogfooding in Phase 6. → This document replaces all three definitions with one.

**B2. Dogfooding starts far too late.** Resurfacing is worthless without months of real captures, and A1 already guarantees old captures can be reprocessed. Every week spent building the AI before capture is usable is a week of lost corpus.
→ **Proposed decision D-016:** dogfooding starts the day Milestone 1 works (capture plus search). Every later milestone ships into an app already in daily use.

**B3. Backup and export come last (Phase 6 / §14).** The second success criterion is *"the user trusts that thoughts will not disappear."* That trust has to exist on day one. → Move automatic DB backup and Markdown/JSON export into M1. Both are small.

**B4. Thread synthesis in the MVP brings versioning and invalidation work** (regenerating "after relevant changes"). → For the MVP, synthesis is an **on-demand "Summarize" button** on a project page, stored with provenance and marked stale when newer captures exist. No automatic regeneration.

**B5. The global hotkey is parked, but capture friction is success criterion #1.** It costs about 20 lines with `tauri-plugin-global-shortcut`. → Make it an M1 stretch item.

### C. Data model and pipeline details

| # | Issue | Fix |
|---|---|---|
| C1 | `Capture` has an `updated_at` field, but captures are immutable | Drop it. Add `deleted_at`. Enforce immutability with a SQLite `BEFORE UPDATE` trigger, not just a policy. |
| C2 | Polymorphic `relationships` table (no FKs, 11 relationship types) is premature | MVP uses one `atom_projects` link table with real FKs. Atom↔atom "similar" is computed from embeddings, not stored as relationship rows. Add typed relationships (`revises`, …) once usage asks for them. |
| C3 | The `Action` entity overlaps with `task` atoms | Cut it from the MVP. |
| C4 | The name "project" means both an atom type and an entity | Keep both, but document that a `project` atom is a *candidate* for a Project entity. |
| C5 | `atoms.processor_version` is a free string, while `ProcessorRun` exists separately | Use `atoms.run_id → processor_runs.id`. |
| C6 | `sourceSpan {start,end}`: LLMs are unreliable at character offsets | Ask the model for a verbatim `quote` and compute offsets locally. If the quote isn't found, store no span. |
| C7 | `sqlite-vec` tables have fixed dimensions, so changing embedding models is painful. The data is too small to need an index. | **Proposed decision D-015 (amends D-008):** store vectors as BLOBs keyed by `(object, model)` and do brute-force cosine in Rust (a few thousand vectors takes milliseconds). Adopt `sqlite-vec` when a measurement shows it's needed. |
| C8 | `ThoughtProcessor` bundles embedding with analysis, but the two often come from different providers (and local embeddings are cheap and private) | Split it into an `Analyzer` trait and an `Embedder` trait. |
| C9 | Semantic search needs a query embedding, so search goes blind when offline or when remote processing is off | Hybrid search falls back to FTS-only, and the UI says so. |
| C10 | IDs and timestamp formats aren't specified | UUIDv7 (sortable) IDs and ISO-8601 UTC text timestamps. |
| C11 | Recurrence ("you keep coming back to this") is undefined, and all-pairs similarity on demand doesn't scale | When an atom is embedded, compare it once against the existing atoms and store its top-k neighbors in `atom_neighbors`. Recurrence = the number of neighbors above threshold τ that come from *other* captures. |

### D. Repo hygiene

- The README repo map leaves out `docs/EXAMPLES.md` (fixed in this commit).
- There is no `.gitignore`, `LICENSE`, CI, or `CLAUDE.md`. A short `CLAUDE.md` pointing at these docs and the invariants would help agent-driven development.
- Fixtures live in prose. → Mirror them as `fixtures/*.json` (input, prior history, expected structural properties) so a test runner and an eval script can use them.
- ~~The first LLM provider is undecided.~~ Resolved: **OpenRouter** for both analysis and embeddings (D-020). Default model IDs are still to be picked. They are settings, so this can be iterated on freely.

---

# Part 2 — MVP Plan

## MVP definition

> A desktop app I use every day to dump thoughts. It never loses them, splits them into typed atoms, links them to my projects, finds them by meaning, and brings back forgotten ones with a reason why.

The MVP is done when **M1–M4 ship and 3–4 weeks of dogfooding** meet the success criteria at the end of this document.

## Architecture (MVP)

```text
React + TS (Vite)  ── typed Tauri commands (tauri-specta) ──►  Rust core
                                                              ├─ db/        rusqlite (bundled, FTS5), WAL, migrations
                                                              ├─ capture    append-only writes, export, backup
                                                              ├─ jobs       single tokio worker, backoff, resume on start
                                                              ├─ processor/ Analyzer + Embedder traits
                                                              │               ├─ mock (deterministic, tests/CI)
                                                              │               └─ openrouter (reqwest; chat/completions + embeddings)
                                                              ├─ search     FTS5 bm25 ⊕ cosine, reciprocal-rank fusion
                                                              ├─ resurface  deterministic scoring + explanations
                                                              └─ secrets    keyring (OS keychain)
prompts/     versioned prompt files (analyze_capture.v1.md, link_projects.v1.md)
fixtures/    golden cases as JSON (from docs/EXAMPLES.md)
scripts/     eval runner: real provider × fixtures → structural assertions report
```

Suggested crates: `tauri` 2, `rusqlite` (`bundled`), `rusqlite_migration`, `tokio`, `reqwest`, `serde`/`serde_json`, `schemars`, `uuid` (v7), `keyring`, `tauri-specta`, `tauri-plugin-global-shortcut`.

## MVP schema

This replaces the illustrative schema in `DATA_MODEL.md` for MVP purposes.

```sql
-- SOURCE DATA ----------------------------------------------------------
CREATE TABLE captures (
  id           TEXT PRIMARY KEY,             -- UUIDv7
  text         TEXT NOT NULL,
  source       TEXT NOT NULL DEFAULT 'desktop',
  captured_at  TEXT NOT NULL,                -- ISO-8601 UTC
  deleted_at   TEXT                          -- user tombstone; only mutable column
);
CREATE TRIGGER captures_immutable
BEFORE UPDATE OF text, source, captured_at ON captures
BEGIN SELECT RAISE(ABORT, 'captures are immutable'); END;

CREATE VIRTUAL TABLE captures_fts USING fts5(text, content='captures', content_rowid='rowid');
-- + AFTER INSERT / AFTER DELETE triggers to keep captures_fts in sync

CREATE TABLE projects (
  id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, description TEXT,
  momentum TEXT NOT NULL DEFAULT 'exploring',  -- user-set only in MVP
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);

CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

-- PROVENANCE / QUEUE ---------------------------------------------------
CREATE TABLE processor_runs (
  id TEXT PRIMARY KEY, capture_id TEXT REFERENCES captures(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,                        -- analyze | embed | link | synthesize
  provider TEXT NOT NULL, model TEXT NOT NULL,
  prompt_version TEXT, schema_version TEXT, app_version TEXT NOT NULL,
  status TEXT NOT NULL, error TEXT, started_at TEXT NOT NULL, finished_at TEXT
);

CREATE TABLE processing_jobs (
  id TEXT PRIMARY KEY, capture_id TEXT NOT NULL REFERENCES captures(id) ON DELETE CASCADE,
  job_type TEXT NOT NULL,                    -- analyze_capture | embed | link_projects
  status TEXT NOT NULL,                      -- queued | running | done | failed
  attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
  run_after TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);

-- DERIVED DATA (+ user corrections, which are source) -------------------
CREATE TABLE atoms (
  id TEXT PRIMARY KEY, capture_id TEXT NOT NULL REFERENCES captures(id) ON DELETE CASCADE,
  run_id TEXT REFERENCES processor_runs(id),  -- NULL when origin = 'user'
  text TEXT NOT NULL, type TEXT NOT NULL, confidence REAL,
  quote TEXT, span_start INTEGER, span_end INTEGER,
  origin TEXT NOT NULL DEFAULT 'ai',          -- ai | user  (user = created or edited by user)
  status TEXT NOT NULL DEFAULT 'active',      -- active | rejected | superseded
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE atoms_fts USING fts5(text, content='atoms', content_rowid='rowid');

CREATE TABLE atom_projects (
  atom_id TEXT NOT NULL REFERENCES atoms(id) ON DELETE CASCADE,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  status TEXT NOT NULL,                      -- suggested | confirmed | rejected
  origin TEXT NOT NULL,                      -- ai | user
  confidence REAL, reason TEXT, run_id TEXT REFERENCES processor_runs(id),
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  PRIMARY KEY (atom_id, project_id)
);

CREATE TABLE embeddings (
  object_type TEXT NOT NULL, object_id TEXT NOT NULL,   -- capture | atom | project
  model TEXT NOT NULL, dims INTEGER NOT NULL, vector BLOB NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (object_type, object_id, model)
);

CREATE TABLE atom_neighbors (                           -- top-k, computed at embed time
  atom_id TEXT NOT NULL, neighbor_id TEXT NOT NULL, similarity REAL NOT NULL,
  model TEXT NOT NULL, PRIMARY KEY (atom_id, neighbor_id, model)
);

CREATE TABLE project_syntheses (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  run_id TEXT REFERENCES processor_runs(id), body_json TEXT NOT NULL,
  based_on_through TEXT NOT NULL,            -- latest captured_at included; stale if newer exists
  created_at TEXT NOT NULL
);

CREATE TABLE resurfacing_events (
  id TEXT PRIMARY KEY, atom_id TEXT NOT NULL REFERENCES atoms(id) ON DELETE CASCADE,
  score REAL NOT NULL, reasons_json TEXT NOT NULL,
  shown_at TEXT NOT NULL,
  response TEXT, responded_at TEXT           -- interesting | not_now | make_active | dismiss
);
```

Reprocessing rule: running `analyze_capture` again sets `status='superseded'` on that capture's `origin='ai'` atoms and inserts the new set. `origin='user'` atoms and `rejected` links are never touched.

## Milestones

Sizes are relative (S ≈ an evening or two, M ≈ a weekend, L ≈ a couple of weekends).

### M1 — Safe dump (dogfooding starts here)

Goal: a trustworthy place to throw thoughts, with no AI at all.

- [ ] **S** Scaffold Tauri 2 + React + TS + Vite. Add `.gitignore`, formatter/linter (`rustfmt`, `clippy`, `eslint`/`prettier` or `biome`), `cargo test` + `vitest`.
- [ ] **S** CI: lint + test + build on push.
- [ ] **S** `db` module: open the DB in the app data dir, WAL on, migration runner, the `captures` table plus the immutability trigger and FTS.
- [ ] **M** Capture screen: multiline input, Ctrl/Cmd+Enter to save, save disabled when empty, input cleared only after the write commits, "Saved ✓" confirmation.
- [ ] **S** History list: exact text (monospace or otherwise visibly "yours"), exact timestamp, newest first, paginated.
- [ ] **S** Lexical search over captures (FTS5, bm25, highlighted snippets).
- [ ] **S** Delete capture (tombstone + purge).
- [ ] **S** Safety: copy the DB to a rolling `backups/` folder on startup (keep N), plus **Export → Markdown** (one file per day) and **Export → JSON**.
- [ ] **S** *(stretch)* Global hotkey that opens or focuses the capture box.
- [ ] Tests: text round-trips byte-for-byte (including unicode and emoji), the trigger rejects UPDATE, the capture survives a restart, migrations run cleanly on an empty DB and on the previous version's DB.

**Exit:** you have used it as your only thought-dump for a week, and exporting then re-reading the export loses nothing.

### M2 — Understand

Goal: every capture becomes typed atoms, and the original stays visibly separate.

- [ ] **M** Job worker: one tokio task that picks up `queued` jobs whose `run_after <= now`, with exponential backoff (max 5 attempts). On startup it resets `running` jobs to `queued`. It emits Tauri events so the UI updates status live (`Saved → Analyzing… → Processed / Needs retry`).
- [ ] **S** `Analyzer` trait and `MockAnalyzer` (deterministic: splits on sentences, types atoms by keyword).
- [ ] **M** First real adapter: `OpenRouterAnalyzer` (D-020).
  - `POST https://openrouter.ai/api/v1/chat/completions` with the prompt from `prompts/analyze_capture.v1.md`.
  - `response_format: { type: "json_schema", json_schema: { name, strict: true, schema } }`, with the schema generated from Rust types via `schemars`.
  - `provider: { require_parameters: true, data_collection: "deny" }` (plus `zdr: true` when the privacy setting is on).
  - serde validation of every response (one repair retry, then `failed`), timeout, 429/5xx backoff.
  - A `processor_runs` row recording the model OpenRouter reports actually serving the request.
- [ ] **S** Settings: OpenRouter API key (keychain), analyzer model id, embedding model id, privacy toggles (`data_collection: deny`, ZDR-only), **remote processing on/off** (off ⇒ jobs stay queued), and a visible "sent to OpenRouter → <model>" indicator.
- [ ] **M** Atom display under each capture: type chip, confidence, "generated" styling. Atom actions: change type, edit text (→ `origin='user'`), reject, add missing atom.
- [ ] **S** "Reprocess" per capture and "Process backlog" for all unprocessed captures (this is how the M1 corpus gets processed).
- [ ] **S** Convert `docs/EXAMPLES.md` into `fixtures/*.json`. Contract tests run on the mock in CI. `scripts/eval` runs the real provider against the fixtures and prints a pass/fail table of the structural assertions (not a CI gate).
- [ ] Add FTS over atoms; search now returns captures and atoms.

**Exit:** the fixture eval passes the structural assertions for 7 of 8 fixtures, and in daily use you correct fewer than ~1 in 5 atoms.

### M3 — Connect

Goal: thoughts stop being islands.

- [ ] **S** `Embedder` trait with mock and `OpenRouterEmbedder` (`POST /api/v1/embeddings`, batched inputs). `embed` job for captures and atoms, keyed by model id. Changing the model in settings queues a re-embed.
- [ ] **S** Brute-force cosine in Rust. Compute `atom_neighbors` top-k at embed time.
- [ ] **M** Hybrid search: FTS5 results ⊕ vector results merged by reciprocal-rank fusion. Falls back to lexical-only, with a UI note, when no query embedding is available.
- [ ] **M** Projects: create, rename, describe, set momentum. Seed your 5–10 real projects. Embed each project's name and description.
- [ ] **M** `link_projects` job: for each new atom, take the top-k candidate projects (vector + lexical) → the LLM classifies `belongs_to` with confidence and reason → rows with `status='suggested'`. The prompt includes previously rejected pairs so it doesn't repeat them.
- [ ] **S** Link chips on atoms: ✓ confirm / ✗ reject / + add project (one click each, no modals).
- [ ] **S** "Create project?" suggestion when a `project` atom has no confident link. Nothing is created without a click.
- [ ] **M** Project page: timeline of linked captures, newest first, raw text as evidence, linked atoms shown inline.
- [ ] **S** "Possibly related" panel on a capture's detail view (from `atom_neighbors`).
- [ ] **S** *(stretch)* "Summarize" button on the project page → `project_syntheses` with source links. Shows "stale" once newer captures exist.

**Exit:** a query using different words from the original finds it, one capture can link to two projects (Fixture 7), and rejected links stay rejected after reprocessing.

### M4 — Resurface

Goal: solve the forgetting problem, deterministically and with an explanation for every pick.

- [ ] **M** Candidate pool: active atoms of types `spark | question | research | problem | someday | project | feature`, excluding any shown in the last 14 days or ever dismissed.
- [ ] **M** Score v0 (weights in `settings`, tuned from feedback, not guessed forever):

  ```text
  score = 1.0 · dormancy        (days since captured or last shown, log-scaled, capped)
        + 1.0 · recurrence      (# atom_neighbors ≥ τ from other captures, log-scaled)
        + 0.5 · active_project  (linked to a project with momentum active|ready)
        + 0.3 · open_question   (type = question | research)
        + 0.4 · U(0,1)          (exploration)
        − 2.0 · not_now_recent  (answered "not now" in last 30 days)
  ```

  Store the individual terms in `reasons_json` and render the largest one as the explanation ("You've mentioned this 4 times", "Untouched for 63 days", …).
- [ ] **S** "Resurface something" button → one card with the atom, its source capture and the explanation, plus **Interesting / Not now / Make active / Dismiss**. "Make active" sets the momentum of the linked project, or offers to create one.
- [ ] **M** Home screen v1: Recently captured (the safety net), Resurface card, Active projects, **You keep coming back to this** (top recurrence atoms that aren't linked to an active project). Leave out "Your brain made a connection" until link quality is proven.
- [ ] **S** Stats view (plain SQL): captures/week, atom correction rate, link confirm/reject ratio, resurfacing response mix.

**Exit:** over two weeks, at least a few resurfaced items get "Interesting" or "Make active", and "Dismiss" doesn't dominate.

### Then: dogfood 3–4 weeks (no new features)

Use the stats view and a `DOGFOOD.md` log. Turn bad classifications into new fixtures, and tune the resurfacing weights and τ. Only then look at the Phase 7 capture surfaces, and pick the one that would have saved the most lost thoughts.

## Explicitly out of the MVP

Action entity · the generic polymorphic relationship graph · `revises`/`contradicts` detection · automatic project clustering · automatic synthesis regeneration · automatic momentum changes · `sqlite-vec` · local LLM (a local *embedder* is fine if it's easy) · everything in the TASKS parking lot except the global hotkey.

## Decisions (recorded in `DECISIONS.md`)

- **D-014 (accepted):** Rust owns the core (DB, jobs, AI adapters, scoring). React is a view layer with generated types.
- **D-015:** Brute-force vectors in SQLite BLOBs before `sqlite-vec`. Amends D-008.
- **D-016:** Dogfooding starts at M1. The AI can process the backlog later.
- **D-017:** Projects are user-confirmed entities. The AI suggests links and new projects but never creates projects on its own.
- **D-018:** Captures can't be edited but can be deleted by the user. Deletion cascades to derived data.
- **D-019:** User corrections are source data (`origin='user'`, `status='rejected'`) and survive reprocessing.
- **D-020 (accepted):** OpenRouter is the first provider for both analysis and embeddings. Model IDs are configuration.

## Risks

| Risk | Mitigation |
|---|---|
| Rust + Tauri friction slows M1 | M1 is mostly CRUD. If it drags past two weekends, revisit D-007/D-014 before going further. |
| Over-atomization (every sentence becomes a task) | Prompt rules from `AI_PIPELINE.md`, fixtures 2/6/8, a cap on atoms per capture, zero atoms allowed. |
| Project sprawl | D-017 (no automatic creation). |
| Link suggestions are noisy and erode trust | Show suggested links visibly as suggestions. Measure the confirm/reject ratio before building "connection" features. |
| Resurfacing is annoying | 14-day suppression, the dismiss penalty, and one card on demand (no notifications in the MVP). |
| Private text sent to a remote provider | Remote processing off switch, a visible indicator, OpenRouter `data_collection: "deny"` / ZDR routing, capture and search keep working offline, and the `Embedder` split makes a local embedder possible. |
| OpenRouter routes a request to an upstream that ignores the schema | `require_parameters: true`, plus local serde validation and a repair retry. The fixture eval is re-run whenever the default model changes. |

## MVP success criteria (evaluate after dogfooding)

1. Capture takes under 3 seconds from intent to saved, and nothing has been lost.
2. At least 80% of atoms need no correction.
3. The link confirm rate is at least 70%.
4. At least one forgotten idea per week is resurfaced and marked Interesting or Make active.
5. You open ThoughtRouter without having to remind yourself to.

## Next five actions

1. Accept, amend or reject D-015 … D-019.
2. Scaffold Tauri 2 + React + TS with CI and the `db` module (`captures` + trigger + FTS).
3. Build the capture screen and history, and start dumping real thoughts that day.
4. Add backup and export, then delete. M1 is done, and so is the week of dogfooding.
5. Convert `EXAMPLES.md` to `fixtures/*.json`, then shortlist 2–3 OpenRouter analyzer models and one embedding model, and compare them with the fixture eval.

---

# Implementation notes (as built)

Where the implementation differs from the plan above, and why:

| Plan | As built | Why |
|---|---|---|
| `processing_jobs.capture_id` | `processing_jobs.target_id` (+ `embed_project` job) | Projects need embedding jobs too. |
| FTS5 external-content tables on `rowid` | FTS5 tables that store their own copy, keyed by id | Implicit rowids can be renumbered by `VACUUM`; the text volume is tiny. |
| "Create project?" held in the link table | Separate `project_suggestions` table | Suggestions aren't links; they need their own accept/dismiss state. |
| `project_syntheses` | + `source_capture_ids` column | Provenance: which captures a synthesis was based on. |
| Schema generated with `schemars` | Hand-written strict JSON schemas in `crates/core/prompts/*.schema.json`, validated locally with serde | Strict structured-output mode rejects several keywords `schemars` emits; a test checks every schema is strict. |
| `tauri-specta` | `ts-rs` generates `src/bindings/*.ts` from the Rust types; commands are wrapped by hand in `src/api.ts` | `tauri-specta` for Tauri 2 is still a release candidate. |
| Linking only runs for new captures | Creating or renaming a project also queues linking for up to 15 related older captures | Without it, anything captured before a project existed would never be linked. |
| Mock provider for tests only | Mock is also selectable in Settings | Lets you try the full pipeline without a key. It is heuristic, so switch back to OpenRouter for real use. |
| (not specified) | Trash is purged after 30 days; a backup is written on every launch (newest 10 kept) | D-018, B3. |

**Placeholders waiting on you:** the analyzer and embedding model ids (empty in Settings until chosen), the similarity threshold (0.75 until tuned for the chosen embedding model), resurfacing weights (the Score v0 values above), the global shortcut (`CommandOrControl+Shift+Space`), the app identifier (`com.sonoransolutions.thoughtrouter`, which also names the data folder), and the license (`UNLICENSED`).
