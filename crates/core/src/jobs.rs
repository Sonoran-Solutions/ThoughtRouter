//! Local, resumable processing queue (docs/ARCHITECTURE.md "Background job model").

use anyhow::Result;
use chrono::{Duration, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::models::ProcessingState;
use crate::util::{iso, new_id, now_iso};

pub const MAX_ATTEMPTS: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobType {
    AnalyzeCapture,
    EmbedCapture,
    LinkProjects,
    EmbedProject,
}

impl JobType {
    pub fn as_str(self) -> &'static str {
        match self {
            JobType::AnalyzeCapture => "analyze_capture",
            JobType::EmbedCapture => "embed_capture",
            JobType::LinkProjects => "link_projects",
            JobType::EmbedProject => "embed_project",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "analyze_capture" => JobType::AnalyzeCapture,
            "embed_capture" => JobType::EmbedCapture,
            "link_projects" => JobType::LinkProjects,
            "embed_project" => JobType::EmbedProject,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub job_type: JobType,
    pub target_id: String,
    pub attempts: i64,
}

/// Queues a job unless an identical one is already queued or running.
pub fn enqueue(conn: &Connection, job_type: JobType, target_id: &str) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM processing_jobs
          WHERE job_type = ?1 AND target_id = ?2 AND status IN ('queued', 'running'))",
        params![job_type.as_str(), target_id],
        |r| r.get(0),
    )?;
    if !exists {
        let now = now_iso();
        conn.execute(
            "INSERT INTO processing_jobs (id, job_type, target_id, status, run_after, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'queued', ?4, ?4, ?4)",
            params![new_id(), job_type.as_str(), target_id, now],
        )?;
    }
    Ok(())
}

/// Atomically claims the oldest runnable job.
pub fn claim_next(conn: &Connection) -> Result<Option<Job>> {
    let now = now_iso();
    let job = conn
        .query_row(
            "UPDATE processing_jobs SET status = 'running', attempts = attempts + 1, updated_at = ?1
             WHERE id = (SELECT id FROM processing_jobs
                         WHERE status = 'queued' AND run_after <= ?1
                         ORDER BY created_at, id LIMIT 1)
             RETURNING id, job_type, target_id, attempts",
            [&now],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;
    Ok(job.and_then(|(id, t, target_id, attempts)| {
        JobType::parse(&t).map(|job_type| Job {
            id,
            job_type,
            target_id,
            attempts,
        })
    }))
}

pub fn complete(conn: &Connection, job_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE processing_jobs SET status = 'done', last_error = NULL, updated_at = ?2 WHERE id = ?1",
        params![job_id, now_iso()],
    )?;
    Ok(())
}

/// Records a failure; re-queues with exponential backoff until
/// [`MAX_ATTEMPTS`], then parks the job as `failed` for manual retry.
pub fn fail(conn: &Connection, job: &Job, error: &str) -> Result<()> {
    if job.attempts >= MAX_ATTEMPTS {
        conn.execute(
            "UPDATE processing_jobs SET status = 'failed', last_error = ?2, updated_at = ?3 WHERE id = ?1",
            params![job.id, error, now_iso()],
        )?;
    } else {
        let delay = Duration::seconds(5 * 2i64.pow(job.attempts as u32));
        conn.execute(
            "UPDATE processing_jobs SET status = 'queued', last_error = ?2, run_after = ?3, updated_at = ?4
             WHERE id = ?1",
            params![job.id, error, iso(Utc::now() + delay), now_iso()],
        )?;
    }
    Ok(())
}

/// Returns a claimed job to the queue without counting an attempt (used
/// when processing becomes unavailable mid-flight).
pub fn release(conn: &Connection, job: &Job) -> Result<()> {
    conn.execute(
        "UPDATE processing_jobs SET status = 'queued', attempts = max(attempts - 1, 0), updated_at = ?2
         WHERE id = ?1",
        params![job.id, now_iso()],
    )?;
    Ok(())
}

/// On startup: anything left `running` was interrupted.
pub fn reset_running(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE processing_jobs SET status = 'queued', updated_at = ?1 WHERE status = 'running'",
        [now_iso()],
    )?)
}

