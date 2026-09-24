//! Derived processing stages (docs/AI_PIPELINE.md). Each stage is a job;
//! the DB lock is never held across a network call.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use rusqlite::{Connection, params};

use crate::embeddings;
use crate::jobs::{self, Job, JobType};
use crate::models::{AtomType, SynthesisView};
use crate::processor::{
    AnalyzeInput, LinkAtom, LinkCandidate, LinkInput, MAX_ATOMS_PER_CAPTURE, Processors, RunMeta,
    SynthesisEntry, SynthesisInput,
};
use crate::util::{new_id, now_iso};
use crate::{APP_VERSION, Db, atoms, captures, projects};

/// AI link suggestions below this confidence are dropped.
pub const LINK_MIN_CONFIDENCE: f64 = 0.5;
/// Above this many projects, only the most similar are sent as candidates.
pub const MAX_LINK_CANDIDATES: usize = 12;
/// When a project is created or renamed, at most this many existing
/// captures are queued for (re)linking against it.
pub const BACKFILL_MAX_CAPTURES: usize = 15;
const BACKFILL_MIN_SIMILARITY: f32 = 0.35;
/// Synthesis looks at this many most-recent linked captures.
pub const SYNTHESIS_MAX_CAPTURES: usize = 40;

pub fn record_run(
    conn: &Connection,
    capture_id: Option<&str>,
    kind: &str,
    meta: &RunMeta,
    error: Option<&str>,
    started_at: &str,
) -> Result<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO processor_runs (id, capture_id, kind, provider, model, prompt_version, schema_version,
                                     app_version, status, error, started_at, finished_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            capture_id,
            kind,
            meta.provider,
            meta.model,
            meta.prompt_version,
            meta.schema_version,
            APP_VERSION,
            if error.is_some() { "failed" } else { "succeeded" },
            error,
            started_at,
            now_iso()
        ],
    )?;
    Ok(id)
}

fn failed_meta(p: &Processors, prompt: &str) -> RunMeta {
    RunMeta {
        provider: p.analyzer.provider().to_string(),
        model: "unknown".into(),
        prompt_version: Some(prompt.into()),
        schema_version: None,
    }
}

pub async fn run_job(db: &Db, p: &Processors, job: &Job) -> Result<()> {
    match job.job_type {
        JobType::AnalyzeCapture => analyze_capture(db, p, &job.target_id).await,
        JobType::EmbedCapture => embed_capture(db, p, &job.target_id).await,
        JobType::LinkProjects => link_projects(db, p, &job.target_id).await,
        JobType::EmbedProject => embed_project(db, p, &job.target_id).await,
    }
}

async fn analyze_capture(db: &Db, p: &Processors, capture_id: &str) -> Result<()> {
    let (capture, known_projects) = {
        let conn = db.conn();
        let names = projects::all(&conn)?.into_iter().map(|p| p.name).collect();
        (captures::get_live(&conn, capture_id)?, names)
    };
    let Some(capture) = capture else {
        return Ok(()); // trashed or purged meanwhile
    };
    let started = now_iso();
    let input = AnalyzeInput {
        text: capture.text.clone(),
        known_projects,
    };
    let result = p.analyzer.analyze_capture(&input).await;
    let mut conn = db.conn();
    let (analysis, meta) = match result {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{e:#}");
            let meta = failed_meta(p, crate::processor::ANALYZE_PROMPT_VERSION);
            record_run(
                &conn,
                Some(capture_id),
                "analyze",
                &meta,
                Some(&msg),
                &started,
            )?;
            return Err(e);
        }
    };
    let tx = conn.transaction()?;
    let run_id = record_run(&tx, Some(capture_id), "analyze", &meta, None, &started)?;
    let new_atoms: Vec<atoms::NewAiAtom> = analysis
        .atoms
        .iter()
        .filter(|a| !a.text.trim().is_empty())
        .take(MAX_ATOMS_PER_CAPTURE)
        .map(|a| atoms::NewAiAtom {
            text: &a.text,
            atom_type: a.atom_type,
            confidence: a.confidence,
            quote: a.quote.as_deref(),
        })
        .collect();
    atoms::replace_ai_atoms(&tx, capture_id, &capture.text, &run_id, &new_atoms)?;
    jobs::enqueue(&tx, JobType::EmbedCapture, capture_id)?;
    tx.commit()?;
    Ok(())
}

