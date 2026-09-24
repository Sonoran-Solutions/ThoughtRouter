//! Projects are user-confirmed clusters (D-004, D-017); membership is a
//! many-to-many `atom_projects` link with suggested/confirmed/rejected status.

use anyhow::{Result, bail};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::jobs::{self, JobType};
use crate::models::{
    AtomView, Momentum, Project, ProjectPage, ProjectSuggestion, ProjectSummary, SynthesisView,
    ThreadSynthesis, TimelineEntry,
};
use crate::util::{new_id, now_iso};
use crate::{atoms, captures};

const COLS: &str = "id, name, description, momentum, created_at, updated_at";

fn from_row(r: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get("id")?,
        name: r.get("name")?,
        description: r.get("description")?,
        momentum: r.get("momentum")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Project>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLS} FROM projects WHERE id = ?1"),
            [id],
            from_row,
        )
        .optional()?)
}

pub fn all(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM projects ORDER BY name COLLATE NOCASE"
    ))?;
    Ok(stmt
        .query_map([], from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT count(*) FROM projects", [], |r| r.get(0))?)
}

pub fn create(
    conn: &Connection,
    name: &str,
    description: &str,
    momentum: Momentum,
) -> Result<Project> {
    let name = name.trim();
    if name.is_empty() {
        bail!("project name cannot be empty");
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE name = ?1 COLLATE NOCASE)",
        [name],
        |r| r.get(0),
    )?;
    if exists {
        bail!("a project named \"{name}\" already exists");
    }
    let now = now_iso();
    let id = new_id();
    conn.execute(
        "INSERT INTO projects (id, name, description, momentum, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, name, description.trim(), momentum, now],
    )?;
    jobs::enqueue(conn, JobType::EmbedProject, &id)?;
    Ok(get(conn, &id)?.expect("just inserted"))
}

pub fn update(
    conn: &Connection,
    id: &str,
    name: &str,
    description: &str,
    momentum: Momentum,
) -> Result<Project> {
    let name = name.trim();
    if name.is_empty() {
        bail!("project name cannot be empty");
    }
    let before = get(conn, id)?.ok_or_else(|| anyhow::anyhow!("project not found"))?;
    conn.execute(
        "UPDATE projects SET name = ?2, description = ?3, momentum = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, name, description.trim(), momentum, now_iso()],
    )?;
    if before.name != name || before.description != description.trim() {
        jobs::enqueue(conn, JobType::EmbedProject, id)?;
    }
    Ok(get(conn, id)?.expect("exists"))
}

pub fn set_momentum(conn: &Connection, id: &str, momentum: Momentum) -> Result<()> {
    conn.execute(
        "UPDATE projects SET momentum = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, momentum, now_iso()],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM embeddings WHERE object_type = 'project' AND object_id = ?1",
        [id],
    )?;
    conn.execute("DELETE FROM processing_jobs WHERE target_id = ?1", [id])?;
    conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}

pub fn summaries(
    conn: &Connection,
    momentum_filter: Option<&[Momentum]>,
) -> Result<Vec<ProjectSummary>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS},
           (SELECT count(*) FROM atom_projects l JOIN atoms a ON a.id = l.atom_id
              JOIN captures c ON c.id = a.capture_id
            WHERE l.project_id = projects.id AND l.status = 'confirmed'
              AND a.status = 'active' AND c.deleted_at IS NULL) AS confirmed,
           (SELECT count(*) FROM atom_projects l JOIN atoms a ON a.id = l.atom_id
              JOIN captures c ON c.id = a.capture_id
            WHERE l.project_id = projects.id AND l.status = 'suggested'
              AND a.status = 'active' AND c.deleted_at IS NULL) AS suggested,
           (SELECT max(c.captured_at) FROM atom_projects l JOIN atoms a ON a.id = l.atom_id
              JOIN captures c ON c.id = a.capture_id
            WHERE l.project_id = projects.id AND l.status != 'rejected'
              AND a.status = 'active' AND c.deleted_at IS NULL) AS last_activity
         FROM projects
         ORDER BY coalesce(last_activity, updated_at) DESC"
    ))?;
    let rows = stmt.query_map([], |r| {
        Ok(ProjectSummary {
            project: from_row(r)?,
            confirmed_atoms: r.get("confirmed")?,
            suggested_atoms: r.get("suggested")?,
            last_activity: r.get("last_activity")?,
        })
    })?;
    let mut out: Vec<ProjectSummary> = rows.collect::<rusqlite::Result<_>>()?;
    if let Some(f) = momentum_filter {
        out.retain(|s| f.contains(&s.project.momentum));
    }
    Ok(out)
}

