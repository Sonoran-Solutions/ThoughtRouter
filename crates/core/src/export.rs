//! Data safety & portability: backups, Markdown/JSON export, JSON import.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params, types::ValueRef};
use serde_json::{Map, Value, json};

use crate::models::ExportResult;
use crate::util::now_iso;
use crate::{APP_VERSION, db, jobs};

pub const EXPORT_FORMAT: &str = "thoughtrouter-export/1";

fn stamp() -> String {
    chrono::Utc::now()
        .format("%Y%m%d-%H%M%S%.3f")
        .to_string()
        .replace('.', "-")
}

/// Consistent snapshot via `VACUUM INTO`; keeps the newest `keep` backups.
pub fn backup(conn: &Connection, dir: &Path, keep: usize) -> Result<PathBuf> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut path = dir.join(format!("thoughtrouter-{}.db", stamp()));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("thoughtrouter-{}-{n}.db", stamp()));
        n += 1;
    }
    conn.execute("VACUUM INTO ?1", [path.to_string_lossy()])?;
    let mut backups: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("thoughtrouter-") && n.ends_with(".db"))
        })
        .collect();
    // Sort by stem so "x-1.db" orders after "x.db".
    backups.sort_by_key(|p| p.file_stem().map(|s| s.to_os_string()));
    while backups.len() > keep.max(1) {
        let old = backups.remove(0);
        let _ = fs::remove_file(old);
    }
    Ok(path)
}

/// One Markdown file per day, raw text verbatim. Trashed captures excluded.
pub fn export_markdown(conn: &Connection, dir: &Path) -> Result<ExportResult> {
    let out = dir.join(format!("markdown-{}", stamp()));
    fs::create_dir_all(&out)?;
    let mut stmt = conn.prepare(
        "SELECT captured_at, source, text FROM captures WHERE deleted_at IS NULL ORDER BY captured_at, id",
    )?;
    let mut days: BTreeMap<String, String> = BTreeMap::new();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (at, source, text) = row?;
        let day = at.get(..10).unwrap_or("unknown").to_string();
        let time = at.get(11..19).unwrap_or("");
        let body = days
            .entry(day.clone())
            .or_insert_with(|| format!("# {day}\n"));
        body.push_str(&format!("\n## {time} UTC · {source}\n\n{text}\n"));
    }
    let mut files = Vec::new();
    for (day, body) in days {
        let f = out.join(format!("{day}.md"));
        fs::write(&f, body)?;
        files.push(f.to_string_lossy().into_owned());
    }
    Ok(ExportResult {
        directory: out.to_string_lossy().into_owned(),
        files,
    })
}

fn table_json(conn: &Connection, sql: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let mut m = Map::new();
        for (i, n) in names.iter().enumerate() {
            let v = match r.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(x) => json!(x),
                ValueRef::Real(x) => json!(x),
                ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                ValueRef::Blob(_) => Value::Null,
            };
            m.insert(n.clone(), v);
        }
        out.push(Value::Object(m));
    }
    Ok(out)
}

/// Everything except disposable data (embeddings, neighbours, job queue).
pub fn export_json(conn: &Connection, dir: &Path) -> Result<ExportResult> {
    fs::create_dir_all(dir)?;
    let doc = json!({
        "format": EXPORT_FORMAT,
        "app_version": APP_VERSION,
        "schema_version": db::schema_version(conn)?,
        "exported_at": now_iso(),
        "captures": table_json(conn, "SELECT * FROM captures ORDER BY captured_at, id")?,
        "projects": table_json(conn, "SELECT * FROM projects ORDER BY created_at")?,
        "atoms": table_json(conn, "SELECT * FROM atoms ORDER BY created_at")?,
        "atom_projects": table_json(conn, "SELECT * FROM atom_projects ORDER BY created_at")?,
        "project_suggestions": table_json(conn, "SELECT * FROM project_suggestions ORDER BY created_at")?,
        "project_syntheses": table_json(conn, "SELECT * FROM project_syntheses ORDER BY created_at")?,
        "resurfacing_events": table_json(conn, "SELECT * FROM resurfacing_events ORDER BY shown_at")?,
        "processor_runs": table_json(conn, "SELECT * FROM processor_runs ORDER BY started_at")?,
    });
    let f = dir.join(format!("thoughtrouter-{}.json", stamp()));
    fs::write(&f, serde_json::to_vec_pretty(&doc)?)?;
    Ok(ExportResult {
        directory: dir.to_string_lossy().into_owned(),
        files: vec![f.to_string_lossy().into_owned()],
    })
}