async fn embed_capture(db: &Db, p: &Processors, capture_id: &str) -> Result<()> {
    let has_projects;
    if let Some(embedder) = &p.embedder {
        let model = embedder.model_id();
        // (object_type, id, text) still missing a vector for this model.
        let todo: Vec<(&'static str, String, String)> = {
            let conn = db.conn();
            let Some(capture) = captures::get_live(&conn, capture_id)? else {
                return Ok(());
            };
            let mut todo = Vec::new();
            if embeddings::get(&conn, "capture", capture_id, &model)?.is_none() {
                todo.push(("capture", capture.id.clone(), capture.text.clone()));
            }
            for a in atoms::active_for_capture(&conn, capture_id)? {
                if embeddings::get(&conn, "atom", &a.id, &model)?.is_none() {
                    todo.push(("atom", a.id, a.text));
                }
            }
            todo
        };
        if !todo.is_empty() {
            let texts: Vec<String> = todo.iter().map(|t| t.2.clone()).collect();
            let vectors = embedder.embed(&texts).await?;
            if vectors.len() != todo.len() {
                return Err(anyhow!(
                    "embedder returned {} vectors for {} texts",
                    vectors.len(),
                    todo.len()
                ));
            }
            let mut conn = db.conn();
            let tx = conn.transaction()?;
            for ((kind, id, _), v) in todo.iter().zip(&vectors) {
                embeddings::store(&tx, kind, id, &model, v)?;
            }
            for (kind, id, _) in &todo {
                if *kind == "atom" {
                    embeddings::update_neighbors(&tx, id, &model)?;
                }
            }
            tx.commit()?;
        }
    }
    {
        let conn = db.conn();
        has_projects = projects::count(&conn)? > 0;
        if has_projects {
            jobs::enqueue(&conn, JobType::LinkProjects, capture_id)?;
        }
    }
    Ok(())
}

fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect()
}

async fn link_projects(db: &Db, p: &Processors, capture_id: &str) -> Result<()> {
    let prepared = {
        let conn = db.conn();
        if captures::get_live(&conn, capture_id)?.is_none() {
            return Ok(());
        }
        let atom_list = atoms::active_for_capture(&conn, capture_id)?;
        let all_projects = projects::all(&conn)?;
        if atom_list.is_empty() || all_projects.is_empty() {
            return Ok(());
        }
        // Candidate retrieval: cheap similarity + lexical, never the whole DB.
        let candidates = if all_projects.len() <= MAX_LINK_CANDIDATES {
            all_projects
        } else {
            let model = p.embedder.as_ref().map(|e| e.model_id());
            let project_vecs: HashMap<String, Vec<f32>> = match &model {
                Some(m) => embeddings::projects(&conn, m)?.into_iter().collect(),
                None => HashMap::new(),
            };
            let atom_vecs: Vec<Vec<f32>> = match &model {
                Some(m) => atom_list
                    .iter()
                    .filter_map(|a| embeddings::get(&conn, "atom", &a.id, m).ok().flatten())
                    .collect(),
                None => vec![],
            };
            let atom_tokens: Vec<String> = atom_list.iter().flat_map(|a| tokens(&a.text)).collect();
            let mut scored: Vec<(f64, crate::models::Project)> = all_projects
                .into_iter()
                .map(|proj| {
                    let sim = project_vecs
                        .get(&proj.id)
                        .map(|pv| {
                            atom_vecs
                                .iter()
                                .map(|av| embeddings::dot(av, pv) as f64)
                                .fold(0.0, f64::max)
                        })
                        .unwrap_or(0.0);
                    let lexical = tokens(&proj.name).iter().any(|t| atom_tokens.contains(t));
                    (sim + if lexical { 1.0 } else { 0.0 }, proj)
                })
                .collect();
            scored.sort_by(|a, b| b.0.total_cmp(&a.0));
            scored
                .into_iter()
                .take(MAX_LINK_CANDIDATES)
                .map(|s| s.1)
                .collect()
        };
        let atom_keys: Vec<(String, String)> = atom_list
            .iter()
            .enumerate()
            .map(|(i, a)| (format!("a{}", i + 1), a.id.clone()))
            .collect();
        let proj_keys: Vec<(String, String)> = candidates
            .iter()
            .enumerate()
            .map(|(i, p)| (format!("p{}", i + 1), p.id.clone()))
            .collect();
        let atom_id_to_key: HashMap<&str, &str> = atom_keys
            .iter()
            .map(|(k, id)| (id.as_str(), k.as_str()))
            .collect();
        let proj_id_to_key: HashMap<&str, &str> = proj_keys
            .iter()
            .map(|(k, id)| (id.as_str(), k.as_str()))
            .collect();
        let rejected = projects::rejected_pairs(
            &conn,
            &atom_list.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
        )?
        .into_iter()
        .filter_map(|(a, pr)| {
            Some((
                atom_id_to_key.get(a.as_str())?.to_string(),
                proj_id_to_key.get(pr.as_str())?.to_string(),
            ))
        })
        .collect();
        let input = LinkInput {
            atoms: atom_list
                .iter()
                .zip(&atom_keys)
                .map(|(a, (k, _))| LinkAtom {
                    key: k.clone(),
                    text: a.text.clone(),
                    atom_type: a.atom_type,
                })
                .collect(),
            candidates: candidates
                .iter()
                .zip(&proj_keys)
                .map(|(pr, (k, _))| LinkCandidate {
                    key: k.clone(),
                    name: pr.name.clone(),
                    description: pr.description.clone(),
                })
                .collect(),
            rejected,
        };
        (input, atom_keys, proj_keys, atom_list)
    };
    let (input, atom_keys, proj_keys, atom_list) = prepared;
    let started = now_iso();
    let result = p.analyzer.link_projects(&input).await;
    let mut conn = db.conn();
    let (out, meta) = match result {
        Ok(r) => r,
        Err(e) => {
            let meta = failed_meta(p, crate::processor::LINK_PROMPT_VERSION);
            record_run(
                &conn,
                Some(capture_id),
                "link",
                &meta,
                Some(&format!("{e:#}")),
                &started,
            )?;
            return Err(e);
        }
    };
    let atom_map: HashMap<String, String> = atom_keys.into_iter().collect();
    let proj_map: HashMap<String, String> = proj_keys.into_iter().collect();
    let tx = conn.transaction()?;
    let run_id = record_run(&tx, Some(capture_id), "link", &meta, None, &started)?;
    for l in &out.links {
        if l.confidence < LINK_MIN_CONFIDENCE {
            continue;
        }
        if let (Some(a), Some(pr)) = (atom_map.get(&l.atom), proj_map.get(&l.project)) {
            projects::suggest_link(&tx, a, pr, l.confidence, &l.reason, Some(&run_id))?;
        }
    }
    for s in &out.new_projects {
        let Some(a) = atom_map.get(&s.atom) else {
            continue;
        };
        // D-017 guard: only project-sized atoms may propose a new project.
        let ok_type = atom_list.iter().any(|x| {
            &x.id == a
                && matches!(
                    x.atom_type,
                    AtomType::Project | AtomType::Feature | AtomType::Spark
                )
        });
        if ok_type {
            projects::add_suggestion(&tx, a, &s.name, &s.description, Some(&run_id))?;
        }
    }
    tx.commit()?;
    Ok(())
}

