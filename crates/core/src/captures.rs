//! Raw captures: append-only source records (D-003, D-018).

use anyhow::{Result, bail};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::models::{Capture, CaptureView};
use crate::util::{new_id, now_iso};
use crate::{atoms, jobs, projects};

pub const SOURCE_DESKTOP: &str = "desktop";

pub(crate) fn from_row(r: &Row) -> rusqlite::Result<Capture> {
    Ok(Capture {
        id: r.get("id")?,
        text: r.get("text")?,
        source: r.get("source")?,
        captured_at: r.get("captured_at")?,
        deleted_at: r.get("deleted_at")?,
    })
}

const COLS: &str = "id, text, source, captured_at, deleted_at";

/// Durably stores the exact text and queues analysis in the same
/// transaction. No AI work happens here (D-010).
pub fn create(conn: &mut Connection, text: &str, source: &str) -> Result<Capture> {
    if text.trim().is_empty() {
        bail!("cannot save an empty thought");
    }
    let capture = Capture {
        id: new_id(),
        text: text.to_string(),
        source: source.to_string(),
        captured_at: now_iso(),
        deleted_at: None,
    };
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO captures (id, text, source, captured_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            capture.id,
            capture.text,
            capture.source,
            capture.captured_at
        ],
    )?;
    jobs::enqueue(&tx, jobs::JobType::AnalyzeCapture, &capture.id)?;
    tx.commit()?;
    Ok(capture)
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Capture>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLS} FROM captures WHERE id = ?1"),
            [id],
            from_row,
        )
        .optional()?)
}

pub fn get_live(conn: &Connection, id: &str) -> Result<Option<Capture>> {
    Ok(get(conn, id)?.filter(|c| c.deleted_at.is_none()))
}

/// Newest first. `before` is the id of the last capture on the previous page.
pub fn list(conn: &Connection, before: Option<&str>, limit: i64) -> Result<Vec<Capture>> {
    let cursor = match before {
        Some(id) => get(conn, id)?.map(|c| (c.captured_at, c.id)),
        None => None,
    };
    let mut out = Vec::new();
    match cursor {
        Some((at, id)) => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLS} FROM captures WHERE deleted_at IS NULL
                 AND (captured_at < ?1 OR (captured_at = ?1 AND id < ?2))
                 ORDER BY captured_at DESC, id DESC LIMIT ?3"
            ))?;
            for c in stmt.query_map(params![at, id, limit], from_row)? {
                out.push(c?);
            }
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLS} FROM captures WHERE deleted_at IS NULL
                 ORDER BY captured_at DESC, id DESC LIMIT ?1"
            ))?;
            for c in stmt.query_map(params![limit], from_row)? {
                out.push(c?);
            }
        }
    }
    Ok(out)
}

pub fn list_deleted(conn: &Connection) -> Result<Vec<Capture>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM captures WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn count_live(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM captures WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?)
}

/// Moves a capture to the trash. It disappears from history, search and
/// resurfacing but can be restored until purged.
pub fn trash(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute(
        "UPDATE captures SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, now_iso()],
    )?;
    if n == 0 {
        bail!("capture not found");
    }
    Ok(())
}