#[derive(Debug, Default, PartialEq)]
pub struct ImportSummary {
    pub captures_added: usize,
    pub projects_added: usize,
}

/// Imports source data (captures with original ids/timestamps, projects)
/// from a JSON export. Derived data is regenerated by reprocessing, so no AI
/// provider state is required (docs/ARCHITECTURE.md "Data portability").
pub fn import_json(conn: &mut Connection, file: &Path) -> Result<ImportSummary> {
    let doc: Value = serde_json::from_slice(&fs::read(file)?).context("not a JSON export")?;
    if doc["format"] != EXPORT_FORMAT {
        anyhow::bail!("unsupported export format: {}", doc["format"]);
    }
    let tx = conn.transaction()?;
    let mut summary = ImportSummary::default();
    for c in doc["captures"].as_array().into_iter().flatten() {
        let n = tx.execute(
            "INSERT OR IGNORE INTO captures (id, text, source, captured_at, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                c["id"].as_str(),
                c["text"].as_str(),
                c["source"].as_str().unwrap_or("import"),
                c["captured_at"].as_str(),
                c["deleted_at"].as_str()
            ],
        )?;
        if n > 0 {
            summary.captures_added += 1;
            if c["deleted_at"].is_null() {
                jobs::enqueue(
                    &tx,
                    jobs::JobType::AnalyzeCapture,
                    c["id"].as_str().unwrap_or_default(),
                )?;
            }
        }
    }
    for p in doc["projects"].as_array().into_iter().flatten() {
        let n = tx.execute(
            "INSERT OR IGNORE INTO projects (id, name, description, momentum, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                p["id"].as_str(),
                p["name"].as_str(),
                p["description"].as_str().unwrap_or(""),
                p["momentum"].as_str().unwrap_or("exploring"),
                p["created_at"].as_str(),
                p["updated_at"].as_str()
            ],
        )?;
        if n > 0 {
            summary.projects_added += 1;
            jobs::enqueue(
                &tx,
                jobs::JobType::EmbedProject,
                p["id"].as_str().unwrap_or_default(),
            )?;
        }
    }
    tx.commit()?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Momentum;
    use crate::{Db, captures, projects};

    #[test]
    fn backup_is_a_usable_database_and_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("tr.db")).unwrap();
        captures::create(&mut db.conn(), "backed up", "t").unwrap();
        let bdir = dir.path().join("backups");
        let mut last = PathBuf::new();
        for _ in 0..4 {
            last = backup(&db.conn(), &bdir, 2).unwrap();
        }
        assert_eq!(fs::read_dir(&bdir).unwrap().count(), 2);
        let restored = Db::open(&last).unwrap();
        assert_eq!(
            captures::list(&restored.conn(), None, 10).unwrap()[0].text,
            "backed up"
        );
    }

    #[test]
    fn markdown_export_preserves_exact_text() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let raw = "Line one\n\n  # not a heading, just my text 😀";
        captures::create(&mut db.conn(), raw, "desktop").unwrap();
        let r = export_markdown(&db.conn(), dir.path()).unwrap();
        assert_eq!(r.files.len(), 1);
        let body = fs::read_to_string(&r.files[0]).unwrap();
        assert!(body.contains(raw));
    }

    #[test]
    fn json_export_then_import_round_trips_source_data() {
        let dir = tempfile::tempdir().unwrap();
        let src = Db::open_in_memory().unwrap();
        let c = captures::create(&mut src.conn(), "portable 😀", "desktop").unwrap();
        projects::create(&src.conn(), "Save Doctor", "desc", Momentum::Active).unwrap();
        let r = export_json(&src.conn(), dir.path()).unwrap();

        let dst = Db::open_in_memory().unwrap();
        let s = import_json(&mut dst.conn(), Path::new(&r.files[0])).unwrap();
        assert_eq!(
            s,
            ImportSummary {
                captures_added: 1,
                projects_added: 1
            }
        );
        let back = captures::get(&dst.conn(), &c.id).unwrap().unwrap();
        assert_eq!(back, c, "id, text and timestamp preserved");
        // Idempotent.
        let s = import_json(&mut dst.conn(), Path::new(&r.files[0])).unwrap();
        assert_eq!(s, ImportSummary::default());
    }
}
