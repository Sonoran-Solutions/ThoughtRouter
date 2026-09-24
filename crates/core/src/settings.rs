//! User settings, stored as one JSON document in the `settings` table.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

const KEY: &str = "app";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ProviderKind {
    /// OpenRouter (D-020). Needs an API key and model ids.
    Openrouter,
    /// Deterministic heuristics; no network. For trying the app/tests.
    Mock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ResurfaceWeights {
    pub dormancy: f64,
    pub recurrence: f64,
    pub active_project: f64,
    pub open_question: f64,
    pub exploration: f64,
    pub not_now_penalty: f64,
}

impl Default for ResurfaceWeights {
    /// docs/MVP_PLAN.md "Score v0". Starting points, to be tuned from feedback.
    fn default() -> Self {
        Self {
            dormancy: 1.0,
            recurrence: 1.0,
            active_project: 0.5,
            open_question: 0.3,
            exploration: 0.4,
            not_now_penalty: 2.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    /// Master switch for AI processing ("remote processing off" ⇒ jobs stay queued).
    pub processing_enabled: bool,
    pub provider: ProviderKind,
    /// OpenRouter model id for analysis/linking/synthesis, e.g. "vendor/model".
    /// PLACEHOLDER: intentionally empty until a default is chosen (TASKS §6).
    pub analyzer_model: String,
    /// OpenRouter embedding model id. Empty ⇒ semantic features disabled.
    /// PLACEHOLDER: intentionally empty until a default is chosen (TASKS §8).
    pub embedding_model: String,
    /// Route only to zero-data-retention endpoints (narrows model choice).
    pub openrouter_zdr: bool,
    /// Cosine similarity at which two atoms count as "the same idea again".
    /// PLACEHOLDER: depends on the embedding model; tune during dogfooding.
    pub similarity_threshold: f64,
    pub resurface_weights: ResurfaceWeights,
    /// Don't resurface an item again within this many days.
    pub resurface_suppress_days: i64,
    /// "Not now" penalty window.
    pub not_now_days: i64,
    pub backups_to_keep: i64,
    pub trash_retention_days: i64,
    /// Global capture hotkey (Tauri accelerator syntax).
    pub global_shortcut: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            processing_enabled: true,
            provider: ProviderKind::Openrouter,
            analyzer_model: String::new(),
            embedding_model: String::new(),
            openrouter_zdr: false,
            similarity_threshold: 0.75,
            resurface_weights: ResurfaceWeights::default(),
            resurface_suppress_days: 14,
            not_now_days: 30,
            backups_to_keep: 10,
            trash_retention_days: 30,
            global_shortcut: "CommandOrControl+Shift+Space".into(),
        }
    }
}

pub fn get(conn: &Connection) -> Result<Settings> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [KEY], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(match raw {
        Some(s) => serde_json::from_str(&s).unwrap_or_default(),
        None => Settings::default(),
    })
}

pub fn save(conn: &Connection, s: &Settings) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![KEY, serde_json::to_string(s)?],
    )?;
    Ok(())
}

/// Model id embeddings are stored under for the current settings, if any.
/// Reads (search, related, recurrence) use this without needing a key.
pub fn embedding_model_id(s: &Settings) -> Option<String> {
    match s.provider {
        ProviderKind::Mock => Some(format!("mock/hash-{}", crate::processor::mock::EMBED_DIMS)),
        ProviderKind::Openrouter if !s.embedding_model.trim().is_empty() => Some(format!(
            "{}:{}",
            crate::processor::openrouter::PROVIDER,
            s.embedding_model.trim()
        )),
        ProviderKind::Openrouter => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;

    #[test]
    fn defaults_then_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let mut s = get(&db.conn()).unwrap();
        assert_eq!(s, Settings::default());
        assert!(s.analyzer_model.is_empty(), "model ids are placeholders");
        s.provider = ProviderKind::Mock;
        s.resurface_weights.dormancy = 2.0;
        save(&db.conn(), &s).unwrap();
        assert_eq!(get(&db.conn()).unwrap(), s);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('app', '{\"provider\":\"mock\"}')",
                [],
            )
            .unwrap();
        let s = get(&db.conn()).unwrap();
        assert_eq!(s.provider, ProviderKind::Mock);
        assert_eq!(s.backups_to_keep, 10);
    }
}
