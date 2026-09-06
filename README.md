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

## Proposed stack

Initial direction, not an irreversible commitment:

```text
Desktop UI       React + TypeScript
Desktop shell    Tauri
Local database   SQLite
Vector search    sqlite-vec (or equivalent SQLite extension)
AI               Provider-agnostic ThoughtProcessor interface
Primary mode     Local-first
Later clients    Android / browser capture / CLI / share targets
```

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Repository map

```text
README.md                  Project overview
TASKS.md                   Concrete implementation checklist
ROADMAP.md                 Phased build plan
DECISIONS.md               Important product/technical decisions

docs/
  PRODUCT.md               Product model and principles
  UX.md                    Intended interaction model
  ARCHITECTURE.md          Technical architecture
  DATA_MODEL.md            Initial domain/data model
  AI_PIPELINE.md           Thought processing and AI contracts
```

## First milestone

The first milestone is deliberately boring:

> Type a messy thought, save it locally, see the untouched original, and see a model extract useful atoms without losing anything.

If that loop does not feel good, nothing else matters yet.

## Status

**Pre-alpha / design phase.**

The next concrete work is tracked in [`TASKS.md`](TASKS.md).
