# Task List

This file is the concrete build checklist. Keep `ROADMAP.md` for phases and this file for implementation work.

## Current focus

**Milestone: Phase 0 + Phase 1 — local app foundation and raw capture.**

Do not start semantic linking, GitHub integration, mobile capture, or agent orchestration until the raw-capture loop works reliably.

---

## 0. Repository / project setup

- [ ] Initialize Tauri + React + TypeScript application
- [ ] Choose package manager and commit lockfile
- [ ] Add formatter/linter configuration
- [ ] Add unit-test runner
- [ ] Add `.gitignore`
- [ ] Add basic CI for lint + test + build
- [ ] Add application versioning strategy
- [ ] Add local development instructions to README

### Definition of done

A fresh clone can be installed, tested, and launched from documented commands.

---

## 1. SQLite foundation

- [ ] Add SQLite access layer
- [ ] Add migration runner
- [ ] Create `captures` table
- [ ] Create `processing_jobs` table
- [ ] Add repository/data-access abstraction
- [ ] Enable safe transaction handling
- [ ] Decide whether WAL mode is useful and document the decision
- [ ] Add tests for migrations
- [ ] Add tests for capture persistence

### Definition of done

A capture survives application restart and database migration tests run deterministically.

---

## 2. Raw capture flow

- [ ] Build primary `What's on your mind?` screen
- [ ] Support multiline freeform text
- [ ] Add keyboard-first save action
- [ ] Disable empty submissions
- [ ] Save raw text before any AI work begins
- [ ] Show immediate saved confirmation
- [ ] Clear input only after durable save succeeds
- [ ] Show recent capture history
- [ ] Show exact timestamp
- [ ] Preserve exact submitted text
- [ ] Add processing status indicator

### Important invariant test

- [ ] Verify AI/processing code cannot mutate `captures.text`

### Definition of done

You can launch the app, dump a messy paragraph, hit save, close the app, reopen it, and see the exact original text.

---

## 3. Processing job framework

- [ ] Define local job statuses
- [ ] Queue `analyze_capture` job after raw capture save
- [ ] Add retry count
- [ ] Store last error
- [ ] Resume unfinished jobs on app startup
- [ ] Ensure failed processing does not mark capture as failed
- [ ] Add manual retry action
- [ ] Add deterministic tests using fake jobs

### Definition of done

A simulated processor failure leaves the raw capture intact and retryable.

---

## 4. ThoughtProcessor abstraction

- [ ] Define provider-independent `ThoughtProcessor` interface
- [ ] Define `AnalyzeCaptureInput`
- [ ] Define structured `CaptureAnalysis` result
- [ ] Define version metadata contract
- [ ] Implement `MockThoughtProcessor`
- [ ] Create fixture-based tests
- [ ] Keep provider credentials out of source control
- [ ] Use OS secure credential storage where practical

### Definition of done

The app can process captures with a deterministic mock implementation without knowing anything about a real provider.

---

## 5. Atom model

- [ ] Create `atoms` table
- [ ] Implement initial atom type enum
- [ ] Store confidence
- [ ] Store processor/prompt/schema version
- [ ] Link every atom to its source capture
- [ ] Build atom display under a raw capture
- [ ] Visually label output as generated/derived
- [ ] Add tests for one capture producing multiple atoms

Initial atom types:

- [ ] Spark
- [ ] Project
- [ ] Feature
- [ ] Task
- [ ] Question
- [ ] Research
- [ ] Reference
- [ ] Decision
- [ ] Problem
- [ ] Someday

### Definition of done

A messy multi-topic capture can produce multiple structured atoms while the untouched original remains obvious in the UI.

---

## 6. First real AI adapter

- [ ] Select first provider/model for iteration
- [ ] Implement adapter behind `ThoughtProcessor`
- [ ] Use schema-constrained structured output
- [ ] Add timeout handling
- [ ] Add retry/backoff behavior where appropriate
- [ ] Store processing metadata
- [ ] Make remote processing visibly configurable
- [ ] Add clear failure state
- [ ] Test against golden examples

