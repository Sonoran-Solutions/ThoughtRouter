# Task List

This file is the concrete build checklist. Keep `ROADMAP.md` for phases and this file for implementation work.

## Current focus

**The MVP (M1–M4 in [`docs/MVP_PLAN.md`](docs/MVP_PLAN.md)) is implemented. Next: pick default models, then dogfood (§13).**

Still open before/while dogfooding:

- [ ] Pick the default OpenRouter analyzer model (Settings → *Analyzer model id*; compare candidates with the eval runner)
- [ ] Pick the default OpenRouter embedding model (Settings → *Embedding model id*)
- [ ] Tune the recurrence similarity threshold for that embedding model (Settings → *Resurfacing*; default 0.75 is a placeholder)
- [ ] Run `cargo run -p thoughtrouter-core --example eval -- --analyzer …` against the chosen model and record results
- [ ] Decide on D-015 … D-019 (implemented as proposed) and mark them accepted/rejected
- [ ] Choose a license (currently `UNLICENSED`) and an application versioning strategy

Do not start GitHub integration, mobile capture, or agent orchestration until dogfooding says the core loop works.

---

## 0. Repository / project setup

- [x] Initialize Tauri + React + TypeScript application
- [x] Choose package manager and commit lockfile
- [x] Add formatter/linter configuration
- [x] Add unit-test runner
- [x] Add `.gitignore`
- [x] Add basic CI for lint + test + build
- [ ] Add application versioning strategy (versions are 0.1.0 in Cargo.toml / package.json / tauri.conf.json)
- [x] Add local development instructions to README

### Definition of done

A fresh clone can be installed, tested, and launched from documented commands.

---

## 1. SQLite foundation

- [x] Add SQLite access layer
- [x] Add migration runner
- [x] Create `captures` table
- [x] Create `processing_jobs` table
- [x] Add repository/data-access abstraction
- [x] Enable safe transaction handling
- [x] Decide whether WAL mode is useful and document the decision
- [x] Add tests for migrations
- [x] Add tests for capture persistence

### Definition of done

A capture survives application restart and database migration tests run deterministically.

---

## 2. Raw capture flow

- [x] Build primary `What's on your mind?` screen
- [x] Support multiline freeform text
- [x] Add keyboard-first save action
- [x] Disable empty submissions
- [x] Save raw text before any AI work begins
- [x] Show immediate saved confirmation
- [x] Clear input only after durable save succeeds
- [x] Show recent capture history
- [x] Show exact timestamp
- [x] Preserve exact submitted text
- [x] Add processing status indicator

### Important invariant test

- [x] Verify AI/processing code cannot mutate `captures.text`

### Definition of done

You can launch the app, dump a messy paragraph, hit save, close the app, reopen it, and see the exact original text.

---

## 3. Processing job framework

- [x] Define local job statuses
- [x] Queue `analyze_capture` job after raw capture save
- [x] Add retry count
- [x] Store last error
- [x] Resume unfinished jobs on app startup
- [x] Ensure failed processing does not mark capture as failed
- [x] Add manual retry action
- [x] Add deterministic tests using fake jobs

### Definition of done

A simulated processor failure leaves the raw capture intact and retryable.

---

## 4. ThoughtProcessor abstraction

- [x] Define provider-independent `Analyzer` + `Embedder` traits (Rust)
- [x] Define `AnalyzeCaptureInput`
- [x] Define structured `CaptureAnalysis` result
- [x] Define version metadata contract
- [x] Implement `MockThoughtProcessor`
- [x] Create fixture-based tests
- [x] Keep provider credentials out of source control
- [x] Use OS secure credential storage where practical

### Definition of done

The app can process captures with a deterministic mock implementation without knowing anything about a real provider.

---

## 5. Atom model

- [x] Create `atoms` table
- [x] Implement initial atom type enum
- [x] Store confidence
- [x] Store processor/prompt/schema version
- [x] Link every atom to its source capture
- [x] Build atom display under a raw capture
- [x] Visually label output as generated/derived
- [x] Add tests for one capture producing multiple atoms

Initial atom types:

- [x] Spark
- [x] Project
- [x] Feature
- [x] Task
- [x] Question
- [x] Research
- [x] Reference
- [x] Decision
- [x] Problem
- [x] Someday

### Definition of done

A messy multi-topic capture can produce multiple structured atoms while the untouched original remains obvious in the UI.

---

## 6. First real AI adapter

- [x] Select first provider for iteration — OpenRouter (D-020)
- [ ] Pick default analyzer model (configurable model id)
- [x] Implement adapter behind `ThoughtProcessor`
- [x] Use schema-constrained structured output
- [x] Add timeout handling
- [x] Add retry/backoff behavior where appropriate
- [x] Store processing metadata
- [x] Make remote processing visibly configurable
- [x] Add clear failure state
- [ ] Test against golden examples

### Definition of done

The processor reliably separates representative brain dumps into useful atoms without fabricating commitments.

---

## 7. Search foundation

- [x] Add SQLite FTS lexical search
- [x] Search raw captures
- [x] Search atoms
- [x] Build minimal search UI
- [x] Rank exact/lexical matches sensibly
- [x] Add tests for known search fixtures

