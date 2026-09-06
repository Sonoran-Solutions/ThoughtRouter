# Data Model

This model is intentionally small. The goal is to support the core loop without prematurely designing a universal knowledge graph.

## Principles

1. Raw captures are immutable source records.
2. AI-derived data is stored separately and can be regenerated.
3. Relationships are first-class and many-to-many.
4. User-confirmed data outranks AI suggestions.
5. Every derived object should retain provenance.

## Core entities

### Capture

The exact thing the user submitted.

```text
Capture
- id
- text
- source
- captured_at
- created_at
- updated_at
```

Possible future fields:

```text
- shared_url
- attachment_manifest
- device_id
- transcription_id
```

Invariant: `text` is never silently rewritten by AI.

### Atom

A smaller interpreted thought extracted from a capture.

```text
Atom
- id
- capture_id
- text
- type
- confidence
- processor_version
- created_at
```

Initial `type` values:

```text
spark
project
feature
task
question
research
reference
decision
problem
someday
```

The atom text is a derived representation. It should link back to the original capture.

### Project

A semantic cluster that represents an evolving project or body of work.

```text
Project
- id
- name
- description
- momentum
- created_at
- updated_at
```

Suggested `momentum` values:

```text
spark
exploring
ready
active
blocked
dormant
finished
```

### Relationship

A typed link between any two supported objects.

```text
Relationship
- id
- source_type
- source_id
- target_type
- target_id
- relationship_type
- confidence
- origin
- status
- created_at
- updated_at
```

Possible `relationship_type` values:

```text
related_to
belongs_to
extends
contradicts
blocks
blocked_by
requires
supports
similar_to
revises
inspired_by
```

Possible `origin` values:

```text
user
ai
system
import
```

Possible `status` values:

```text
suggested
confirmed
rejected
```

Do not over-expand the relationship vocabulary until real captures expose missing semantics.

### Action

A lightweight actionable interpretation. This should not evolve into a full enterprise task system by accident.

```text
Action
- id
- atom_id
- status
- created_at
- completed_at
```

Possible status values:

```text
open
in_progress
blocked
done
dismissed
```

Due dates should be optional and absent from the MVP unless usage proves they are needed.

### Embedding

Vector representation used for semantic retrieval.

```text
Embedding
- id
- object_type
- object_id
- model
- dimensions
- vector
- created_at
```

Embeddings are fully disposable derived data.

### ProcessingJob

Tracks resumable local processing.

```text
ProcessingJob
- id
- capture_id
- job_type
- status
- attempts
- last_error
- created_at
- updated_at
```

### ProcessorRun

Useful for provenance and future reprocessing.

```text
ProcessorRun
- id
- capture_id
- provider
- model
- prompt_version
- schema_version
- started_at
- completed_at
- status
- raw_response_optional
```

Storing raw provider responses should be configurable because they may contain redundant/private content.

### ResurfacingEvent

Records what the system showed and how the user reacted.

```text
ResurfacingEvent
- id
- object_type
- object_id
- reason
- score
- shown_at
- response
```

Possible responses:

```text
interesting
not_now
make_active
dismiss
opened
ignored
```

This history is important so the system does not repeatedly surface the same annoying item.

## Example

Raw input:

```text
Need to fix the SSD disconnect issue on the handheld. Also want a Linux replacement for the proprietary control app. The cooler protocol is probably the ugly part and maybe an agent could research it.
```

Stored as one `Capture`.

Derived atoms might be:

```text
Problem:
"External SSD intermittently disconnects on the handheld."

Project:
"Build a Linux replacement for the proprietary control application."

Research:
"Determine how the external cooler protocol works."

Task:
"Investigate the cooler protocol with an AI/research agent."
```

Then relationships might link the project and research atom to the same existing project cluster while leaving the SSD problem as a separate but related thread.

## Candidate SQLite schema

This is illustrative, not migration-ready SQL.

```sql
captures(
  id TEXT PRIMARY KEY,
  text TEXT NOT NULL,
  source TEXT,
  captured_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

atoms(
  id TEXT PRIMARY KEY,
  capture_id TEXT NOT NULL REFERENCES captures(id),
  text TEXT NOT NULL,
  type TEXT NOT NULL,
  confidence REAL,
  processor_version TEXT,
  created_at TEXT NOT NULL
);

projects(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  momentum TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

relationships(
  id TEXT PRIMARY KEY,
  source_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  target_type TEXT NOT NULL,
  target_id TEXT NOT NULL,
  relationship_type TEXT NOT NULL,
  confidence REAL,
  origin TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

processing_jobs(
  id TEXT PRIMARY KEY,
  capture_id TEXT NOT NULL REFERENCES captures(id),
  job_type TEXT NOT NULL,
  status TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

## Data ownership hierarchy

When data conflicts, use this priority:

```text
explicit current user correction
    > explicit prior user decision
    > confirmed derived relationship
    > current AI suggestion
    > inferred similarity
```

The model is an assistant to the user's memory, not the owner of it.
