# Example Captures / Golden Fixtures

These examples are starter fixtures for AI-pipeline development and regression testing.

They are not meant to require exact generated wording. Tests should assert structural behavior and invariants.

## Fixture 1 — Multiple unrelated thoughts in one dump

### Raw capture

```text
Need to figure out why the handheld SSD keeps disconnecting. Also I still want one Linux control center that replaces the proprietary Windows app. The external cooler is probably the ugly part. Could an agent research that? Oh and Save Doctor could eventually detect weird save corruption automatically.
```

### Expected properties

- raw text remains untouched;
- produces multiple atoms;
- SSD disconnect is recognized as a problem/research lead;
- Linux control center is recognized as project/feature-level intent;
- cooler protocol investigation is research/question/task-like, not silently assumed completed;
- Save Doctor thought remains a separate spark/feature;
- one capture may link to multiple project clusters.

---

## Fixture 2 — Question, not commitment

### Raw capture

```text
Could DualDex eventually have a plugin system so other game-specific helpers can hook into it?
```

### Expected properties

- should not automatically become a committed task;
- likely type is Spark, Question, or Feature with uncertainty;
- may relate to DualDex;
- should preserve "eventually" as low immediacy rather than assigning urgency.

---

## Fixture 3 — Concrete task

### Raw capture

```text
Change the DualDex import button copy so it doesn't specifically say MyBoy anymore. RetroArch saves already work.
```

### Expected properties

- concrete task/feature atom exists;
- decision/evidence may be represented: RetroArch save import has already been tested;
- links strongly to DualDex;
- should not invent implementation details.

---

## Fixture 4 — Recurring concept

### Existing history

```text
Capture A: "Would be nice to replace the OneXPlayer Windows utility on Linux."
Capture B: "Most of the device controls already have Linux tools; maybe wrap them together."
Capture C: "The cooler still needs reverse engineering."
```

### New capture

```text
I keep coming back to that Linux control center idea. This might actually be worth building.
```

### Expected properties

- links strongly to the existing thread;
- recurrence signal increases;
- system may suggest promoting momentum from Spark/Exploring toward Ready;
- old captures remain visible in chronological history.

---

## Fixture 5 — Changed assumption

### Older capture

```text
The app probably needs its own cloud backend so I can access thoughts everywhere.
```

### New capture

```text
Actually keep ThoughtRouter local-first. I don't want cloud sync to block the first useful version.
```

### Expected properties

- new statement is a Decision;
- relationship to old statement may be `revises` or `contradicts`;
- current synthesis should prefer the newer explicit decision;
- old capture must not be deleted or rewritten.

---

## Fixture 6 — No project required

### Raw capture

```text
Random thought: maybe old malls would make sick settings for a horror game.
```

### Expected properties

- valid Spark even if no existing project matches;
- system should be comfortable leaving it unassigned;
- must not create a new project cluster automatically at low confidence.

---

## Fixture 7 — Many-to-many relationship

### Raw capture

```text
The save-state reverse engineering work for Pokémon Unbound might teach me techniques that are useful for Save Doctor too.
```

### Expected properties

- likely links to both Pokémon Unbound Completion and Save Doctor;
- identifies a cross-project relationship;
- may classify the relationship as related/shared research rather than ownership;
- this is a strong candidate for future "Your brain made a connection" resurfacing.

---

## Fixture 8 — "Not now" should not mean delete

### Raw capture

```text
A browser extension for ThoughtRouter would be useful someday, but absolutely not before the desktop version works.
```

### Expected properties

- browser extension is captured as Someday/Feature;
- explicit sequencing constraint is preserved;
- should not appear in current active tasks;
- may resurface after the desktop milestone is complete;
- thought remains searchable immediately.

---

# Suggested test invariants

Every processor implementation should satisfy these regardless of model/provider:

1. `Capture.text` is byte-for-byte unchanged after processing.
2. Processing failure does not remove or invalidate the capture.
3. A capture may produce zero, one, or many atoms.
4. An atom may have zero, one, or many project relationships.
5. A question is not automatically converted into a commitment.
6. User corrections outrank later AI guesses.
7. New explicit decisions can revise older decisions without erasing history.
8. Uncertainty is allowed.
9. "No relationship" is allowed.
10. Derived data can be regenerated without loss of source data.