pub fn restore(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE captures SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

/// Permanently deletes a capture and everything derived from it.
pub fn purge(conn: &mut Connection, id: &str) -> Result<()> {
    let tx = conn.transaction()?;
    let atom_ids: Vec<String> = {
        let mut stmt = tx.prepare("SELECT id FROM atoms WHERE capture_id = ?1")?;
        stmt.query_map([id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for a in &atom_ids {
        tx.execute(
            "DELETE FROM embeddings WHERE object_type = 'atom' AND object_id = ?1",
            [a],
        )?;
    }
    tx.execute(
        "DELETE FROM embeddings WHERE object_type = 'capture' AND object_id = ?1",
        [id],
    )?;
    tx.execute("DELETE FROM processing_jobs WHERE target_id = ?1", [id])?;
    // Syntheses quoting this capture are no longer faithful to the source.
    tx.execute(
        "DELETE FROM project_syntheses WHERE EXISTS
           (SELECT 1 FROM json_each(project_syntheses.source_capture_ids) WHERE value = ?1)",
        [id],
    )?;
    // Cascades: atoms -> links, neighbours, suggestions, events; runs.
    tx.execute("DELETE FROM captures WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(())
}

/// Purges trashed captures older than `days`.
pub fn purge_trash_older_than(conn: &mut Connection, days: i64) -> Result<usize> {
    let cutoff = crate::util::iso(chrono::Utc::now() - chrono::Duration::days(days));
    let ids: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT id FROM captures WHERE deleted_at IS NOT NULL AND deleted_at < ?1")?;
        stmt.query_map([cutoff], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for id in &ids {
        purge(conn, id)?;
    }
    Ok(ids.len())
}

pub fn view(conn: &Connection, capture: Capture) -> Result<CaptureView> {
    let (state, last_error) = jobs::capture_state(conn, &capture.id)?;
    let atoms = atoms::views_for_capture(conn, &capture.id)?;
    let pending_suggestions = projects::pending_suggestions_for_capture(conn, &capture.id)?;
    let interpreted_by: Option<String> = conn
        .query_row(
            "SELECT provider || ' · ' || model FROM processor_runs
             WHERE capture_id = ?1 AND kind = 'analyze' AND status = 'succeeded'
             ORDER BY finished_at DESC LIMIT 1",
            [&capture.id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(CaptureView {
        capture,
        state,
        last_error,
        atoms,
        pending_suggestions,
        interpreted_by,
    })
}

pub fn views(conn: &Connection, captures: Vec<Capture>) -> Result<Vec<CaptureView>> {
    captures.into_iter().map(|c| view(conn, c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;

    #[test]
    fn text_round_trips_byte_for_byte() {
        let db = Db::open_in_memory().unwrap();
        let raw = "  Need to fix SSD 😀\n\tAlso: “quotes”, trailing spaces   \r\nüñíçødé\n";
        let c = create(&mut db.conn(), raw, SOURCE_DESKTOP).unwrap();
        let back = get(&db.conn(), &c.id).unwrap().unwrap();
        assert_eq!(back.text.as_bytes(), raw.as_bytes());
    }

    #[test]
    fn rejects_empty() {
        let db = Db::open_in_memory().unwrap();
        assert!(create(&mut db.conn(), "  \n ", SOURCE_DESKTOP).is_err());
    }

    #[test]
    fn trigger_blocks_text_update() {
        let db = Db::open_in_memory().unwrap();
        let c = create(&mut db.conn(), "original", SOURCE_DESKTOP).unwrap();
        let err = db
            .conn()
            .execute(
                "UPDATE captures SET text = 'rewritten' WHERE id = ?1",
                [&c.id],
            )
            .unwrap_err();
        assert!(err.to_string().contains("immutable"));
        let err = db
            .conn()
            .execute(
                "UPDATE captures SET captured_at = 'x' WHERE id = ?1",
                [&c.id],
            )
            .unwrap_err();
        assert!(err.to_string().contains("immutable"));
        assert_eq!(get(&db.conn(), &c.id).unwrap().unwrap().text, "original");
    }

    #[test]
    fn capture_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tr.db");
        let id = {
            let db = Db::open(&path).unwrap();
            create(&mut db.conn(), "persist me", SOURCE_DESKTOP)
                .unwrap()
                .id
        };
        let db = Db::open(&path).unwrap();
        assert_eq!(get(&db.conn(), &id).unwrap().unwrap().text, "persist me");
    }

    #[test]
    fn create_queues_analysis() {
        let db = Db::open_in_memory().unwrap();
        let c = create(&mut db.conn(), "hello", SOURCE_DESKTOP).unwrap();
        let (q, _, _) = jobs::counts(&db.conn()).unwrap();
        assert_eq!(q, 1);
        let v = view(&db.conn(), c).unwrap();
        assert_eq!(v.state, crate::models::ProcessingState::Saved);
    }

    #[test]
    fn list_paginates_newest_first_and_hides_trash() {
        let db = Db::open_in_memory().unwrap();
        let mut ids = vec![];
        for i in 0..5 {
            ids.push(
                create(&mut db.conn(), &format!("t{i}"), SOURCE_DESKTOP)
                    .unwrap()
                    .id,
            );
        }
        trash(&db.conn(), &ids[2]).unwrap();
        let p1 = list(&db.conn(), None, 2).unwrap();
        assert_eq!(
            p1.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
            ["t4", "t3"]
        );
        let p2 = list(&db.conn(), Some(&p1[1].id), 10).unwrap();
        assert_eq!(
            p2.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
            ["t1", "t0"]
        );
        assert_eq!(list_deleted(&db.conn()).unwrap().len(), 1);
        restore(&db.conn(), &ids[2]).unwrap();
        assert_eq!(count_live(&db.conn()).unwrap(), 5);
    }

    #[test]
    fn purge_removes_capture_and_derived_data() {
        let db = Db::open_in_memory().unwrap();
        let c = create(&mut db.conn(), "secret password 1234", SOURCE_DESKTOP).unwrap();
        atoms::add_user_atom(
            &db.conn(),
            &c.id,
            "secret",
            crate::models::AtomType::Reference,
        )
        .unwrap();
        trash(&db.conn(), &c.id).unwrap();
        purge(&mut db.conn(), &c.id).unwrap();
        let conn = db.conn();
        assert!(get(&conn, &c.id).unwrap().is_none());
        for table in ["atoms", "atoms_fts", "captures_fts", "processing_jobs"] {
            let n: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 0, "{table} not purged");
        }
    }
}