// ---- links --------------------------------------------------------------

/// AI suggestion. Never overwrites an existing row, so rejected pairs stay
/// rejected and confirmed ones stay confirmed (D-019).
pub fn suggest_link(
    conn: &Connection,
    atom_id: &str,
    project_id: &str,
    confidence: f64,
    reason: &str,
    run_id: Option<&str>,
) -> Result<bool> {
    let now = now_iso();
    let n = conn.execute(
        "INSERT INTO atom_projects (atom_id, project_id, status, origin, confidence, reason, run_id, created_at, updated_at)
         VALUES (?1, ?2, 'suggested', 'ai', ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(atom_id, project_id) DO NOTHING",
        params![atom_id, project_id, confidence.clamp(0.0, 1.0), reason, run_id, now],
    )?;
    Ok(n > 0)
}

/// User confirms (or adds) a link.
pub fn confirm_link(conn: &Connection, atom_id: &str, project_id: &str) -> Result<()> {
    let now = now_iso();
    conn.execute(
        "INSERT INTO atom_projects (atom_id, project_id, status, origin, created_at, updated_at)
         VALUES (?1, ?2, 'confirmed', 'user', ?3, ?3)
         ON CONFLICT(atom_id, project_id) DO UPDATE SET status = 'confirmed', updated_at = ?3",
        params![atom_id, project_id, now],
    )?;
    Ok(())
}

/// User rejects a link; the row is kept so it is never suggested again.
pub fn reject_link(conn: &Connection, atom_id: &str, project_id: &str) -> Result<()> {
    let now = now_iso();
    conn.execute(
        "INSERT INTO atom_projects (atom_id, project_id, status, origin, created_at, updated_at)
         VALUES (?1, ?2, 'rejected', 'user', ?3, ?3)
         ON CONFLICT(atom_id, project_id) DO UPDATE SET status = 'rejected', updated_at = ?3",
        params![atom_id, project_id, now],
    )?;
    Ok(())
}

/// Rejected (atom, project) pairs among the given atoms.
pub fn rejected_pairs(conn: &Connection, atom_ids: &[String]) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT project_id FROM atom_projects WHERE atom_id = ?1 AND status = 'rejected'",
    )?;
    for a in atom_ids {
        for p in stmt.query_map([a], |r| r.get::<_, String>(0))? {
            out.push((a.clone(), p?));
        }
    }
    Ok(out)
}

// ---- "Create project?" suggestions ---------------------------------------

pub fn add_suggestion(
    conn: &Connection,
    atom_id: &str,
    name: &str,
    description: &str,
    run_id: Option<&str>,
) -> Result<bool> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(false);
    }
    let duplicate: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE name = ?1 COLLATE NOCASE)
             OR EXISTS(SELECT 1 FROM project_suggestions
                       WHERE (atom_id = ?2 OR name = ?1 COLLATE NOCASE) AND status != 'accepted')",
        params![name, atom_id],
        |r| r.get(0),
    )?;
    if duplicate {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO project_suggestions (id, atom_id, name, description, status, run_id, created_at)
         VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6)",
        params![new_id(), atom_id, name, description.trim(), run_id, now_iso()],
    )?;
    Ok(true)
}

