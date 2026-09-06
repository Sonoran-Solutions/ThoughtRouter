# Roadmap

This roadmap is intentionally biased toward proving the **capture → understand → connect → resurface** loop before adding integrations.

## Phase 0 — Foundation

Goal: create the smallest stable local app skeleton.

Deliverables:

- Tauri + React + TypeScript app boots locally
- SQLite database opens and migrates
- basic app settings/config layer
- basic logging/error handling
- provider-independent `ThoughtProcessor` interface
- mock processor for tests

Exit condition:

> The app can start, persist local data, and run deterministic tests without any external AI provider.

## Phase 1 — Capture

Goal: make ThoughtRouter useful as a safe dumping ground even before the AI is smart.

Deliverables:

- single brain-dump text box
- keyboard shortcut to save
- immutable raw `Capture` persistence
- recent capture history
- processing status UI
- retryable local processing job record

Exit condition:

> A messy thought can be saved in under a few seconds and remains accessible even if AI processing is unavailable.

## Phase 2 — Understand

Goal: extract useful structure without damaging the original thought.

Deliverables:

- first real `ThoughtProcessor` adapter
- strict structured-output schema
- atom extraction
- atom type/confidence storage
- provenance/version metadata
- UI that clearly separates raw text from AI interpretation
- manual correction of atom type/text where useful

Exit condition:

> A capture containing multiple different thoughts produces a sensible set of atoms, while the original remains untouched.

## Phase 3 — Connect

Goal: stop treating every capture as an island.

Deliverables:

- embeddings
- lexical/full-text search
- semantic search
- project entity
- many-to-many relationships
- project suggestions
- related-thought suggestions
- user confirm/reject relationship controls

Exit condition:

> A new thought can reliably rediscover older relevant thoughts and can relate to more than one project without manual filing.

## Phase 4 — Threads

Goal: reconstruct the evolution of an idea over time.

Deliverables:

- thought/project timeline
- synthesized current understanding
- open questions
- changed assumptions/decisions
- possible next actions
- provenance links from synthesis back to source captures

Exit condition:

> Opening a project after weeks away gives enough context to understand where Past You left it.

## Phase 5 — Resurface

Goal: solve the actual forgetting problem.

Deliverables:

- "Resurface something" action
- recurrence detection
- forgotten-thought resurfacing
- recently-shown suppression
- `Interesting / Not now / Make active / Dismiss` feedback
- resurfacing event history
- first version of home-screen cards

Exit condition:

> The app brings back at least a few genuinely useful forgotten ideas during normal personal use without becoming annoying.

## Phase 6 — Dogfood hard

Goal: stop building features long enough to learn from real use.

Do not rush this phase.

Use ThoughtRouter for several weeks and collect:

- captures that were classified badly;
- relationships that felt wrong;
- ideas that should have resurfaced but did not;
- resurfacing that felt annoying;
- moments when capture friction caused a thought to be lost;
- features you repeatedly wish existed.

Deliverables:

- golden test fixture corpus from real anonymized/personal examples
- revised atom vocabulary if needed
- revised ranking heuristics
- UX cleanup based on actual friction
- database backup/export

Exit condition:

> ThoughtRouter is something you reach for reflexively instead of something you have to remember to use.

## Phase 7 — Faster capture surfaces

Only after dogfooding proves value.

Potential additions:

- global desktop hotkey
- system tray capture
- CLI capture command
- Android capture app
- Android home-screen widget
- Android Quick Settings tile
- Android share target
- voice transcription
- browser extension

Priority rule: build whichever capture surface would have prevented the most real lost thoughts during Phase 6.

## Phase 8 — External context and actions

Only after the thought model is trustworthy.

Potential integrations:

- GitHub issue/project awareness
- create GitHub issue from a confirmed task
- ingest selected ChatGPT conversations
- send research atoms to an agent
- import URLs/articles/references
- project-aware agent prompt generation
- calendar/reminder handoff where explicitly useful

ThoughtRouter should remain the thinking layer; external systems can remain the execution layer.

## Phase 9 — Local AI / advanced intelligence

Potential additions:

- local LLM processor
- local embeddings
- idea-collision engine
- contradiction detection
- recurring-concept promotion
- automatic stale-project summaries
- personalized resurfacing ranking learned from feedback

## Someday / maybe

These are deliberately not promises:

- multi-device sync
- web UI
- plugin system
- public release
- multi-user support
- hosted service
- iOS client

If the app becomes valuable enough personally, revisit them then.

## Scope-control rule

Before starting any post-MVP feature, ask:

1. Did real usage reveal this need?
2. Does it improve capture, understanding, connection, or resurfacing?
3. Is it more valuable than fixing a known failure in the core loop?

If the answer is mostly "it would be cool," put it in Someday and keep shipping the core.
