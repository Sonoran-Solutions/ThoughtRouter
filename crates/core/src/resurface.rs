//! Deterministic resurfacing (docs/MVP_PLAN.md M4 "Score v0"). Every pick
//! records its score terms so the UI can explain it and feedback can tune it.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, params};

use crate::models::{
    Atom, AtomType, Momentum, Project, RecurringItem, RespondOutcome, ResurfaceCard,
    ResurfaceResponse, ScoreTerm,
};
use crate::settings::{Settings, embedding_model_id};
use crate::util::{Rng, iso, new_id, parse_iso};
use crate::{atoms, captures, embeddings, projects};

const CANDIDATE_TYPES: &[AtomType] = &[
    AtomType::Spark,
    AtomType::Question,
    AtomType::Research,
    AtomType::Problem,
    AtomType::Someday,
    AtomType::Project,
    AtomType::Feature,
];

struct Candidate {
    atom: Atom,
    captured_at: String,
    last_shown: Option<String>,
    last_not_now: Option<String>,
    active_project: Option<String>,
}

fn candidates(conn: &Connection, s: &Settings, now: DateTime<Utc>) -> Result<Vec<Candidate>> {
    let suppress_cutoff = iso(now - Duration::days(s.resurface_suppress_days));
    let types = CANDIDATE_TYPES
        .iter()
        .map(|t| format!("'{}'", t.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT {cols}, c.captured_at AS c_at,
           (SELECT max(shown_at) FROM resurfacing_events e WHERE e.atom_id = a.id) AS last_shown,
           (SELECT max(responded_at) FROM resurfacing_events e
              WHERE e.atom_id = a.id AND e.response = 'not_now') AS last_not_now,
           (SELECT p.name FROM atom_projects l JOIN projects p ON p.id = l.project_id
              WHERE l.atom_id = a.id AND l.status != 'rejected' AND p.momentum IN ('active', 'ready')
              LIMIT 1) AS active_project
         FROM atoms a JOIN captures c ON c.id = a.capture_id
         WHERE a.status = 'active' AND c.deleted_at IS NULL AND a.type IN ({types})
           AND NOT EXISTS (SELECT 1 FROM resurfacing_events e
                           WHERE e.atom_id = a.id AND e.response = 'dismiss')
           AND NOT EXISTS (SELECT 1 FROM resurfacing_events e
                           WHERE e.atom_id = a.id AND e.shown_at >= ?1)",
        cols = atoms::cols_a()
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([suppress_cutoff], |r| {
        Ok(Candidate {
            atom: atoms::from_row(r)?,
            captured_at: r.get("c_at")?,
            last_shown: r.get("last_shown")?,
            last_not_now: r.get("last_not_now")?,
            active_project: r.get("active_project")?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn log_scaled(x: f64, cap: f64) -> f64 {
    ((1.0 + x.max(0.0)).ln() / (1.0 + cap).ln()).min(1.0)
}

fn score(
    c: &Candidate,
    mentions: i64,
    s: &Settings,
    now: DateTime<Utc>,
    rng: &mut Rng,
) -> Vec<ScoreTerm> {
    let w = &s.resurface_weights;
    let last_touch = c
        .last_shown
        .as_deref()
        .filter(|l| *l > c.captured_at.as_str())
        .unwrap_or(&c.captured_at);
    let days = parse_iso(last_touch)
        .map(|t| (now - t).num_days().max(0))
        .unwrap_or(0);
    let mut terms = vec![
        ScoreTerm {
            name: "dormancy".into(),
            value: w.dormancy * log_scaled(days as f64, 365.0),
            explanation: match days {
                0 => "Captured today".into(),
                1 => "Untouched since yesterday".into(),
                d => format!("Untouched for {d} days"),
            },
        },
        ScoreTerm {
            name: "recurrence".into(),
            value: w.recurrence * log_scaled(mentions as f64, 10.0),
            explanation: format!("You've come back to this idea {} times", mentions + 1),
        },
        ScoreTerm {
            name: "active_project".into(),
            value: if c.active_project.is_some() {
                w.active_project
            } else {
                0.0
            },
            explanation: format!(
                "Relevant to an active project: {}",
                c.active_project.as_deref().unwrap_or("")
            ),
        },
        ScoreTerm {
            name: "open_question".into(),
            value: if matches!(c.atom.atom_type, AtomType::Question | AtomType::Research) {
                w.open_question
            } else {
                0.0
            },
            explanation: "Still an open question".into(),
        },
        ScoreTerm {
            name: "exploration".into(),
            value: w.exploration * rng.next_f64(),
            explanation: "A random pick from the vault".into(),
        },
    ];
    let not_now_cutoff = iso(now - Duration::days(s.not_now_days));
    if c.last_not_now
        .as_deref()
        .is_some_and(|t| t >= not_now_cutoff.as_str())
    {
        terms.push(ScoreTerm {
            name: "not_now".into(),
            value: -w.not_now_penalty,
            explanation: "You said \"not now\" recently".into(),
        });
    }
    terms.retain(|t| t.value != 0.0);
    terms
}

/// The explanation shown on the card: the largest positive, non-random term
/// (falls back to exploration if that's all there is).
fn reason(terms: &[ScoreTerm]) -> String {
    terms
        .iter()
        .filter(|t| t.value > 0.0 && t.name != "exploration")
        .max_by(|a, b| a.value.total_cmp(&b.value))
        .or_else(|| terms.iter().find(|t| t.name == "exploration"))
        .map(|t| t.explanation.clone())
        .unwrap_or_else(|| "From the vault".into())
}

/// Picks one thought to resurface and records that it was shown.
pub fn pick(
    conn: &Connection,
    s: &Settings,
    rng: &mut Rng,
    now: DateTime<Utc>,
) -> Result<Option<ResurfaceCard>> {
    let cands = candidates(conn, s, now)?;
    if cands.is_empty() {
        return Ok(None);
    }
    let mentions = match embedding_model_id(s) {
        Some(m) => embeddings::all_mentions(conn, &m, s.similarity_threshold)?,
        None => HashMap::new(),
    };
    let best = cands
        .into_iter()
        .map(|c| {
            let terms = score(&c, *mentions.get(&c.atom.id).unwrap_or(&0), s, now, rng);
            let total: f64 = terms.iter().map(|t| t.value).sum();
            (total, terms, c)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .expect("non-empty");
    let (total, terms, c) = best;
    let event_id = new_id();
    conn.execute(
        "INSERT INTO resurfacing_events (id, atom_id, score, reasons_json, shown_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event_id, c.atom.id, total, serde_json::to_string(&terms)?, iso(now)],
    )?;
    let capture = captures::get(conn, &c.atom.capture_id)?.expect("candidate capture exists");
    Ok(Some(ResurfaceCard {
        event_id,
        projects: atoms::links_for_atom(conn, &c.atom.id)?,
        reason: reason(&terms),
        atom: c.atom,
        capture,
        score: total,
        terms,
    }))
}

pub fn respond(
    conn: &Connection,
    event_id: &str,
    response: ResurfaceResponse,
    now: DateTime<Utc>,
) -> Result<RespondOutcome> {
    let atom_id: String = conn
        .query_row(
            "SELECT atom_id FROM resurfacing_events WHERE id = ?1",
            [event_id],
            |r| r.get(0),
        )
        .map_err(|_| anyhow::anyhow!("resurfacing event not found"))?;
    let n = conn.execute(
        "UPDATE resurfacing_events SET response = ?2, responded_at = ?3 WHERE id = ?1",
        params![event_id, response, iso(now)],
    )?;
    if n == 0 {
        bail!("resurfacing event not found");
    }
    let mut outcome = RespondOutcome {
        activated_project: None,
        needs_project: false,
    };
    if response == ResurfaceResponse::MakeActive {
        let links = atoms::links_for_atom(conn, &atom_id)?; // confirmed first
        match links.first() {
            Some(l) => {
                if l.status == crate::models::LinkStatus::Suggested {
                    projects::confirm_link(conn, &atom_id, &l.project_id)?;
                }
                projects::set_momentum(conn, &l.project_id, Momentum::Active)?;
                outcome.activated_project = projects::get(conn, &l.project_id)?;
            }
            None => outcome.needs_project = true,
        }
    }
    Ok(outcome)
}

/// "Make active" on an unlinked atom: create a project from it.
pub fn create_project_from_atom(conn: &Connection, atom_id: &str, name: &str) -> Result<Project> {
    let atom = atoms::get(conn, atom_id)?.ok_or_else(|| anyhow::anyhow!("atom not found"))?;
    let p = projects::create(conn, name, &atom.text, Momentum::Active)?;
    projects::confirm_link(conn, atom_id, &p.id)?;
    Ok(p)
}

/// "You keep coming back to this": most-mentioned ideas not already tied to
/// an active project, one representative atom per cluster.
pub fn recurring(conn: &Connection, s: &Settings, limit: usize) -> Result<Vec<RecurringItem>> {
    let Some(model) = embedding_model_id(s) else {
        return Ok(vec![]);
    };
    let mentions = embeddings::all_mentions(conn, &model, s.similarity_threshold)?;
    let mut ranked: Vec<(i64, Atom)> = Vec::new();
    for (id, n) in mentions {
        if n < 1 {
            continue;
        }
        let Some(a) = atoms::get(conn, &id)? else {
            continue;
        };
        if a.status != crate::models::AtomStatus::Active {
            continue;
        }
        let in_active: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM atom_projects l JOIN projects p ON p.id = l.project_id
              WHERE l.atom_id = ?1 AND l.status = 'confirmed' AND p.momentum IN ('active', 'ready'))",
            [&a.id],
            |r| r.get(0),
        )?;
        let dismissed: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM resurfacing_events WHERE atom_id = ?1 AND response = 'dismiss')",
            [&a.id],
            |r| r.get(0),
        )?;
        if !in_active && !dismissed {
            ranked.push((n, a));
        }
    }
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.created_at.cmp(&a.1.created_at)));
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    let mut nstmt = conn.prepare(
        "SELECT neighbor_id FROM atom_neighbors WHERE atom_id = ?1 AND model = ?2 AND similarity >= ?3",
    )?;
    for (n, a) in ranked {
        if seen.contains(&a.id) {
            continue;
        }
        let cluster: Vec<String> = nstmt
            .query_map(params![a.id, model, s.similarity_threshold], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        seen.insert(a.id.clone());
        seen.extend(cluster);
        out.push(RecurringItem {
            atom: a,
            mentions: n + 1,
        });
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;
    use crate::settings::ProviderKind;

    fn settings() -> Settings {
        Settings {
            provider: ProviderKind::Mock,
            ..Settings::default()
        }
    }

    fn atom(db: &Db, text: &str, t: AtomType) -> String {
        let c = captures::create(&mut db.conn(), text, "t").unwrap();
        atoms::add_user_atom(&db.conn(), &c.id, text, t).unwrap().id
    }

    #[test]
    fn nothing_to_resurface_on_empty_db() {
        let db = Db::open_in_memory().unwrap();
        assert!(
            pick(&db.conn(), &settings(), &mut Rng::seeded(1), Utc::now())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn prefers_dormant_thoughts_and_explains_why() {
        let db = Db::open_in_memory().unwrap();
        let s = Settings {
            resurface_weights: crate::settings::ResurfaceWeights {
                exploration: 0.0,
                ..Default::default()
            },
            ..settings()
        };
        let old = atom(&db, "old spark", AtomType::Spark);
        atom(&db, "new spark", AtomType::Spark);
        // Pretend `old` was captured 90 days ago.
        let old_capture = atoms::get(&db.conn(), &old).unwrap().unwrap().capture_id;
        db.conn()
            .execute("DROP TRIGGER captures_immutable", [])
            .unwrap();
        db.conn()
            .execute(
                "UPDATE captures SET captured_at = ?2 WHERE id = ?1",
                params![old_capture, iso(Utc::now() - Duration::days(90))],
            )
            .unwrap();
        let card = pick(&db.conn(), &s, &mut Rng::seeded(1), Utc::now())
            .unwrap()
            .unwrap();
        assert_eq!(card.atom.id, old);
        assert_eq!(card.reason, "Untouched for 90 days");
    }

    #[test]
    fn suppression_dismissal_and_tasks_are_respected() {
        let db = Db::open_in_memory().unwrap();
        let s = settings();
        let a = atom(&db, "idea", AtomType::Spark);
        atom(&db, "concrete task", AtomType::Task); // tasks are never resurfaced
        let now = Utc::now();
        let card = pick(&db.conn(), &s, &mut Rng::seeded(2), now)
            .unwrap()
            .unwrap();
        assert_eq!(card.atom.id, a);
        // Shown recently ⇒ suppressed.
        assert!(
            pick(&db.conn(), &s, &mut Rng::seeded(2), now)
                .unwrap()
                .is_none()
        );
        // After the window it can come back, unless dismissed.
        let later = now + Duration::days(s.resurface_suppress_days + 1);
        let card2 = pick(&db.conn(), &s, &mut Rng::seeded(2), later)
            .unwrap()
            .unwrap();
        respond(
            &db.conn(),
            &card2.event_id,
            ResurfaceResponse::Dismiss,
            later,
        )
        .unwrap();
        let much_later = later + Duration::days(100);
        assert!(
            pick(&db.conn(), &s, &mut Rng::seeded(2), much_later)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn not_now_is_penalised() {
        let db = Db::open_in_memory().unwrap();
        let s = settings();
        let a = atom(&db, "idea", AtomType::Spark);
        let now = Utc::now();
        let card = pick(&db.conn(), &s, &mut Rng::seeded(3), now)
            .unwrap()
            .unwrap();
        respond(&db.conn(), &card.event_id, ResurfaceResponse::NotNow, now).unwrap();
        let later = now + Duration::days(s.resurface_suppress_days + 1);
        let card = pick(&db.conn(), &s, &mut Rng::seeded(3), later)
            .unwrap()
            .unwrap();
        assert_eq!(card.atom.id, a);
        assert!(
            card.terms
                .iter()
                .any(|t| t.name == "not_now" && t.value < 0.0)
        );
    }

    #[test]
    fn make_active_activates_linked_project_or_asks_for_one() {
        let db = Db::open_in_memory().unwrap();
        let s = settings();
        let a = atom(&db, "linux control center", AtomType::Project);
        let now = Utc::now();
        let card = pick(&db.conn(), &s, &mut Rng::seeded(4), now)
            .unwrap()
            .unwrap();
        let out = respond(
            &db.conn(),
            &card.event_id,
            ResurfaceResponse::MakeActive,
            now,
        )
        .unwrap();
        assert!(out.needs_project);
        let p = create_project_from_atom(&db.conn(), &a, "Linux Control Center").unwrap();
        assert_eq!(p.momentum, Momentum::Active);

        let b = atom(&db, "save doctor idea", AtomType::Spark);
        let p2 = projects::create(&db.conn(), "Save Doctor", "", Momentum::Dormant).unwrap();
        projects::suggest_link(&db.conn(), &b, &p2.id, 0.9, "r", None).unwrap();
        let later = now + Duration::days(1);
        let card = pick(&db.conn(), &s, &mut Rng::seeded(4), later)
            .unwrap()
            .unwrap();
        assert_eq!(card.atom.id, b);
        let out = respond(
            &db.conn(),
            &card.event_id,
            ResurfaceResponse::MakeActive,
            later,
        )
        .unwrap();
        assert_eq!(out.activated_project.unwrap().momentum, Momentum::Active);
    }

    #[tokio::test]
    async fn recurring_ideas_are_detected_and_clustered() {
        use crate::processor::Processors;
        use crate::processor::mock::{MockAnalyzer, MockEmbedder};
        use std::sync::Arc;
        let db = Db::open_in_memory().unwrap();
        let p = Processors {
            analyzer: Arc::new(MockAnalyzer),
            embedder: Some(Arc::new(MockEmbedder)),
        };
        for t in [
            "Linux control center for the handheld cooler",
            "Linux control center for the handheld cooler!",
            "linux control center handheld cooler",
            "Random: horror game set in malls",
        ] {
            captures::create(&mut db.conn(), t, "t").unwrap();
        }
        crate::pipeline::run_until_idle(&db, &p).await.unwrap();
        let items = recurring(&db.conn(), &settings(), 5).unwrap();
        assert_eq!(items.len(), 1, "one cluster, shown once: {items:?}");
        assert_eq!(items[0].mentions, 3);
        assert!(items[0].atom.text.to_lowercase().contains("linux"));
    }
}
