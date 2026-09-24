-- ThoughtRouter schema v1. See docs/MVP_PLAN.md ("MVP schema") and docs/DATA_MODEL.md.
-- Timestamps are ISO-8601 UTC text with millisecond precision; ids are UUIDv7 text.

-- SOURCE DATA ------------------------------------------------------------

CREATE TABLE captures (
  id          TEXT PRIMARY KEY,
  text        TEXT NOT NULL,
  source      TEXT NOT NULL DEFAULT 'desktop',
  captured_at TEXT NOT NULL,
  deleted_at  TEXT -- user tombstone; the only mutable column (D-018)
);
CREATE INDEX captures_by_time ON captures(captured_at);

-- D-003 / D-018: nobody (including the AI pipeline) may rewrite a capture.
CREATE TRIGGER captures_immutable
BEFORE UPDATE OF id, text, source, captured_at ON captures
BEGIN
  SELECT RAISE(ABORT, 'captures are immutable');
END;

-- Full-text index. Stores its own copy keyed by capture id (rather than an
-- external-content table keyed by the implicit rowid, which VACUUM may renumber).
CREATE VIRTUAL TABLE captures_fts USING fts5(
  id UNINDEXED, text, tokenize = 'porter unicode61 remove_diacritics 2'
);
CREATE TRIGGER captures_fts_insert AFTER INSERT ON captures BEGIN
  INSERT INTO captures_fts(id, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER captures_fts_delete AFTER DELETE ON captures BEGIN
  DELETE FROM captures_fts WHERE id = old.id;
END;

CREATE TABLE projects (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
  description TEXT NOT NULL DEFAULT '',
  momentum    TEXT NOT NULL DEFAULT 'exploring',
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- PROVENANCE / QUEUE -----------------------------------------------------

CREATE TABLE processor_runs (
  id             TEXT PRIMARY KEY,
  capture_id     TEXT REFERENCES captures(id) ON DELETE CASCADE,
  kind           TEXT NOT NULL, -- analyze | embed | link | synthesize
  provider       TEXT NOT NULL,
  model          TEXT NOT NULL,
  prompt_version TEXT,
  schema_version TEXT,
  app_version    TEXT NOT NULL,
  status         TEXT NOT NULL, -- succeeded | failed
  error          TEXT,
  started_at     TEXT NOT NULL,
  finished_at    TEXT
);

CREATE TABLE processing_jobs (
  id         TEXT PRIMARY KEY,
  job_type   TEXT NOT NULL, -- analyze_capture | embed_capture | link_projects | embed_project
  target_id  TEXT NOT NULL, -- capture id, or project id for embed_project
  status     TEXT NOT NULL, -- queued | running | done | failed
  attempts   INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  run_after  TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX jobs_by_status ON processing_jobs(status, run_after);
CREATE INDEX jobs_by_target ON processing_jobs(target_id);

-- DERIVED DATA (+ user corrections, which are source data: D-019) ---------

CREATE TABLE atoms (
  id         TEXT PRIMARY KEY,
  capture_id TEXT NOT NULL REFERENCES captures(id) ON DELETE CASCADE,
  run_id     TEXT REFERENCES processor_runs(id) ON DELETE SET NULL,
  text       TEXT NOT NULL,
  type       TEXT NOT NULL,
  confidence REAL,
  quote      TEXT,
  span_start INTEGER, -- UTF-16 offsets into captures.text (JS-friendly)
  span_end   INTEGER,
  origin     TEXT NOT NULL DEFAULT 'ai',     -- ai | user
  status     TEXT NOT NULL DEFAULT 'active', -- active | rejected | superseded
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX atoms_by_capture ON atoms(capture_id);

CREATE VIRTUAL TABLE atoms_fts USING fts5(
  id UNINDEXED, text, tokenize = 'porter unicode61 remove_diacritics 2'
);
CREATE TRIGGER atoms_fts_insert AFTER INSERT ON atoms BEGIN
  INSERT INTO atoms_fts(id, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER atoms_fts_update AFTER UPDATE OF text ON atoms BEGIN
  DELETE FROM atoms_fts WHERE id = old.id;
  INSERT INTO atoms_fts(id, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER atoms_fts_delete AFTER DELETE ON atoms BEGIN
  DELETE FROM atoms_fts WHERE id = old.id;
END;

CREATE TABLE atom_projects (
  atom_id    TEXT NOT NULL REFERENCES atoms(id) ON DELETE CASCADE,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  status     TEXT NOT NULL, -- suggested | confirmed | rejected
  origin     TEXT NOT NULL, -- ai | user
  confidence REAL,
  reason     TEXT,
  run_id     TEXT REFERENCES processor_runs(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (atom_id, project_id)
);
CREATE INDEX atom_projects_by_project ON atom_projects(project_id);

-- "Create project?" suggestions (D-017: never auto-created).
CREATE TABLE project_suggestions (
  id          TEXT PRIMARY KEY,
  atom_id     TEXT NOT NULL REFERENCES atoms(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status      TEXT NOT NULL DEFAULT 'pending', -- pending | accepted | dismissed
  run_id      TEXT REFERENCES processor_runs(id) ON DELETE SET NULL,
  created_at  TEXT NOT NULL
);

CREATE TABLE embeddings (
  object_type TEXT NOT NULL, -- capture | atom | project
  object_id   TEXT NOT NULL,
  model       TEXT NOT NULL,
  dims        INTEGER NOT NULL,
  vector      BLOB NOT NULL, -- little-endian f32, L2-normalised
  created_at  TEXT NOT NULL,
  PRIMARY KEY (object_type, object_id, model)
);

-- Top-k nearest atoms, computed when an atom is embedded (drives recurrence).
CREATE TABLE atom_neighbors (
  atom_id     TEXT NOT NULL REFERENCES atoms(id) ON DELETE CASCADE,
  neighbor_id TEXT NOT NULL REFERENCES atoms(id) ON DELETE CASCADE,
  similarity  REAL NOT NULL,
  model       TEXT NOT NULL,
  PRIMARY KEY (atom_id, neighbor_id, model)
);

CREATE TABLE project_syntheses (
  id               TEXT PRIMARY KEY,
  project_id       TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  run_id           TEXT REFERENCES processor_runs(id) ON DELETE SET NULL,
  body_json        TEXT NOT NULL,
  source_capture_ids TEXT NOT NULL, -- JSON array: provenance
  based_on_through TEXT NOT NULL,   -- latest captured_at included; stale if newer exists
  created_at       TEXT NOT NULL
);

CREATE TABLE resurfacing_events (
  id           TEXT PRIMARY KEY,
  atom_id      TEXT NOT NULL REFERENCES atoms(id) ON DELETE CASCADE,
  score        REAL NOT NULL,
  reasons_json TEXT NOT NULL,
  shown_at     TEXT NOT NULL,
  response     TEXT, -- interesting | not_now | make_active | dismiss
  responded_at TEXT
);
CREATE INDEX resurfacing_by_atom ON resurfacing_events(atom_id);