### Definition of done

You can find an old thought from remembered words even before vector search exists.

---

## 8. Embeddings + semantic search

- [x] Choose initial embedding provider — OpenRouter `/api/v1/embeddings` (D-020)
- [ ] Pick default embedding model
- [x] Create embedding storage
- [x] Record embedding model/version
- [x] ~~Add vector extension (`sqlite-vec` or equivalent)~~ Brute-force cosine over SQLite BLOBs instead (D-015)
- [x] Embed raw captures
- [x] Embed atoms
- [x] Implement semantic nearest-neighbor retrieval
- [x] Blend semantic and lexical results
- [x] Add re-embedding command/job for model changes
- [x] Add natural-language search UX

### Definition of done

A query can recover an older thought even when it does not use the same words.

---

## 9. Project clusters

- [x] Create `projects` table
- [x] Implement momentum enum
- [x] Create project manually
- [x] Create `relationships` table
- [x] Link atom to multiple projects
- [x] Allow unassigned atoms
- [x] Build project page
- [x] Show related captures/atoms chronologically
- [x] Add AI project-link suggestions
- [x] Add confirm/reject controls
- [x] Persist explicit corrections

### Definition of done

One thought can belong to several projects, and rejecting a bad AI link prevents the UI from pretending it is true.

---

## 10. Thought Threads

- [x] Build chronological thread view
- [x] Add generated current-understanding summary
- [x] Add open questions section
- [x] Add possible next actions section
- [x] Add changed-assumptions section
- [x] Link generated synthesis back to source captures
- [x] Version thread synthesis
- [x] ~~Regenerate synthesis after relevant changes~~ On-demand *Summarize* + "stale" flag instead (MVP_PLAN B4)

### Definition of done

Opening a project after time away quickly reconstructs what you were thinking and why.

---

## 11. Resurfacing v1

- [x] Create `resurfacing_events` table
- [x] Implement `Resurface something` button
- [x] Add old/unseen thought candidate selection
- [x] Add repeat-mention signal
- [x] Add recently-shown penalty
- [x] Add active-project relevance signal
- [x] Add simple random exploration factor
- [x] Record why each item was surfaced
- [x] Add feedback actions:
  - [x] Interesting
  - [x] Not now
  - [x] Make active
  - [x] Dismiss
- [x] Prevent immediate repeated resurfacing

### Definition of done

The app can bring back a forgotten thought and explain why it chose it.

---

## 12. Home screen v1

- [x] Add `Right now`
- [x] Add `You keep coming back to this`
- [x] Add `From the vault`
- [x] Add `Recently captured`
- [ ] Add `Your brain made a connection` only after relationship quality is good enough
- [x] Ensure the screen does not become a guilt-inducing backlog

### Definition of done

Opening ThoughtRouter gives useful context rather than a giant todo list.

---

## 13. Dogfood / evaluation

- [ ] Use ThoughtRouter as the default idea-dump location for several weeks
- [ ] Record classification failures
- [ ] Record bad relationship suggestions
- [ ] Record missed resurfacing opportunities
- [ ] Record annoying resurfacing
- [ ] Record moments where capture was too slow
- [ ] Convert representative real captures into test fixtures
- [ ] Review atom vocabulary based on actual usage
- [ ] Review momentum states based on actual usage
- [ ] Tune resurfacing based on feedback history

### Definition of done

You reach for ThoughtRouter automatically and it has recovered at least several ideas you otherwise would have forgotten.

---

## 14. Data safety / portability

- [x] Add database backup
- [x] Add JSON export
- [x] Add raw Markdown/text export
- [x] Test restore/re-import
- [x] Document database location
- [x] Document provider data flow
- [x] Ensure deleting derived AI state never deletes raw captures

---

# Parking lot — do not build yet

These ideas are intentionally captured so they do not need to live in your head, but they are **not current work**.

- [ ] Android Quick Settings capture tile
- [ ] Android share target
- [ ] Android widget
- [ ] Voice capture
- [x] Global desktop capture shortcut (pulled into M1 as a stretch item)
- [ ] Tray icon
- [ ] CLI capture command
- [ ] Browser extension
- [ ] GitHub integration
- [ ] Convert confirmed task to GitHub issue
- [ ] ChatGPT conversation ingestion
- [ ] Agent/research handoff
- [ ] Automatic agent prompt construction from thought context
- [ ] Local LLM processing
- [ ] Local embeddings
- [ ] Idea-collision engine
- [ ] Contradiction detection
- [ ] Recurring-idea promotion
- [ ] Multi-device sync
- [ ] Public release/productization

# The next five things to do

If you open this repo later and have forgotten where to start, do these in order:

1. [x] Scaffold Tauri + React + TypeScript.
2. [x] Add SQLite + migrations with a `captures` table.
3. [x] Build the one-screen raw brain-dump capture flow.
4. [x] Add capture history and prove text survives restart exactly unchanged.
5. [x] Add a mock processor and atom extraction plumbing **before** connecting a real model.

Now: add an OpenRouter key, choose models (see *Current focus*), and start dogfooding (§13).
Do not jump straight to the cool integrations.
