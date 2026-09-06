# Architecture

## Goals

The initial architecture should optimize for:

- local-first operation;
- low capture latency;
- model/provider independence;
- reversible AI interpretation;
- semantic retrieval;
- easy reprocessing of old thoughts;
- eventual support for multiple capture surfaces without rewriting the core.

It should **not** optimize for multi-user SaaS scale yet.

## Proposed high-level architecture

```text
┌──────────────────────────────┐
│        Capture Surfaces      │
│                              │
│  Desktop UI                  │
│  CLI              (later)    │
│  Android          (later)    │
│  Share Target     (later)    │
│  Browser          (later)    │
└──────────────┬───────────────┘
               │
               v
┌──────────────────────────────┐
│      Application Core        │
│                              │
│  CaptureService              │
│  ThoughtProcessor            │
│  RelationshipService         │
│  SearchService               │
│  ResurfacingService          │
│  ProjectService              │
└──────────────┬───────────────┘
               │
        ┌──────┴───────┐
        v              v
┌──────────────┐  ┌──────────────┐
│ SQLite       │  │ AI Provider  │
│              │  │ Abstraction  │
│ raw data     │  │              │
│ derived data │  │ OpenAI       │
│ relationships│  │ Local LLM    │
│ embeddings   │  │ Future       │
└──────────────┘  └──────────────┘
```

## Suggested stack

### Desktop shell

**Tauri**

Reasons:

- lightweight compared with Electron;
- good fit for a local-first desktop utility;
- Rust backend gives direct access to SQLite/filesystem/system integration;
- web frontend keeps UI development fast;
- possible path toward mobile later without making mobile an MVP dependency.

This is a recommendation, not a sacred decision. If Tauri becomes friction, replace it rather than protecting the stack.

### Frontend

**React + TypeScript**

Primary jobs:

- capture UI;
- thought history;
- thought/thread/project views;
- semantic search;
- correction/confirmation UI for AI-derived structure;
- resurfacing cards.

### Persistence

**SQLite**

Why:

- local-first;
- zero infrastructure;
- portable single-user database;
- robust migrations;
- easy backup/export;
- sufficient for the expected data volume.

Use WAL mode if appropriate for background processing and foreground reads.

### Vector search

Start with **sqlite-vec** or an equivalent SQLite-native vector extension.

Do not introduce a separate vector database unless local usage proves SQLite-based retrieval insufficient.

### AI provider layer

The application core should depend on an interface, not on one model/vendor.

Illustrative TypeScript shape:

```ts
interface ThoughtProcessor {
  analyzeCapture(input: AnalyzeCaptureInput): Promise<ThoughtAnalysis>;
  embedText(text: string): Promise<number[]>;
  synthesizeThread(input: ThreadSynthesisInput): Promise<ThreadSynthesis>;
}
```

Potential implementations:

```text
OpenAIThoughtProcessor
LocalLLMThoughtProcessor
MockThoughtProcessor
```

The mock implementation is important for deterministic tests.

## Processing model

Capture should be a two-stage operation.

### Stage 1: durable write

The raw capture is stored immediately.

```text
user presses Save
        ↓
write raw Capture
        ↓
confirm saved
```

No LLM call should be required for the thought to be safely captured.

### Stage 2: asynchronous derived processing

After the raw capture exists:

```text
Capture
  ↓
Atom extraction
  ↓
Embedding generation
  ↓
Candidate relationship retrieval
  ↓
Relationship classification
  ↓
Project/theme suggestions
  ↓
Resurfacing metadata update
```

If processing fails, the capture remains intact and can be retried.

## Important boundary: source vs derived data

Source data:

- raw capture text;
- capture timestamp;
- explicitly user-entered metadata;
- explicit user corrections/decisions.

Derived data:

- atoms;
- classifications;
- generated summaries;
- embeddings;
- semantic relationships;
- confidence values;
- resurfacing scores;
- AI-suggested project membership.

Derived data should be safe to delete and regenerate.

## Background job model

MVP does not need a distributed queue.

A local jobs table is enough:

```text
processing_jobs
- id
- capture_id
- job_type
- status
- attempts
- last_error
- created_at
- updated_at
```

The desktop process can resume unfinished work on startup.

Potential job types:

- analyze_capture
- generate_embedding
- link_relationships
- synthesize_thread
- recompute_resurfacing

## Search architecture

Search should combine at least two signals:

1. lexical/full-text match;
2. vector similarity.

Later, ranking may also incorporate:

- project membership;
- recency;
- relationship strength;
- user-confirmed links;
- recurrence frequency.

Do not make pure embedding similarity the only retrieval path.

## Resurfacing architecture

Resurfacing should eventually be a ranking problem over candidate thoughts.

Illustrative factors:

```text
score =
  semantic_relevance
+ recurrence_signal
+ dormancy_signal
+ active_project_relevance
+ unresolved_question_signal
+ random_exploration_weight
- recently_shown_penalty
- dismissed_penalty
```

Exact weights should be learned from usage, not invented up front.

The system should record why something was resurfaced so the UI can explain it.

## Data portability

Because this is personal software, portability is mandatory.

Eventually support:

- SQLite database backup;
- JSON export of captures and relationships;
- Markdown/text export of raw thoughts;
- re-import without requiring the original AI provider.

Raw user data should never be trapped behind model-generated state.

## Privacy model

Default assumption: thoughts may contain private project ideas or personal notes.

Principles:

- local storage by default;
- no telemetry required for core use;
- explicit indication when text is being sent to a remote model;
- ability to use a local model later;
- provider credentials stored using OS-appropriate secure storage, not plaintext config files.

## Configuration

Initial config can remain simple:

```text
AI provider
Model
Embedding provider/model
Local-only mode
Automatic processing on/off
Resurfacing preferences
```

Avoid exposing model-tuning knobs unless they solve an observed problem.

## Testing strategy

Prioritize deterministic tests around the non-LLM core.

### Unit tests

- capture persistence;
- relationship CRUD;
- project clustering behavior;
- job retries;
- resurfacing ranking inputs;
- data migrations.

### Contract tests

Test `ThoughtProcessor` against saved fixtures.

### Golden fixtures

Maintain a small set of intentionally messy example captures and expected structural properties, e.g.:

- one capture containing multiple ideas;
- a task plus unrelated spark;
- a decision revising an older decision;
- a recurring project idea;
- a thought relevant to multiple projects.

Tests should avoid requiring an exact LLM wording match. Assert schemas, relationships, types, and invariants instead.

## Architecture rule of thumb

If a new subsystem cannot answer the question:

> How does this make it easier to capture, understand, connect, or resurface my thoughts?

it probably does not belong in the current build.