### Definition of done

The processor reliably separates representative brain dumps into useful atoms without fabricating commitments.

---

## 7. Search foundation

- [ ] Add SQLite FTS lexical search
- [ ] Search raw captures
- [ ] Search atoms
- [ ] Build minimal search UI
- [ ] Rank exact/lexical matches sensibly
- [ ] Add tests for known search fixtures

### Definition of done

You can find an old thought from remembered words even before vector search exists.

---

## 8. Embeddings + semantic search

- [ ] Choose initial embedding provider/model
- [ ] Create embedding storage
- [ ] Record embedding model/version
- [ ] Add vector extension (`sqlite-vec` or equivalent)
- [ ] Embed raw captures
- [ ] Embed atoms
- [ ] Implement semantic nearest-neighbor retrieval
- [ ] Blend semantic and lexical results
- [ ] Add re-embedding command/job for model changes
- [ ] Add natural-language search UX

### Definition of done

A query can recover an older thought even when it does not use the same words.

---

## 9. Project clusters

- [ ] Create `projects` table
- [ ] Implement momentum enum
- [ ] Create project manually
- [ ] Create `relationships` table
- [ ] Link atom to multiple projects
- [ ] Allow unassigned atoms
- [ ] Build project page
- [ ] Show related captures/atoms chronologically
- [ ] Add AI project-link suggestions
- [ ] Add confirm/reject controls
- [ ] Persist explicit corrections

### Definition of done

One thought can belong to several projects, and rejecting a bad AI link prevents the UI from pretending it is true.

---

## 10. Thought Threads

- [ ] Build chronological thread view
- [ ] Add generated current-understanding summary
- [ ] Add open questions section
- [ ] Add possible next actions section
- [ ] Add changed-assumptions section
- [ ] Link generated synthesis back to source captures
- [ ] Version thread synthesis
- [ ] Regenerate synthesis after relevant changes

### Definition of done

Opening a project after time away quickly reconstructs what you were thinking and why.

---

## 11. Resurfacing v1

- [ ] Create `resurfacing_events` table
- [ ] Implement `Resurface something` button
- [ ] Add old/unseen thought candidate selection
- [ ] Add repeat-mention signal
- [ ] Add recently-shown penalty
- [ ] Add active-project relevance signal
- [ ] Add simple random exploration factor
- [ ] Record why each item was surfaced
- [ ] Add feedback actions:
  - [ ] Interesting
  - [ ] Not now
  - [ ] Make active
  - [ ] Dismiss
- [ ] Prevent immediate repeated resurfacing

### Definition of done

The app can bring back a forgotten thought and explain why it chose it.

---

## 12. Home screen v1

- [ ] Add `Right now`
- [ ] Add `You keep coming back to this`
- [ ] Add `From the vault`
- [ ] Add `Recently captured`
- [ ] Add `Your brain made a connection` only after relationship quality is good enough
- [ ] Ensure the screen does not become a guilt-inducing backlog

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

- [ ] Add database backup
- [ ] Add JSON export
- [ ] Add raw Markdown/text export
- [ ] Test restore/re-import
- [ ] Document database location
- [ ] Document provider data flow
- [ ] Ensure deleting derived AI state never deletes raw captures

---

# Parking lot — do not build yet

These ideas are intentionally captured so they do not need to live in your head, but they are **not current work**.

- [ ] Android Quick Settings capture tile
- [ ] Android share target
- [ ] Android widget
- [ ] Voice capture
- [ ] Global desktop capture shortcut
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

1. [ ] Scaffold Tauri + React + TypeScript.
2. [ ] Add SQLite + migrations with a `captures` table.
3. [ ] Build the one-screen raw brain-dump capture flow.
4. [ ] Add capture history and prove text survives restart exactly unchanged.
5. [ ] Add a mock `ThoughtProcessor` and atom extraction plumbing **before** connecting a real model.

After that, reassess. Do not jump straight to the cool integrations.
