# AI Processing Pipeline

AI is a derived interpretation layer. It is never the canonical record of what the user thought.

## Primary invariant

```text
RAW CAPTURE IS SACRED
```

The system may classify, summarize, connect, and reinterpret a capture, but it must always preserve the exact original text.

## Pipeline

```text
Raw Capture
   ↓
Durable local save
   ↓
Atom extraction
   ↓
Embedding generation
   ↓
Candidate retrieval
   ↓
Relationship classification
   ↓
Project/theme suggestions
   ↓
Resurfacing signal updates
```

Each stage should be independently retryable where practical.

## Stage 1: atom extraction

Input:

- raw capture text;
- optionally minimal known context needed to resolve obvious references.

Output should use a strict schema rather than freeform prose.

Illustrative shape:

```ts
type AtomType =
  | "spark"
  | "project"
  | "feature"
  | "task"
  | "question"
  | "research"
  | "reference"
  | "decision"
  | "problem"
  | "someday";

interface ExtractedAtom {
  text: string;
  type: AtomType;
  confidence: number;
  sourceSpan?: {
    start: number;
    end: number;
  };
}

interface CaptureAnalysis {
  atoms: ExtractedAtom[];
  needsClarification: boolean;
  notes?: string[];
}
```

### Extraction rules

The processor should:

- split multiple independent thoughts when useful;
- retain uncertainty;
- avoid inventing commitments;
- distinguish "I should" from "I wonder if";
- preserve meaningful project names and proper nouns;
- avoid converting every sentence into a task;
- avoid forcing every atom into an existing project.

## Stage 2: embeddings

Generate embeddings for at least:

- raw capture;
- extracted atoms;
- project summaries.

Keep the embedding model/version recorded so vectors can be regenerated after provider/model changes.

## Stage 3: candidate retrieval

Before asking a model whether two thoughts are related, cheaply retrieve plausible candidates.

Use a blend of:

- vector similarity;
- lexical search;
- recently active projects;
- repeated named entities;
- explicit prior relationships.

Do not send the user's entire database to the model for every capture.

## Stage 4: relationship classification

Given a new atom and a small candidate set, classify potential relationships.

Possible result:

```json
{
  "targetId": "project_123",
  "relationship": "belongs_to",
  "confidence": 0.91,
  "reason": "Both concern replacing the same proprietary handheld utility on Linux."
}
```

The `reason` is useful for debugging and user-facing explanation but remains derived data.

## Stage 5: project suggestions

The processor may suggest:

- linking an atom to an existing project;
- creating a new project cluster;
- merging obviously duplicate project clusters;
- leaving the atom unassigned.

"No project" is a valid answer.

Project creation should use a higher threshold than relationship creation so the app does not generate dozens of useless micro-projects.

## Stage 6: thought-thread synthesis

A thread synthesis should summarize **current understanding**, not overwrite history.

Suggested schema:

```ts
interface ThreadSynthesis {
  summary: string;
  openQuestions: string[];
  possibleNextActions: string[];
  decisions: string[];
  changedAssumptions: string[];
  relatedProjectIds: string[];
}
```

Each synthesis should record which captures/atoms it was based on.

## Stage 7: resurfacing signals

The AI may contribute semantic signals, but resurfacing should not be entirely delegated to a generative model.

Prefer deterministic inputs where possible:

- number of mentions;
- time since last interaction;
- active-project relevance;
- unresolved questions;
- explicit "not now" history;
- recently shown penalty;
- similarity to newly captured thoughts.

Use an LLM when explanation or nuanced connection detection adds value.

## Correction loop

User corrections are training signals for the local system even if no model fine-tuning occurs.

Examples:

- "This belongs to DualDex, not Save Doctor."
- "These are unrelated."
- "This isn't a task; it's just an idea."
- "These two projects are actually the same project."

Store those decisions explicitly.

Future processing must avoid casually overriding them.

## Prompt/version discipline

Every processing run should record enough metadata to reproduce or debug behavior:

```text
provider
model
prompt_version
schema_version
application_version
```

Prompts should live in version-controlled source rather than being hidden in random UI components.

## Failure behavior

AI failure must never block capture.

If processing fails:

```text
Capture status: Saved
Processing status: Needs retry
```

The user should still be able to search/read the raw capture.

## Provider independence

The core should not know whether analysis came from:

- OpenAI;
- a local model;
- another remote provider;
- a deterministic test double.

Provider-specific code belongs behind a `ThoughtProcessor` adapter.

## Local model future

Local inference is a strong eventual fit because the corpus may contain private and highly contextual data.

Do not make local inference a blocker for v0.1. First establish the interface and get the behavior right with whichever provider makes iteration easiest.

## Quality evaluation

Avoid evaluating the processor only by "does this output look smart?"

Maintain golden examples that test structural behavior.

Useful assertions:

- all independent thoughts were represented;
- no fabricated task was created;
- a question remained a question;
- one capture may map to multiple projects;
- old explicit user corrections are respected;
- uncertain relationships have lower confidence;
- the original capture never changes.

See `docs/EXAMPLES.md` for starter fixtures.