async fn embed_project(db: &Db, p: &Processors, project_id: &str) -> Result<()> {
    let Some(pr) = projects::get(&db.conn(), project_id)? else {
        return Ok(());
    };
    let mut vector = None;
    if let Some(embedder) = &p.embedder {
        let v = embedder
            .embed(&[format!("{}. {}", pr.name, pr.description)])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no embedding returned"))?;
        embeddings::store(&db.conn(), "project", project_id, &embedder.model_id(), &v)?;
        vector = embeddings::get(&db.conn(), "project", project_id, &embedder.model_id())?
            .map(|v| (embedder.model_id(), v));
    }
    // Backfill: captures processed before this project existed would never
    // be linked to it, so queue linking for the most related ones.
    let conn = db.conn();
    for capture_id in backfill_candidates(&conn, &pr.name, vector.as_ref())? {
        jobs::enqueue(&conn, JobType::LinkProjects, &capture_id)?;
    }
    Ok(())
}

/// Captures most likely related to a project: lexical hits on its name plus
/// (when available) the nearest atoms to its embedding.
fn backfill_candidates(
    conn: &Connection,
    name: &str,
    vector: Option<&(String, Vec<f32>)>,
) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    if let Some(q) = crate::search::fts_query(name) {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT a.capture_id FROM atoms_fts f JOIN atoms a ON a.id = f.id
             JOIN captures c ON c.id = a.capture_id
             WHERE atoms_fts MATCH ?1 AND a.status = 'active' AND c.deleted_at IS NULL
             ORDER BY bm25(atoms_fts) LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![q, BACKFILL_MAX_CAPTURES as i64], |r| {
            r.get::<_, String>(0)
        })?;
        for r in rows {
            let id = r?;
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    if let Some((model, pv)) = vector {
        let mut scored: Vec<(f32, String)> = embeddings::live_atoms(conn, model)?
            .into_iter()
            .map(|(_, cap, v)| (embeddings::dot(pv, &v), cap))
            .filter(|(s, _)| *s >= BACKFILL_MIN_SIMILARITY)
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, cap) in scored {
            if !out.contains(&cap) {
                out.push(cap);
            }
        }
    }
    out.truncate(BACKFILL_MAX_CAPTURES);
    Ok(out)
}

