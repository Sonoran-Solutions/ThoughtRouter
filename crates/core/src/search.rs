//! Hybrid search: FTS5 (bm25) ⊕ vector similarity, merged by reciprocal-rank
//! fusion. Falls back to lexical-only when no query embedding is available (C9).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use rusqlite::{Connection, params};

use crate::models::{AtomType, MatchKind, SearchKind, SearchResponse, SearchResult};
use crate::processor::Embedder;
use crate::{Db, embeddings};

const RRF_K: f64 = 60.0;
const PER_SOURCE: i64 = 30;
/// Semantic hits below this cosine similarity are noise.
const MIN_SIMILARITY: f32 = 0.2;

/// Turns natural language into a forgiving FTS5 query: every word is a
/// quoted prefix term, OR-ed together, so bm25 ranks by how many match.
pub fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"*", t.to_lowercase()))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" OR "))
    }
}

struct Hit {
    kind: SearchKind,
    id: String,
    capture_id: String,
    text: String,
    snippet: Option<String>,
    atom_type: Option<AtomType>,
    captured_at: String,
}

fn lexical(conn: &Connection, q: &str) -> Result<Vec<Hit>> {
    let Some(fq) = fts_query(q) else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT c.id, c.text, c.captured_at, snippet(captures_fts, 1, '[', ']', '…', 16)
         FROM captures_fts f JOIN captures c ON c.id = f.id
         WHERE captures_fts MATCH ?1 AND c.deleted_at IS NULL
         ORDER BY bm25(captures_fts) LIMIT ?2",
    )?;
    for r in stmt.query_map(params![fq, PER_SOURCE], |r| {
        Ok(Hit {
            kind: SearchKind::Capture,
            id: r.get(0)?,
            capture_id: r.get(0)?,
            text: r.get(1)?,
            captured_at: r.get(2)?,
            snippet: r.get(3)?,
            atom_type: None,
        })
    })? {
        out.push(r?);
    }
    let mut stmt = conn.prepare(
        "SELECT a.id, a.capture_id, a.text, a.type, c.captured_at, snippet(atoms_fts, 1, '[', ']', '…', 16)
         FROM atoms_fts f JOIN atoms a ON a.id = f.id JOIN captures c ON c.id = a.capture_id
         WHERE atoms_fts MATCH ?1 AND a.status = 'active' AND c.deleted_at IS NULL
         ORDER BY bm25(atoms_fts) LIMIT ?2",
    )?;
    for r in stmt.query_map(params![fq, PER_SOURCE], |r| {
        Ok(Hit {
            kind: SearchKind::Atom,
            id: r.get(0)?,
            capture_id: r.get(1)?,
            text: r.get(2)?,
            atom_type: Some(r.get(3)?),
            captured_at: r.get(4)?,
            snippet: r.get(5)?,
        })
    })? {
        out.push(r?);
    }
    Ok(out)
}

fn semantic(conn: &Connection, qv: &[f32], model: &str) -> Result<Vec<Hit>> {
    let mut scored: Vec<(f32, SearchKind, String)> = Vec::new();
    for (id, v) in embeddings::live_captures(conn, model)? {
        scored.push((embeddings::dot(qv, &v), SearchKind::Capture, id));
    }
    for (id, _, v) in embeddings::live_atoms(conn, model)? {
        scored.push((embeddings::dot(qv, &v), SearchKind::Atom, id));
    }
    scored.retain(|s| s.0 >= MIN_SIMILARITY);
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.truncate((PER_SOURCE * 2) as usize);
    let mut out = Vec::new();
    for (_, kind, id) in scored {
        let hit = match kind {
            SearchKind::Capture => conn.query_row(
                "SELECT id, text, captured_at FROM captures WHERE id = ?1",
                [&id],
                |r| {
                    Ok(Hit {
                        kind,
                        id: r.get(0)?,
                        capture_id: r.get(0)?,
                        text: r.get(1)?,
                        captured_at: r.get(2)?,
                        snippet: None,
                        atom_type: None,
                    })
                },
            )?,
            SearchKind::Atom => conn.query_row(
                "SELECT a.id, a.capture_id, a.text, a.type, c.captured_at
                 FROM atoms a JOIN captures c ON c.id = a.capture_id WHERE a.id = ?1",
                [&id],
                |r| {
                    Ok(Hit {
                        kind,
                        id: r.get(0)?,
                        capture_id: r.get(1)?,
                        text: r.get(2)?,
                        atom_type: Some(r.get(3)?),
                        captured_at: r.get(4)?,
                        snippet: None,
                    })
                },
            )?,
        };
        out.push(hit);
    }
    Ok(out)
}

