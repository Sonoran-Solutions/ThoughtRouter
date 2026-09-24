//! Dogfooding metrics (docs/MVP_PLAN.md "MVP success criteria").

use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rusqlite::Connection;

use crate::models::{CountRow, Stats};

fn count_rows(conn: &Connection, sql: &str) -> Result<Vec<CountRow>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok(CountRow {
            label: r.get(0)?,
            count: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn compute(conn: &Connection) -> Result<Stats> {
    let total_captures: i64 = conn.query_row(
        "SELECT count(*) FROM captures WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;

    let today = Utc::now().date_naive();
    let this_monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let mut captures_per_week = Vec::new();
    for i in (0..8).rev() {
        let start: NaiveDate = this_monday - Duration::weeks(i);
        let end = start + Duration::weeks(1);
        let n: i64 = conn.query_row(
            "SELECT count(*) FROM captures WHERE deleted_at IS NULL AND captured_at >= ?1 AND captured_at < ?2",
            [start.to_string(), end.to_string()],
            |r| r.get(0),
        )?;
        captures_per_week.push(CountRow {
            label: start.to_string(),
            count: n,
        });
    }

    // Atoms the user has seen an AI version of, or added: active + rejected.
    let (total_atoms, corrected_atoms): (i64, i64) = conn.query_row(
        "SELECT count(*), coalesce(sum(a.origin = 'user' OR a.status = 'rejected'), 0)
         FROM atoms a JOIN captures c ON c.id = a.capture_id
         WHERE c.deleted_at IS NULL AND a.status IN ('active', 'rejected')",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let links_by_status = count_rows(
        conn,
        "SELECT status, count(*) FROM atom_projects GROUP BY status ORDER BY status",
    )?;
    // Of AI suggestions the user acted on, how many were confirmed.
    let (confirmed_ai, rejected_ai): (i64, i64) = conn.query_row(
        "SELECT coalesce(sum(status = 'confirmed'), 0), coalesce(sum(status = 'rejected'), 0)
         FROM atom_projects WHERE run_id IS NOT NULL",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let resurfacing_responses = count_rows(
        conn,
        "SELECT coalesce(response, 'no response'), count(*) FROM resurfacing_events
         GROUP BY 1 ORDER BY 2 DESC",
    )?;

    Ok(Stats {
        total_captures,
        captures_per_week,
        total_atoms,
        corrected_atoms,
        atom_correction_rate: (total_atoms > 0)
            .then(|| corrected_atoms as f64 / total_atoms as f64),
        links_by_status,
        link_confirm_rate: (confirmed_ai + rejected_ai > 0)
            .then(|| confirmed_ai as f64 / (confirmed_ai + rejected_ai) as f64),
        resurfacing_responses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, captures};

    #[test]
    fn counts_this_week() {
        let db = Db::open_in_memory().unwrap();
        captures::create(&mut db.conn(), "one", "t").unwrap();
        let s = compute(&db.conn()).unwrap();
        assert_eq!(s.total_captures, 1);
        assert_eq!(s.captures_per_week.len(), 8);
        assert_eq!(s.captures_per_week.last().unwrap().count, 1);
        assert_eq!(s.atom_correction_rate, None);
    }
}