/// On-demand "Summarize" for a project page (not a job: the user waits).
pub async fn synthesize_project(
    db: &Db,
    p: &Processors,
    project_id: &str,
) -> Result<SynthesisView> {
    let (input, sources, through) = {
        let conn = db.conn();
        let project =
            projects::get(&conn, project_id)?.ok_or_else(|| anyhow!("project not found"))?;
        let mut timeline = projects::timeline(&conn, project_id)?; // newest first
        if timeline.is_empty() {
            return Err(anyhow!(
                "this project has no linked thoughts to summarize yet"
            ));
        }
        timeline.truncate(SYNTHESIS_MAX_CAPTURES);
        timeline.reverse();
        let through = timeline
            .last()
            .expect("non-empty")
            .capture
            .captured_at
            .clone();
        let sources: Vec<String> = timeline.iter().map(|e| e.capture.id.clone()).collect();
        let input = SynthesisInput {
            project_name: project.name,
            project_description: project.description,
            entries: timeline
                .into_iter()
                .map(|e| SynthesisEntry {
                    captured_at: e.capture.captured_at,
                    text: e.capture.text,
                })
                .collect(),
        };
        (input, sources, through)
    };
    let started = now_iso();
    let result = p.analyzer.synthesize(&input).await;
    let conn = db.conn();
    let (body, meta) = match result {
        Ok(r) => r,
        Err(e) => {
            let meta = failed_meta(p, crate::processor::SYNTHESIZE_PROMPT_VERSION);
            record_run(
                &conn,
                None,
                "synthesize",
                &meta,
                Some(&format!("{e:#}")),
                &started,
            )?;
            return Err(e);
        }
    };
    let run_id = record_run(&conn, None, "synthesize", &meta, None, &started)?;
    projects::store_synthesis(&conn, project_id, &run_id, &body, &sources, &through)?;
    projects::latest_synthesis(&conn, project_id, Some(&through))?
        .ok_or_else(|| anyhow!("synthesis not stored"))
}

/// Re-runs analysis for one capture (user's "Reprocess").
pub fn reprocess(conn: &Connection, capture_id: &str) -> Result<()> {
    jobs::enqueue(conn, JobType::AnalyzeCapture, capture_id)
}

/// Queues analysis for every live capture ("Reprocess all").
pub fn reprocess_all(conn: &Connection) -> Result<usize> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM captures WHERE deleted_at IS NULL")?;
        stmt.query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for id in &ids {
        jobs::enqueue(conn, JobType::AnalyzeCapture, id)?;
    }
    Ok(ids.len())
}

/// After an embedding-model change: embed everything under the new model.
pub fn reembed_all(conn: &Connection) -> Result<()> {
    let caps: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM captures WHERE deleted_at IS NULL")?;
        stmt.query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for id in &caps {
        jobs::enqueue(conn, JobType::EmbedCapture, id)?;
    }
    for pr in projects::all(conn)? {
        jobs::enqueue(conn, JobType::EmbedProject, &pr.id)?;
    }
    Ok(())
}