/// Manual retry: re-queues failed jobs for a capture (or all), resetting attempts.
pub fn retry_failed(conn: &Connection, target_id: Option<&str>) -> Result<usize> {
    let now = now_iso();
    Ok(match target_id {
        Some(t) => conn.execute(
            "UPDATE processing_jobs SET status = 'queued', attempts = 0, run_after = ?2, updated_at = ?2
             WHERE status = 'failed' AND target_id = ?1",
            params![t, now],
        )?,
        None => conn.execute(
            "UPDATE processing_jobs SET status = 'queued', attempts = 0, run_after = ?1, updated_at = ?1
             WHERE status = 'failed'",
            [now],
        )?,
    })
}

/// Makes every queued job runnable now (e.g. after the user adds an API key).
pub fn wake_all(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE processing_jobs SET run_after = ?1 WHERE status = 'queued'",
        [now_iso()],
    )?;
    Ok(())
}

pub fn counts(conn: &Connection) -> Result<(i64, i64, i64)> {
    Ok(conn.query_row(
        "SELECT
           coalesce(sum(status = 'queued'), 0),
           coalesce(sum(status = 'running'), 0),
           coalesce(sum(status = 'failed'), 0)
         FROM processing_jobs",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?)
}

/// Derives the UX processing state for a capture from its jobs.
pub fn capture_state(
    conn: &Connection,
    capture_id: &str,
) -> Result<(ProcessingState, Option<String>)> {
    let mut stmt = conn.prepare(
        "SELECT job_type, status, last_error FROM processing_jobs
         WHERE target_id = ?1 ORDER BY updated_at DESC",
    )?;
    let rows: Vec<(String, String, Option<String>)> = stmt
        .query_map([capture_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let analyzed = rows
        .iter()
        .any(|(t, s, _)| t == "analyze_capture" && s == "done");
    if rows.iter().any(|(_, s, _)| s == "running") {
        return Ok((ProcessingState::Analyzing, None));
    }
    if let Some((_, _, e)) = rows.iter().find(|(_, s, _)| s == "failed") {
        return Ok((ProcessingState::NeedsRetry, e.clone()));
    }
    if let Some((_, _, e)) = rows.iter().find(|(_, s, _)| s == "queued") {
        let state = if analyzed {
            ProcessingState::Analyzing
        } else {
            ProcessingState::Saved
        };
        return Ok((state, e.clone()));
    }
    Ok((
        if analyzed {
            ProcessingState::Processed
        } else {
            ProcessingState::Saved
        },
        None,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;

    #[test]
    fn enqueue_dedupes_and_claim_marks_running() {
        let db = Db::open_in_memory().unwrap();
        let c = db.conn();
        enqueue(&c, JobType::AnalyzeCapture, "cap").unwrap();
        enqueue(&c, JobType::AnalyzeCapture, "cap").unwrap();
        assert_eq!(counts(&c).unwrap(), (1, 0, 0));
        let job = claim_next(&c).unwrap().unwrap();
        assert_eq!(job.attempts, 1);
        assert_eq!(counts(&c).unwrap(), (0, 1, 0));
        assert!(claim_next(&c).unwrap().is_none());
        complete(&c, &job.id).unwrap();
        assert_eq!(counts(&c).unwrap(), (0, 0, 0));
    }

    #[test]
    fn failure_backs_off_then_parks() {
        let db = Db::open_in_memory().unwrap();
        let c = db.conn();
        enqueue(&c, JobType::AnalyzeCapture, "cap").unwrap();
        let mut job = claim_next(&c).unwrap().unwrap();
        fail(&c, &job, "boom").unwrap();
        // Backoff: not runnable immediately.
        assert!(claim_next(&c).unwrap().is_none());
        assert_eq!(capture_state(&c, "cap").unwrap().1.as_deref(), Some("boom"));
        job.attempts = MAX_ATTEMPTS;
        fail(&c, &job, "still boom").unwrap();
        assert_eq!(counts(&c).unwrap(), (0, 0, 1));
        assert_eq!(
            capture_state(&c, "cap").unwrap().0,
            ProcessingState::NeedsRetry
        );
        assert_eq!(retry_failed(&c, Some("cap")).unwrap(), 1);
        assert!(claim_next(&c).unwrap().is_some());
    }

    #[test]
    fn reset_running_requeues() {
        let db = Db::open_in_memory().unwrap();
        let c = db.conn();
        enqueue(&c, JobType::EmbedCapture, "cap").unwrap();
        claim_next(&c).unwrap().unwrap();
        assert_eq!(reset_running(&c).unwrap(), 1);
        assert_eq!(counts(&c).unwrap(), (1, 0, 0));
    }
}