/// Lists are split by kind before ranking so captures and atoms each get a
/// fair share of the fused ranking.
fn fuse(lexical: Vec<Hit>, semantic: Vec<Hit>, limit: usize) -> Vec<SearchResult> {
    struct Acc {
        hit: Hit,
        score: f64,
        lex: bool,
        sem: bool,
    }
    let mut acc: HashMap<(SearchKind, String), Acc> = HashMap::new();
    for (is_lex, list) in [(true, lexical), (false, semantic)] {
        let mut rank_by_kind: HashMap<SearchKind, usize> = HashMap::new();
        for hit in list {
            let rank = rank_by_kind.entry(hit.kind).or_insert(0);
            *rank += 1;
            let contribution = 1.0 / (RRF_K + *rank as f64);
            let key = (hit.kind, hit.id.clone());
            let e = acc.entry(key).or_insert(Acc {
                hit,
                score: 0.0,
                lex: false,
                sem: false,
            });
            e.score += contribution;
            if is_lex {
                e.lex = true;
            } else {
                e.sem = true;
            }
        }
    }
    let mut out: Vec<SearchResult> = acc
        .into_values()
        .map(|a| SearchResult {
            kind: a.hit.kind,
            id: a.hit.id,
            capture_id: a.hit.capture_id,
            text: a.hit.text,
            snippet: a.hit.snippet,
            atom_type: a.hit.atom_type,
            captured_at: a.hit.captured_at,
            matched: match (a.lex, a.sem) {
                (true, true) => MatchKind::Both,
                (true, false) => MatchKind::Lexical,
                _ => MatchKind::Semantic,
            },
            score: a.score,
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(b.captured_at.cmp(&a.captured_at))
    });
    out.truncate(limit);
    out
}

pub async fn search(
    db: &Db,
    embedder: Option<Arc<dyn Embedder>>,
    embed_unavailable_reason: Option<String>,
    query: &str,
    limit: usize,
) -> Result<SearchResponse> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(SearchResponse {
            results: vec![],
            semantic_used: false,
            semantic_note: None,
        });
    }
    let lex = lexical(&db.conn(), query)?;
    let (sem, note) = match embedder {
        None => (
            vec![],
            Some(embed_unavailable_reason.unwrap_or_else(|| "Semantic search is off.".into())),
        ),
        Some(e) => match e.embed(&[query.to_string()]).await {
            Ok(mut v) if !v.is_empty() => {
                let mut qv = v.remove(0);
                embeddings::normalize(&mut qv);
                (semantic(&db.conn(), &qv, &e.model_id())?, None)
            }
            Ok(_) => (
                vec![],
                Some("Embedder returned nothing; showing word matches only.".into()),
            ),
            Err(err) => (
                vec![],
                Some(format!(
                    "Semantic search unavailable ({err}); showing word matches only."
                )),
            ),
        },
    };
    let semantic_used = note.is_none();
    Ok(SearchResponse {
        results: fuse(lex, sem, limit),
        semantic_used,
        semantic_note: note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::Processors;
    use crate::processor::mock::{MockAnalyzer, MockEmbedder};
    use crate::{captures, pipeline};

    #[test]
    fn fts_query_is_injection_safe() {
        assert_eq!(
            fts_query("linux \"control\" AND*"),
            Some("\"linux\"* OR \"control\"* OR \"and\"*".into())
        );
        assert_eq!(fts_query("  ?! "), None);
    }

    #[tokio::test]
    async fn finds_by_words_and_hides_trash() {
        let db = Db::open_in_memory().unwrap();
        let a =
            captures::create(&mut db.conn(), "Linux control center for the handheld", "t").unwrap();
        let b = captures::create(&mut db.conn(), "Horror game in abandoned malls", "t").unwrap();
        let r = search(&db, None, None, "handheld linux", 10).await.unwrap();
        assert_eq!(r.results[0].capture_id, a.id);
        assert!(!r.semantic_used);
        captures::trash(&db.conn(), &b.id).unwrap();
        let r = search(&db, None, None, "malls", 10).await.unwrap();
        assert!(r.results.is_empty());
    }

    #[tokio::test]
    async fn stemming_matches_other_word_forms() {
        let db = Db::open_in_memory().unwrap();
        let a = captures::create(&mut db.conn(), "Researching save corruption", "t").unwrap();
        let r = search(&db, None, None, "research", 10).await.unwrap();
        assert_eq!(r.results[0].capture_id, a.id);
    }

    #[tokio::test]
    async fn hybrid_merges_semantic_hits() {
        let db = Db::open_in_memory().unwrap();
        let p = Processors {
            analyzer: Arc::new(MockAnalyzer),
            embedder: Some(Arc::new(MockEmbedder)),
        };
        let a = captures::create(&mut db.conn(), "Pokemon save persistence research", "t").unwrap();
        pipeline::run_until_idle(&db, &p).await.unwrap();
        let r = search(&db, p.embedder.clone(), None, "pokemon saves", 10)
            .await
            .unwrap();
        assert!(r.semantic_used);
        assert!(
            r.results
                .iter()
                .any(|x| x.capture_id == a.id && x.matched == MatchKind::Both)
        );
    }
}