/// Runs queued jobs until none are runnable. Used by tests and the eval script.
pub async fn run_until_idle(db: &Db, p: &Processors) -> Result<usize> {
    let mut n = 0;
    loop {
        let job = jobs::claim_next(&db.conn())?;
        let Some(job) = job else { return Ok(n) };
        let res = run_job(db, p, &job).await;
        let conn = db.conn();
        match res {
            Ok(()) => jobs::complete(&conn, &job.id)?,
            Err(e) => jobs::fail(&conn, &job, &format!("{e:#}"))?,
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::models::{Momentum, ProcessingState};
    use crate::processor::mock::{MockAnalyzer, MockEmbedder};

    fn mock() -> Processors {
        Processors {
            analyzer: Arc::new(MockAnalyzer),
            embedder: Some(Arc::new(MockEmbedder)),
        }
    }

    #[tokio::test]
    async fn full_pipeline_with_mock() {
        let db = Db::open_in_memory().unwrap();
        let p = mock();
        let proj = projects::create(
            &db.conn(),
            "Save Doctor",
            "save corruption tooling",
            Momentum::Active,
        )
        .unwrap();
        let raw = "Need to figure out the SSD disconnect. Save Doctor could detect save corruption automatically.";
        let c = captures::create(&mut db.conn(), raw, "t").unwrap();
        run_until_idle(&db, &p).await.unwrap();

        let conn = db.conn();
        assert_eq!(
            captures::get(&conn, &c.id).unwrap().unwrap().text,
            raw,
            "raw text unchanged"
        );
        let view = captures::view(&conn, captures::get(&conn, &c.id).unwrap().unwrap()).unwrap();
        assert_eq!(view.state, ProcessingState::Processed);
        assert_eq!(view.atoms.len(), 2);
        let linked: Vec<_> = view.atoms.iter().filter(|a| !a.links.is_empty()).collect();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].links[0].project_id, proj.id);
        assert!(
            embeddings::get(&conn, "capture", &c.id, "mock/hash-256")
                .unwrap()
                .is_some()
        );
        let runs: i64 = conn
            .query_row(
                "SELECT count(*) FROM processor_runs WHERE status = 'succeeded'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(runs, 2, "analyze + link runs recorded");
    }

    #[tokio::test]
    async fn processing_failure_leaves_capture_intact_and_retryable() {
        use crate::processor::{Analyzer, CaptureAnalysis, LinkOutput};
        struct Failing;
        #[async_trait::async_trait]
        impl Analyzer for Failing {
            fn provider(&self) -> &str {
                "failing"
            }
            async fn analyze_capture(
                &self,
                _: &AnalyzeInput,
            ) -> Result<(CaptureAnalysis, RunMeta)> {
                Err(anyhow!("model outage"))
            }
            async fn link_projects(&self, _: &LinkInput) -> Result<(LinkOutput, RunMeta)> {
                unreachable!()
            }
            async fn synthesize(
                &self,
                _: &SynthesisInput,
            ) -> Result<(crate::models::ThreadSynthesis, RunMeta)> {
                unreachable!()
            }
        }
        let db = Db::open_in_memory().unwrap();
        let c = captures::create(&mut db.conn(), "keep me", "t").unwrap();
        let failing = Processors {
            analyzer: Arc::new(Failing),
            embedder: None,
        };
        run_until_idle(&db, &failing).await.unwrap();
        {
            let conn = db.conn();
            let v = captures::view(&conn, captures::get(&conn, &c.id).unwrap().unwrap()).unwrap();
            assert_eq!(v.capture.text, "keep me");
            assert_eq!(
                v.state,
                ProcessingState::Saved,
                "backing off, not failed capture"
            );
            assert!(v.last_error.unwrap().contains("model outage"));
            // Make it runnable now and recover with a working processor.
            jobs::wake_all(&conn).unwrap();
        }
        run_until_idle(&db, &mock()).await.unwrap();
        let conn = db.conn();
        let v = captures::view(&conn, captures::get(&conn, &c.id).unwrap().unwrap()).unwrap();
        assert_eq!(v.state, ProcessingState::Processed);
    }

    #[tokio::test]
    async fn new_project_backfills_links_for_older_captures() {
        let db = Db::open_in_memory().unwrap();
        let p = mock();
        let c =
            captures::create(&mut db.conn(), "Save Doctor could detect corruption.", "t").unwrap();
        captures::create(&mut db.conn(), "Unrelated horror game idea.", "t").unwrap();
        run_until_idle(&db, &p).await.unwrap();
        projects::create(&db.conn(), "Save Doctor", "", Momentum::Active).unwrap();
        run_until_idle(&db, &p).await.unwrap();
        let conn = db.conn();
        let v = captures::view(&conn, captures::get(&conn, &c.id).unwrap().unwrap()).unwrap();
        assert_eq!(
            v.atoms[0].links.len(),
            1,
            "older capture linked after project creation"
        );
    }

    #[tokio::test]
    async fn synthesize_records_provenance_and_staleness() {
        let db = Db::open_in_memory().unwrap();
        let p = mock();
        let proj =
            projects::create(&db.conn(), "Linux Control Center", "", Momentum::Exploring).unwrap();
        captures::create(
            &mut db.conn(),
            "Want a Linux control center. What about the cooler protocol?",
            "t",
        )
        .unwrap();
        run_until_idle(&db, &p).await.unwrap();
        let s = synthesize_project(&db, &p, &proj.id).await.unwrap();
        assert!(!s.stale);
        assert_eq!(s.source_capture_ids.len(), 1);
        captures::create(
            &mut db.conn(),
            "The Linux control center cooler needs RE.",
            "t",
        )
        .unwrap();
        run_until_idle(&db, &p).await.unwrap();
        let page = projects::page(&db.conn(), &proj.id).unwrap();
        assert!(page.synthesis.unwrap().stale);
    }
}
