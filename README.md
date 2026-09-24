# ThoughtRouter

> A local-first system for capturing messy thoughts now and organizing them later.

ThoughtRouter is a personal tool for a brain that generates ideas faster than it reliably records or organizes them.

The core rule is simple:

**Capture first. Structure later.**

ThoughtRouter should never require the user to decide which project, folder, tag, priority, or due date a thought belongs to before saving it. The raw thought is preserved exactly as entered. AI and semantic tooling can then break it into useful pieces, connect it to existing ideas, and resurface it when it becomes relevant again.

This repository is being built for one user first. If it eventually becomes useful enough to ship to other people, cool. Productization is not an MVP requirement.

## The problem

Traditional note and task apps usually assume the user already knows what a thought *is* before capturing it:

- Which project does this belong to?
- Is this a task or an idea?
- What tag should it have?
- Is it important?
- When is it due?

That ceremony is exactly where thoughts disappear.

ThoughtRouter instead accepts input like:

> Need to figure out the SSD issue on the handheld. Also I want one Linux control center that replaces the proprietary Windows app. Cooler support is probably the ugly part. Could an agent research the protocol? Oh, and Save Doctor could maybe detect save corruption automatically.

The user hits **Save**. The system handles the rest.

## Core loop

```text
CAPTURE -> UNDERSTAND -> CONNECT -> RESURFACE
```

### Capture

One low-friction input for anything currently bouncing around in the user's head.

### Understand

The original text is immutable. A processing layer extracts smaller structured `Atoms` such as ideas, tasks, questions, problems, decisions, research leads, and references.

### Connect

Atoms and captures are linked semantically to projects, themes, and each other. A thought may belong to multiple projects or none at all.

### Resurface

ThoughtRouter proactively brings back useful forgotten context instead of becoming a graveyard of notes.

Examples:

- "You keep coming back to this idea."
- "This looks related to something you wrote three months ago."
- "These two projects appear to share the same technical problem."
- "You mentioned this five times. Promote it to an active project?"
- "Here's one forgotten thought that may be worth revisiting."

## Product principles

1. **Zero ceremony at capture time.**
2. **Never destroy or rewrite the raw thought.**
3. **Projects are clusters, not folders.**
4. **Dormant is not failed.**
5. **Resurfacing matters as much as capture.**
6. **AI proposes structure; the user remains authoritative.**
7. **Local-first by default.**
8. **Optimize for one brain before optimizing for a market.**
9. **Avoid productivity theater.** A feature is only useful if it reduces cognitive load.
10. **Scope creep is a bug until the core loop proves itself.**

See [`docs/PRODUCT.md`](docs/PRODUCT.md) for the fuller product model.

## MVP

The first useful version needs only:

- Brain-dump text input
- Immutable raw capture history
- LLM atomization
- Automatic semantic project/theme linking
- Project/thread pages
- Semantic search
- A "Resurface something" action
- Local persistence

Explicitly **not MVP**:

- Calendar integration
- Kanban boards
- Team collaboration
- Billing/accounts
- GitHub automation
- Mobile quick tiles
- Browser extensions
- Voice capture
- Full agent orchestration
- SaaS infrastructure

Those can come later if the core system becomes something worth opening every day.

The milestone breakdown (M1 Safe dump → M2 Understand → M3 Connect → M4 Resurface) lives in [`docs/MVP_PLAN.md`](docs/MVP_PLAN.md).

## Stack

As built for the MVP (still not an irreversible commitment):

```text
Desktop UI       React + TypeScript
Desktop shell    Tauri (Rust core owns data, jobs, AI calls)
Local database   SQLite
Vector search    Brute-force cosine over vectors stored in SQLite (D-015); sqlite-vec if ever needed
AI               Provider-agnostic Analyzer/Embedder traits; OpenRouter first
Primary mode     Local-first
Later clients    Android / browser capture / CLI / share targets
```

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Repository map

```text
README.md                  Project overview (this file)
TASKS.md                   Concrete implementation checklist
ROADMAP.md                 Phased build plan
DECISIONS.md               Important product/technical decisions
CLAUDE.md                  Orientation + invariants for coding agents

crates/core/               Rust application core (D-014): DB, jobs, AI, search, resurfacing
  migrations/              SQLite schema (append-only migrations)
  prompts/                 Versioned prompts + strict JSON schemas sent to the model
  examples/eval.rs         Golden-fixture evaluation runner
src-tauri/                 Tauri desktop shell (commands, keychain, hotkey, worker startup)
src/                       React + TypeScript UI
  bindings/                TypeScript types generated from Rust (ts-rs) — do not edit
fixtures/                  Golden fixtures (JSON) from docs/EXAMPLES.md

docs/
  PRODUCT.md               Product model and principles
  UX.md                    Intended interaction model
  ARCHITECTURE.md          Technical architecture
  DATA_MODEL.md            Initial domain/data model
  AI_PIPELINE.md           Thought processing and AI contracts
  EXAMPLES.md              Golden fixture captures for the AI pipeline
  MVP_PLAN.md              Design review + milestone plan for the MVP
```

## Development

### Prerequisites

- Rust (stable, 1.88+) and Node.js 22+
- Tauri system libraries for your OS — see <https://tauri.app/start/prerequisites/>.
  On Debian/Ubuntu: `libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev libayatana-appindicator3-dev libdbus-1-dev libxdo-dev`

### Run it

```sh
npm ci
npm run tauri dev        # desktop app with hot reload
npm run tauri build      # installable bundle for this OS
```

On first launch captures are saved and searchable immediately; AI processing waits until you add an
OpenRouter API key and model ids in **Settings** (or pick the **Mock** provider to try the full
pipeline offline).

### Test and lint

```sh
cargo test -p thoughtrouter-core            # core tests; also regenerates src/bindings
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
npm test                                     # UI tests (vitest)
npm run typecheck && npm run lint
```

### Evaluate a model against the golden fixtures

```sh
cargo run -p thoughtrouter-core --example eval -- --mock
OPENROUTER_API_KEY=… cargo run -p thoughtrouter-core --example eval -- \
    --analyzer <openrouter model id> [--embedder <embedding model id>] [--zdr]
```

### Data and configuration

| What | Where |
|---|---|
| Database | `<app data dir>/thoughtrouter.db` (Linux: `~/.local/share/com.sonoransolutions.thoughtrouter/`, macOS: `~/Library/Application Support/com.sonoransolutions.thoughtrouter/`, Windows: `%APPDATA%\com.sonoransolutions.thoughtrouter\`) |
| Backups | `<app data dir>/backups/` — one per launch, newest 10 kept |
| Exports | `<app data dir>/exports/` |
| API key | OS keychain, or `OPENROUTER_API_KEY` env var (takes precedence) |
| Dev data dir override | `THOUGHTROUTER_DATA_DIR=/some/dir` |

**Provider data flow:** capture text leaves the machine only when processing is on and the provider is
OpenRouter. Requests go to `openrouter.ai` with `provider.data_collection = "deny"` (and
`zdr = true` if enabled in Settings), so they are only routed to upstreams that don't train on or
retain prompts. Search queries are embedded through the same provider when an embedding model is
configured; otherwise search is word-based and fully local.

## Status

**MVP implemented (M1–M4); ready for dogfooding.** What's left is choosing default models and using it
for real — see the open items at the top of [`TASKS.md`](TASKS.md) and the dogfooding phase in
[`docs/MVP_PLAN.md`](docs/MVP_PLAN.md).
