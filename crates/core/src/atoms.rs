//! Atoms: derived interpretations of a capture, plus user corrections (D-019).

use anyhow::{Result, bail};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::models::{Atom, AtomStatus, AtomType, AtomView, LinkStatus, LinkView, Origin};
use crate::util::{new_id, now_iso};

pub(crate) const COLS: &str = "id, capture_id, run_id, text, type, confidence, quote, span_start, \
                               span_end, origin, status, created_at, updated_at";

pub(crate) fn from_row(r: &Row) -> rusqlite::Result<Atom> {
    Ok(Atom {
        id: r.get("id")?,
        capture_id: r.get("capture_id")?,
        run_id: r.get("run_id")?,
        text: r.get("text")?,
        atom_type: r.get("type")?,
        confidence: r.get("confidence")?,
        quote: r.get("quote")?,
        span_start: r.get("span_start")?,
        span_end: r.get("span_end")?,
        origin: r.get("origin")?,
        status: r.get("status")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

/// Prefixes every column with `a.` for joins.
pub(crate) fn cols_a() -> String {
    COLS.split(", ")
        .map(|c| format!("a.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Atom>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLS} FROM atoms WHERE id = ?1"),
            [id],
            from_row,
        )
        .optional()?)
}

pub fn active_for_capture(conn: &Connection, capture_id: &str) -> Result<Vec<Atom>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM atoms WHERE capture_id = ?1 AND status = 'active'
         ORDER BY coalesce(span_start, 1e9), created_at, id"
    ))?;
    Ok(stmt
        .query_map([capture_id], from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn links_for_atom(conn: &Connection, atom_id: &str) -> Result<Vec<LinkView>> {
    let mut stmt = conn.prepare(
        "SELECT l.atom_id, l.project_id, p.name, l.status, l.origin, l.confidence, l.reason
         FROM atom_projects l JOIN projects p ON p.id = l.project_id
         WHERE l.atom_id = ?1 AND l.status != 'rejected'
         ORDER BY l.status = 'confirmed' DESC, l.confidence DESC",
    )?;
    Ok(stmt
        .query_map([atom_id], |r| {
            Ok(LinkView {
                atom_id: r.get(0)?,
                project_id: r.get(1)?,
                project_name: r.get(2)?,
                status: r.get(3)?,
                origin: r.get(4)?,
                confidence: r.get(5)?,
                reason: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn view(conn: &Connection, atom: Atom) -> Result<AtomView> {
    let links = links_for_atom(conn, &atom.id)?;
    Ok(AtomView { atom, links })
}

pub fn views_for_capture(conn: &Connection, capture_id: &str) -> Result<Vec<AtomView>> {
    active_for_capture(conn, capture_id)?
        .into_iter()
        .map(|a| view(conn, a))
        .collect()
}

/// New atom produced by a processor run.
pub struct NewAiAtom<'a> {
    pub text: &'a str,
    pub atom_type: AtomType,
    pub confidence: f64,
    pub quote: Option<&'a str>,
}

/// Replaces the AI interpretation of a capture: previous `origin = ai`
/// atoms become `superseded`; user atoms and rejections are untouched.
pub fn replace_ai_atoms(
    conn: &Connection,
    capture_id: &str,
    capture_text: &str,
    run_id: &str,
    new_atoms: &[NewAiAtom],
) -> Result<Vec<String>> {
    let now = now_iso();
    conn.execute(
        "UPDATE atoms SET status = 'superseded', updated_at = ?2
         WHERE capture_id = ?1 AND origin = 'ai' AND status = 'active'",
        params![capture_id, now],
    )?;
    let mut ids = Vec::with_capacity(new_atoms.len());
    for a in new_atoms {
        // C6: the model gives a verbatim quote; offsets are computed locally.
        let span = a
            .quote
            .filter(|q| !q.trim().is_empty())
            .and_then(|q| capture_text.find(q).map(|b| (b, b + q.len())))
            .map(|(s, e)| {
                (
                    crate::util::utf16_offset(capture_text, s) as i64,
                    crate::util::utf16_offset(capture_text, e) as i64,
                )
            });
        let id = new_id();
        conn.execute(
            "INSERT INTO atoms (id, capture_id, run_id, text, type, confidence, quote,
                                span_start, span_end, origin, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'ai', 'active', ?10, ?10)",
            params![
                id,
                capture_id,
                run_id,
                a.text.trim(),
                a.atom_type,
                a.confidence.clamp(0.0, 1.0),
                a.quote,
                span.map(|s| s.0),
                span.map(|s| s.1),
                now
            ],
        )?;
        ids.push(id);
    }
    Ok(ids)
}

pub fn set_type(conn: &Connection, atom_id: &str, atom_type: AtomType) -> Result<()> {
    let n = conn.execute(
        "UPDATE atoms SET type = ?2, origin = 'user', updated_at = ?3 WHERE id = ?1",
        params![atom_id, atom_type, now_iso()],
    )?;
    if n == 0 {
        bail!("atom not found");
    }
    Ok(())
}

pub fn set_text(conn: &Connection, atom_id: &str, text: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("atom text cannot be empty");
    }
    let n = conn.execute(
        "UPDATE atoms SET text = ?2, origin = 'user', updated_at = ?3 WHERE id = ?1",
        params![atom_id, text.trim(), now_iso()],
    )?;
    if n == 0 {
        bail!("atom not found");
    }
    Ok(())
}

pub fn set_status(conn: &Connection, atom_id: &str, status: AtomStatus) -> Result<()> {
    conn.execute(
        "UPDATE atoms SET status = ?2, updated_at = ?3 WHERE id = ?1",
        params![atom_id, status, now_iso()],
    )?;
    Ok(())
}

/// "Add missing atom": a user-authored interpretation.
pub fn add_user_atom(
    conn: &Connection,
    capture_id: &str,
    text: &str,
    atom_type: AtomType,
) -> Result<Atom> {
    if text.trim().is_empty() {
        bail!("atom text cannot be empty");
    }
    let now = now_iso();
    let id = new_id();
    conn.execute(
        "INSERT INTO atoms (id, capture_id, text, type, origin, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'user', 'active', ?5, ?5)",
        params![id, capture_id, text.trim(), atom_type, now],
    )?;
    Ok(get(conn, &id)?.expect("just inserted"))
}

pub fn origin_is_user(atom: &Atom) -> bool {
    atom.origin == Origin::User
}

pub fn link_status(
    conn: &Connection,
    atom_id: &str,
    project_id: &str,
) -> Result<Option<LinkStatus>> {
    Ok(conn
        .query_row(
            "SELECT status FROM atom_projects WHERE atom_id = ?1 AND project_id = ?2",
            params![atom_id, project_id],
            |r| r.get(0),
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, captures};

    fn ai<'a>(text: &'a str, t: AtomType, quote: Option<&'a str>) -> NewAiAtom<'a> {
        NewAiAtom {
            text,
            atom_type: t,
            confidence: 0.8,
            quote,
        }
    }

    #[test]
    fn spans_are_computed_from_quotes() {
        let db = Db::open_in_memory().unwrap();
        let text = "😀 Fix the SSD. Maybe a horror game?";
        let c = captures::create(&mut db.conn(), text, "t").unwrap();
        let conn = db.conn();
        replace_ai_atoms(
            &conn,
            &c.id,
            text,
            "run",
            &[
                ai("Fix SSD", AtomType::Task, Some("Fix the SSD.")),
                ai("Horror game", AtomType::Spark, Some("not in text")),
            ],
        )
        .unwrap_err(); // run_id FK doesn't exist
        drop(conn);
        let conn = db.conn();
        conn.execute(
            "INSERT INTO processor_runs (id, kind, provider, model, app_version, status, started_at)
             VALUES ('run', 'analyze', 'mock', 'mock', '0', 'succeeded', 'x')",
            [],
        )
        .unwrap();
        replace_ai_atoms(
            &conn,
            &c.id,
            text,
            "run",
            &[
                ai("Fix SSD", AtomType::Task, Some("Fix the SSD.")),
                ai("Horror game", AtomType::Spark, Some("not in text")),
            ],
        )
        .unwrap();
        let atoms = active_for_capture(&conn, &c.id).unwrap();
        let fix = atoms.iter().find(|a| a.text == "Fix SSD").unwrap();
        // "😀 " is 3 UTF-16 units.
        assert_eq!((fix.span_start, fix.span_end), (Some(3), Some(15)));
        let horror = atoms.iter().find(|a| a.text == "Horror game").unwrap();
        assert_eq!(horror.span_start, None);
    }

    #[test]
    fn reprocessing_supersedes_ai_atoms_but_keeps_user_corrections() {
        let db = Db::open_in_memory().unwrap();
        let c = captures::create(&mut db.conn(), "a. b.", "t").unwrap();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO processor_runs (id, kind, provider, model, app_version, status, started_at)
             VALUES ('r1', 'analyze', 'mock', 'mock', '0', 'succeeded', 'x'),
                    ('r2', 'analyze', 'mock', 'mock', '0', 'succeeded', 'x')",
            [],
        )
        .unwrap();
        let ids = replace_ai_atoms(
            &conn,
            &c.id,
            &c.text,
            "r1",
            &[
                ai("a", AtomType::Task, None),
                ai("b", AtomType::Task, None),
                ai("c", AtomType::Task, None),
            ],
        )
        .unwrap();
        set_type(&conn, &ids[0], AtomType::Spark).unwrap(); // corrected -> user
        set_status(&conn, &ids[1], AtomStatus::Rejected).unwrap();
        add_user_atom(&conn, &c.id, "mine", AtomType::Question).unwrap();

        replace_ai_atoms(
            &conn,
            &c.id,
            &c.text,
            "r2",
            &[ai("new", AtomType::Problem, None)],
        )
        .unwrap();
        let active: Vec<_> = active_for_capture(&conn, &c.id).unwrap();
        let texts: Vec<_> = active.iter().map(|a| a.text.as_str()).collect();
        assert!(texts.contains(&"a"), "user-corrected atom must survive");
        assert!(texts.contains(&"mine"));
        assert!(texts.contains(&"new"));
        assert!(!texts.contains(&"c"), "old AI atom superseded");
        assert_eq!(
            get(&conn, &ids[1]).unwrap().unwrap().status,
            AtomStatus::Rejected
        );
        assert_eq!(
            get(&conn, &ids[2]).unwrap().unwrap().status,
            AtomStatus::Superseded
        );
    }
}
