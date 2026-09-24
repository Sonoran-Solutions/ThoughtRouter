//! Vector storage and brute-force similarity (D-015). Vectors are stored
//! L2-normalised, so cosine similarity is a dot product.

use std::collections::HashMap;

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

use crate::models::RelatedAtom;
use crate::util::now_iso;

pub const NEIGHBORS_K: i64 = 10;

pub fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_blob(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

pub fn store(
    conn: &Connection,
    object_type: &str,
    object_id: &str,
    model: &str,
    v: &[f32],
) -> Result<()> {
    let mut v = v.to_vec();
    normalize(&mut v);
    conn.execute(
        "INSERT INTO embeddings (object_type, object_id, model, dims, vector, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(object_type, object_id, model) DO UPDATE SET
           dims = excluded.dims, vector = excluded.vector, created_at = excluded.created_at",
        params![
            object_type,
            object_id,
            model,
            v.len() as i64,
            to_blob(&v),
            now_iso()
        ],
    )?;
    Ok(())
}

pub fn get(
    conn: &Connection,
    object_type: &str,
    object_id: &str,
    model: &str,
) -> Result<Option<Vec<f32>>> {
    Ok(conn
        .query_row(
            "SELECT vector FROM embeddings WHERE object_type = ?1 AND object_id = ?2 AND model = ?3",
            params![object_type, object_id, model],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .optional()?
        .map(|b| from_blob(&b)))
}

/// Embeddings of active atoms in live captures: (atom id, capture id, vector).
pub fn live_atoms(conn: &Connection, model: &str) -> Result<Vec<(String, String, Vec<f32>)>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.capture_id, e.vector FROM embeddings e
         JOIN atoms a ON a.id = e.object_id
         JOIN captures c ON c.id = a.capture_id
         WHERE e.object_type = 'atom' AND e.model = ?1
           AND a.status = 'active' AND c.deleted_at IS NULL",
    )?;
    let rows = stmt.query_map([model], |r| {
        Ok((r.get(0)?, r.get(1)?, from_blob(&r.get::<_, Vec<u8>>(2)?)))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn live_captures(conn: &Connection, model: &str) -> Result<Vec<(String, Vec<f32>)>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, e.vector FROM embeddings e JOIN captures c ON c.id = e.object_id
         WHERE e.object_type = 'capture' AND e.model = ?1 AND c.deleted_at IS NULL",
    )?;
    let rows = stmt.query_map([model], |r| {
        Ok((r.get(0)?, from_blob(&r.get::<_, Vec<u8>>(1)?)))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn projects(conn: &Connection, model: &str) -> Result<Vec<(String, Vec<f32>)>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, e.vector FROM embeddings e JOIN projects p ON p.id = e.object_id
         WHERE e.object_type = 'project' AND e.model = ?1",
    )?;
    let rows = stmt.query_map([model], |r| {
        Ok((r.get(0)?, from_blob(&r.get::<_, Vec<u8>>(1)?)))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Computes top-k neighbours of `atom_id` among atoms of *other* captures,
/// and inserts the reverse edges, trimming each touched list back to k.
pub fn update_neighbors(conn: &Connection, atom_id: &str, model: &str) -> Result<()> {
    let Some(v) = get(conn, "atom", atom_id, model)? else {
        return Ok(());
    };
    let capture_id: String = conn.query_row(
        "SELECT capture_id FROM atoms WHERE id = ?1",
        [atom_id],
        |r| r.get(0),
    )?;
    let mut scored: Vec<(String, f32)> = live_atoms(conn, model)?
        .into_iter()
        .filter(|(id, cap, _)| id != atom_id && cap != &capture_id)
        .map(|(id, _, w)| {
            let s = dot(&v, &w);
            (id, s)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(NEIGHBORS_K as usize);
    conn.execute(
        "DELETE FROM atom_neighbors WHERE atom_id = ?1 AND model = ?2",
        params![atom_id, model],
    )?;
    let mut upsert = conn.prepare(
        "INSERT INTO atom_neighbors (atom_id, neighbor_id, similarity, model) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(atom_id, neighbor_id, model) DO UPDATE SET similarity = excluded.similarity",
    )?;
    let mut trim = conn.prepare(
        "DELETE FROM atom_neighbors WHERE atom_id = ?1 AND model = ?2 AND neighbor_id NOT IN
           (SELECT neighbor_id FROM atom_neighbors WHERE atom_id = ?1 AND model = ?2
            ORDER BY similarity DESC LIMIT ?3)",
    )?;
    for (nid, s) in &scored {
        upsert.execute(params![atom_id, nid, *s as f64, model])?;
        upsert.execute(params![nid, atom_id, *s as f64, model])?;
        trim.execute(params![nid, model, NEIGHBORS_K])?;
    }
    Ok(())
}

/// Number of *other* captures containing an atom at least `threshold`
/// similar: the recurrence signal ("you keep coming back to this").
pub fn mentions(conn: &Connection, atom_id: &str, model: &str, threshold: f64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT count(DISTINCT a.capture_id) FROM atom_neighbors n
         JOIN atoms a ON a.id = n.neighbor_id
         JOIN captures c ON c.id = a.capture_id
         WHERE n.atom_id = ?1 AND n.model = ?2 AND n.similarity >= ?3
           AND a.status = 'active' AND c.deleted_at IS NULL",
        params![atom_id, model, threshold],
        |r| r.get(0),
    )?)
}

/// Mentions for every atom that has any above-threshold neighbour.
pub fn all_mentions(
    conn: &Connection,
    model: &str,
    threshold: f64,
) -> Result<HashMap<String, i64>> {
    let mut stmt = conn.prepare(
        "SELECT n.atom_id, count(DISTINCT a.capture_id) FROM atom_neighbors n
         JOIN atoms a ON a.id = n.neighbor_id
         JOIN captures c ON c.id = a.capture_id
         WHERE n.model = ?1 AND n.similarity >= ?2 AND a.status = 'active' AND c.deleted_at IS NULL
         GROUP BY n.atom_id",
    )?;
    let rows = stmt.query_map(params![model, threshold], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// "Possibly related": best neighbouring atoms (from other captures) of a
/// capture's atoms.
pub fn related_for_capture(
    conn: &Connection,
    capture_id: &str,
    model: &str,
    limit: i64,
) -> Result<Vec<RelatedAtom>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {}, {}, max(n.similarity) AS sim
         FROM atoms src
         JOIN atom_neighbors n ON n.atom_id = src.id AND n.model = ?2
         JOIN atoms a ON a.id = n.neighbor_id
         JOIN captures c ON c.id = a.capture_id
         WHERE src.capture_id = ?1 AND src.status = 'active'
           AND a.status = 'active' AND c.deleted_at IS NULL AND a.capture_id != ?1
         GROUP BY a.id ORDER BY sim DESC LIMIT ?3",
        crate::atoms::cols_a(),
        "c.id AS c_id, c.text AS c_text, c.source AS c_source, c.captured_at AS c_captured_at, c.deleted_at AS c_deleted_at"
    ))?;
    let rows = stmt.query_map(params![capture_id, model, limit], |r| {
        Ok(RelatedAtom {
            atom: crate::atoms::from_row(r)?,
            capture: crate::models::Capture {
                id: r.get("c_id")?,
                text: r.get("c_text")?,
                source: r.get("c_source")?,
                captured_at: r.get("c_captured_at")?,
                deleted_at: r.get("c_deleted_at")?,
            },
            similarity: r.get("sim")?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_round_trip_and_normalisation() {
        let db = crate::Db::open_in_memory().unwrap();
        store(&db.conn(), "capture", "x", "m", &[3.0, 4.0]).unwrap();
        let v = get(&db.conn(), "capture", "x", "m").unwrap().unwrap();
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
        assert!((dot(&v, &v) - 1.0).abs() < 1e-6);
    }
}