fn suggestions_where(
    conn: &Connection,
    filter: &str,
    arg: Option<&str>,
) -> Result<Vec<ProjectSuggestion>> {
    let sql = format!(
        "SELECT s.id, s.atom_id, s.name, s.description FROM project_suggestions s
         JOIN atoms a ON a.id = s.atom_id JOIN captures c ON c.id = a.capture_id
         WHERE s.status = 'pending' AND a.status = 'active' AND c.deleted_at IS NULL {filter}
         ORDER BY s.created_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let map = |r: &Row| {
        Ok(ProjectSuggestion {
            id: r.get(0)?,
            atom_id: r.get(1)?,
            name: r.get(2)?,
            description: r.get(3)?,
        })
    };
    let rows = match arg {
        Some(a) => stmt.query_map([a], map)?.collect::<rusqlite::Result<_>>()?,
        None => stmt.query_map([], map)?.collect::<rusqlite::Result<_>>()?,
    };
    Ok(rows)
}

pub fn pending_suggestions(conn: &Connection) -> Result<Vec<ProjectSuggestion>> {
    suggestions_where(conn, "", None)
}

pub fn pending_suggestions_for_capture(
    conn: &Connection,
    capture_id: &str,
) -> Result<Vec<ProjectSuggestion>> {
    suggestions_where(conn, "AND a.capture_id = ?1", Some(capture_id))
}

/// Accepting creates the project and a confirmed link from the atom.
pub fn accept_suggestion(
    conn: &Connection,
    suggestion_id: &str,
    name: Option<&str>,
) -> Result<Project> {
    let (atom_id, s_name, desc): (String, String, String) = conn.query_row(
        "SELECT atom_id, name, description FROM project_suggestions WHERE id = ?1 AND status = 'pending'",
        [suggestion_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let project = create(conn, name.unwrap_or(&s_name), &desc, Momentum::Exploring)?;
    confirm_link(conn, &atom_id, &project.id)?;
    conn.execute(
        "UPDATE project_suggestions SET status = 'accepted' WHERE id = ?1",
        [suggestion_id],
    )?;
    Ok(project)
}

pub fn dismiss_suggestion(conn: &Connection, suggestion_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE project_suggestions SET status = 'dismissed' WHERE id = ?1",
        [suggestion_id],
    )?;
    Ok(())
}

// ---- project page ---------------------------------------------------------

/// Linked captures, newest first, with the atoms that link them.
pub fn timeline(conn: &Connection, project_id: &str) -> Result<Vec<TimelineEntry>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT a.capture_id, c.captured_at FROM atom_projects l
         JOIN atoms a ON a.id = l.atom_id JOIN captures c ON c.id = a.capture_id
         WHERE l.project_id = ?1 AND l.status != 'rejected' AND a.status = 'active'
           AND c.deleted_at IS NULL
         ORDER BY c.captured_at DESC",
    )?;
    let capture_ids: Vec<String> = stmt
        .query_map([project_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::with_capacity(capture_ids.len());
    for cid in capture_ids {
        let Some(capture) = captures::get(conn, &cid)? else {
            continue;
        };
        let atoms: Vec<AtomView> = atoms::views_for_capture(conn, &cid)?
            .into_iter()
            .filter(|v| v.links.iter().any(|l| l.project_id == project_id))
            .collect();
        out.push(TimelineEntry { capture, atoms });
    }
    Ok(out)
}

pub fn store_synthesis(
    conn: &Connection,
    project_id: &str,
    run_id: &str,
    body: &ThreadSynthesis,
    source_capture_ids: &[String],
    based_on_through: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO project_syntheses (id, project_id, run_id, body_json, source_capture_ids, based_on_through, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            new_id(),
            project_id,
            run_id,
            serde_json::to_string(body)?,
            serde_json::to_string(source_capture_ids)?,
            based_on_through,
            now_iso()
        ],
    )?;
    Ok(())
}

pub fn latest_synthesis(
    conn: &Connection,
    project_id: &str,
    latest_capture_at: Option<&str>,
) -> Result<Option<SynthesisView>> {
    let row: Option<(String, String, String, String, Option<String>)> = conn
        .query_row(
            "SELECT s.body_json, s.source_capture_ids, s.based_on_through, s.created_at, r.model
             FROM project_syntheses s LEFT JOIN processor_runs r ON r.id = s.run_id
             WHERE s.project_id = ?1 ORDER BY s.created_at DESC LIMIT 1",
            [project_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((body, sources, through, created_at, model)) = row else {
        return Ok(None);
    };
    Ok(Some(SynthesisView {
        body: serde_json::from_str(&body)?,
        source_capture_ids: serde_json::from_str(&sources)?,
        model,
        created_at,
        stale: latest_capture_at.is_some_and(|l| l > through.as_str()),
    }))
}

pub fn page(conn: &Connection, project_id: &str) -> Result<ProjectPage> {
    let project = get(conn, project_id)?.ok_or_else(|| anyhow::anyhow!("project not found"))?;
    let timeline = timeline(conn, project_id)?;
    let latest = timeline.first().map(|e| e.capture.captured_at.clone());
    let synthesis = latest_synthesis(conn, project_id, latest.as_deref())?;
    Ok(ProjectPage {
        project,
        timeline,
        synthesis,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;
    use crate::models::{AtomType, LinkStatus};

    fn setup() -> (Db, String, String) {
        let db = Db::open_in_memory().unwrap();
        let c = captures::create(&mut db.conn(), "Save Doctor and Pokémon Unbound", "t").unwrap();
        let a = atoms::add_user_atom(&db.conn(), &c.id, "shared RE", AtomType::Research).unwrap();
        (db, c.id, a.id)
    }

    #[test]
    fn one_atom_links_to_many_projects() {
        let (db, _, atom) = setup();
        let conn = db.conn();
        let p1 = create(&conn, "Save Doctor", "", Momentum::Active).unwrap();
        let p2 = create(&conn, "Pokémon Unbound Completion", "", Momentum::Exploring).unwrap();
        suggest_link(&conn, &atom, &p1.id, 0.9, "r", None).unwrap();
        confirm_link(&conn, &atom, &p2.id).unwrap();
        let links = atoms::links_for_atom(&conn, &atom).unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(timeline(&conn, &p1.id).unwrap().len(), 1);
        assert_eq!(timeline(&conn, &p2.id).unwrap().len(), 1);
    }

    #[test]
    fn rejected_links_are_never_resuggested() {
        let (db, _, atom) = setup();
        let conn = db.conn();
        let p = create(&conn, "Save Doctor", "", Momentum::Active).unwrap();
        suggest_link(&conn, &atom, &p.id, 0.9, "r", None).unwrap();
        reject_link(&conn, &atom, &p.id).unwrap();
        assert!(!suggest_link(&conn, &atom, &p.id, 0.99, "again", None).unwrap());
        assert_eq!(
            atoms::link_status(&conn, &atom, &p.id).unwrap(),
            Some(LinkStatus::Rejected)
        );
        assert!(atoms::links_for_atom(&conn, &atom).unwrap().is_empty());
        assert_eq!(
            rejected_pairs(&conn, std::slice::from_ref(&atom)).unwrap(),
            vec![(atom, p.id)]
        );
    }

    #[test]
    fn suggestions_are_deduped_and_accept_creates_confirmed_link() {
        let (db, _, atom) = setup();
        let conn = db.conn();
        assert!(add_suggestion(&conn, &atom, "Horror Game", "malls", None).unwrap());
        assert!(!add_suggestion(&conn, &atom, "horror game", "", None).unwrap());
        let s = pending_suggestions(&conn).unwrap();
        assert_eq!(s.len(), 1);
        let p = accept_suggestion(&conn, &s[0].id, None).unwrap();
        assert_eq!(
            atoms::link_status(&conn, &atom, &p.id).unwrap(),
            Some(LinkStatus::Confirmed)
        );
        assert!(pending_suggestions(&conn).unwrap().is_empty());
        assert!(
            create(&conn, "HORROR GAME", "", Momentum::Spark).is_err(),
            "names are unique"
        );
    }
}
