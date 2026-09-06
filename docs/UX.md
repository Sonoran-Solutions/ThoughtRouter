# UX Model

## UX goal

ThoughtRouter should feel like **throwing a thought somewhere safe**, not filling out a form.

The interface should minimize decisions at capture time and maximize useful context later.

## Primary capture screen

The default screen should be nearly empty.

```text
┌──────────────────────────────────────┐
│                                      │
│        What's on your mind?          │
│                                      │
│  [                                  ]│
│  [                                  ]│
│  [                                  ]│
│                                      │
│             Save Thought             │
│                                      │
└──────────────────────────────────────┘
```

Requirements:

- keyboard-first;
- one obvious action;
- no mandatory metadata;
- save should feel instantaneous;
- user should get immediate confirmation that raw text is safe.

AI processing can happen after the save.

## Capture history

The user must be able to browse raw captures chronologically.

Each item should show:

- exact raw text;
- timestamp;
- processing status;
- extracted atoms when expanded;
- linked projects/themes when expanded.

The raw text should be visually distinguished from generated interpretation.

## Suggested processing state

```text
Saved
Analyzing…
Processed
Needs retry
```

Do not represent a temporary model failure as a failed capture.

## Thought Thread view

A thread is an evolving idea reconstructed from multiple captures.

Suggested layout:

```text
Project / Thought Thread Name

Current understanding
[generated synthesis]

Open questions
- ...
- ...

Possible next actions
- ...

Timeline
Sep 06 13:44  raw capture...
Sep 02 22:10  raw capture...
Aug 28 18:31  raw capture...
```

The synthesis is convenience. The timeline is evidence.

## Relationship correction

AI suggestions should be correctable with extremely low friction.

Example:

```text
Related to: OneXPlayer Linux   [✓] [change]
Also related: Save Doctor      [?] [reject]
```

Avoid modal-heavy taxonomy editors.

## "You said this before"

While viewing or capturing a thought, semantic matches may appear unobtrusively.

```text
Possibly related

"Recreate Sims family in InZOI using AI"
Captured 4 months ago
```

This should not block typing or force a decision.

## Home screen

The eventual home screen should prioritize useful resurfacing over backlog guilt.

Possible modules:

### Right now

Active thought clusters with recent activity or concrete next actions.

### You keep coming back to this

Repeated concepts that may deserve promotion into a project.

### Your brain made a connection

Interesting cross-project relationships.

### From the vault

Older thoughts not seen recently.

### Recently captured

A compact safety net showing that recent dumps were actually saved.

## Resurfacing card actions

Keep responses small and meaningful:

```text
[Interesting]
[Not now]
[Make active]
[Dismiss]
```

These actions should affect future resurfacing behavior.

## Search

Search should accept natural language.

Examples:

```text
that idea about editing sims without changing households
linux control app for the handheld
pokemon save persistence research
stuff I thought about for android recently
```

Results can blend:

- exact/lexical matches;
- semantically similar captures;
- related project threads;
- decisions and tasks.

## Visual language

The UI should make a clear distinction between:

- **what the user actually wrote**;
- **what the system inferred**.

Never render an AI summary in a way that makes it look like historical user-authored text.

## Notification philosophy

Avoid turning ThoughtRouter into another app that nags.

Notifications should eventually be selective and configurable.

Good:

> You have mentioned this project four times this week. It may be worth promoting.

Bad:

> You have 37 overdue thoughts.

There should be no concept of an "overdue thought."

## Mobile capture future

Once the desktop MVP proves useful, prioritize capture speed over feature parity.

Potential Android surfaces:

- home-screen widget;
- quick settings tile;
- share target;
- notification shade action;
- voice shortcut.

The mobile client does not initially need the full project-management UI.

## UX smell test

Before adding a control, ask:

> Does this make the user think about organizing the thought before they have safely captured it?

If yes, move that control out of the capture flow.
