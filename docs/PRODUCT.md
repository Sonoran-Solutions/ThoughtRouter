# Product Model

## Purpose

ThoughtRouter exists to reduce the gap between **having a thought** and **being able to make use of it later**.

It is not primarily a task manager. It is not primarily a notes app. It is not a knowledge-base replacement.

It is a **thought router**: a system that accepts messy, incomplete, stream-of-consciousness input and helps determine what it relates to, whether it is actionable, and when it may be worth resurfacing.

## Primary user

For now, the primary user is the person building it.

That matters because design choices should optimize for actual personal behavior rather than generic productivity conventions.

The system should assume:

- ideas arrive at inconvenient times;
- one capture may contain several unrelated thoughts;
- thoughts often evolve across many separate captures;
- project boundaries are fuzzy;
- an idea may be interesting without deserving action yet;
- "not now" should not mean "lost forever";
- recurring thoughts are meaningful signals;
- forgotten context can become newly useful months later.

## Mental model

The user should not need to think in folders.

The system models knowledge more like a graph:

```text
Capture
  -> contains Atom
  -> relates to Project
  -> resembles another Atom
  -> raises Question
  -> suggests Action
  -> revises/extends older Thought Thread
```

Relationships may be many-to-many and confidence-weighted.

## Capture contract

At capture time the system should ask for as little as possible.

Preferred interaction:

```text
What's on your mind?

[ freeform text ]

[ Save Thought ]
```

Optional metadata may be captured automatically:

- timestamp
- source/device
- shared URL
- attachment reference
- voice transcription origin

The user should **not** be forced to choose:

- project
- category
- priority
- status
- tags
- due date

before saving.

## Raw thought preservation

A raw capture is the source of truth.

AI output must never replace it.

Derived interpretations should be stored separately so that:

- model mistakes are reversible;
- future models can reprocess old captures;
- provenance is obvious;
- the user can always see what they actually wrote.

## Atoms

A capture may be decomposed into one or more `Atoms`.

Initial vocabulary:

- **Spark** — an undeveloped "wouldn't it be cool if..." thought
- **Project** — a larger outcome worth pursuing
- **Feature** — an addition or change to an existing project
- **Task** — a concrete action
- **Question** — something unresolved
- **Research** — an investigation requiring evidence
- **Reference** — something useful to retain
- **Decision** — a conclusion already reached
- **Problem** — a pain point or defect
- **Someday** — interesting but intentionally uncommitted

This vocabulary should remain small unless real usage proves another type is necessary.

## Project model

Projects are **clusters**, not containers.

A capture can relate to multiple projects. An atom can be relevant to one project, several projects, or none.

Project membership should therefore be represented as relationships rather than ownership.

Example:

```text
"Reverse engineer undocumented save state"
   -> Pokémon Unbound Completion
   -> Save Doctor
   -> Reverse Engineering theme
```

## Momentum model

Avoid pretending every idea belongs in a conventional task workflow.

Suggested project momentum states:

- **Spark** — barely formed
- **Exploring** — being thought through
- **Ready** — sufficiently understood to start
- **Active** — currently receiving effort
- **Blocked** — waiting on something
- **Dormant** — intentionally not active
- **Finished** — complete or concluded

Dormant is a healthy state, not failure.

## Resurfacing

Resurfacing is a first-class product capability, not a notification afterthought.

Useful resurfacing modes include:

### Recurrence

> You've mentioned this idea five times.

### Forgotten context

> You wrote this 42 days ago and have not touched it since.

### Semantic connection

> This thought resembles work in another project.

### Maturation

> This has evolved from a spark into something with concrete next steps.

### Random vault

> Here's one older thought you have not seen recently.

### Contradiction or revision

> This decision appears to conflict with a later thought.

## Thought Threads

The app should be able to show how an idea evolved through time.

Example:

```text
Aug 28 — "Wish this Windows-only utility worked on Linux."
Sep 02 — "Most of it could wrap existing Linux tools."
Sep 06 — "The external cooler protocol may be the missing piece."
Sep 06 — "This should probably become a unified control center."
```

Above the chronology, the system may generate a current synthesized view:

- current understanding;
- unresolved questions;
- likely next actions;
- related projects;
- contradictions or changed assumptions.

The synthesis is derived. The chronology remains authoritative.

## Home-screen philosophy

The home screen should answer:

> What from my own brain is useful to me right now?

Not:

> Which arbitrary tasks are overdue?

Possible modules:

- Currently active
- You keep coming back to this
- Worth revisiting
- Your brain made a connection
- From the vault
- Recently captured

## Anti-goals

ThoughtRouter should resist becoming:

- a generic Jira clone;
- a Notion clone;
- a calendar replacement;
- a habit tracker;
- a team collaboration platform;
- a social network;
- an AI chat wrapper with a notes sidebar;
- a system that generates more maintenance work than it removes.

## Success criteria

The project is succeeding if, after several weeks of real use:

1. capturing a thought feels nearly frictionless;
2. the user trusts that thoughts will not disappear;
3. forgotten ideas reliably come back at useful moments;
4. project context becomes easier to reconstruct;
5. the system finds non-obvious relationships that are genuinely useful;
6. maintaining ThoughtRouter requires less work than maintaining conventional organization systems.

Everything else is secondary.
