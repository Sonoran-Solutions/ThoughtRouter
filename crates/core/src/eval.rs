//! Golden-fixture evaluation (docs/EXAMPLES.md, docs/AI_PIPELINE.md "Quality
//! evaluation"). Asserts structural properties, never exact wording, so the
//! same fixtures work for the mock (CI) and for real models (`examples/eval.rs`).

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::models::{AtomType, Momentum};
use crate::processor::Processors;
use crate::{Db, captures, pipeline, projects};

#[derive(Debug, Deserialize)]
pub struct FixtureProject {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct AtomExpectation {
    /// An atom whose text contains any of these (case-insensitive)…
    pub contains_any: Vec<String>,
    /// …must exist and have one of these types.
    pub types: Vec<AtomType>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Expect {
    pub min_atoms: Option<usize>,
    pub max_atoms: Option<usize>,
    pub forbid_types: Vec<AtomType>,
    pub atoms: Vec<AtomExpectation>,
    /// Every listed project must get at least one (suggested) link.
    pub links_to: Vec<String>,
    pub no_links: bool,
    pub no_new_project_suggestions: bool,
}

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub projects: Vec<FixtureProject>,
    /// Earlier captures, processed before `capture`.
    #[serde(default)]
    pub history: Vec<String>,
    pub capture: String,
    pub expect: Expect,
}

#[derive(Debug)]
pub struct FixtureResult {
    pub id: String,
    pub failures: Vec<String>,
    /// Human-readable dump of what the processor produced.
    pub produced: Vec<String>,
}

impl FixtureResult {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

pub fn load_fixtures(dir: &Path) -> Result<Vec<Fixture>> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            serde_json::from_slice(&std::fs::read(p)?)
                .with_context(|| format!("parsing {}", p.display()))
        })
        .collect()
}

pub async fn run_fixture(f: &Fixture, p: &Processors) -> Result<FixtureResult> {
    let db = Db::open_in_memory()?;
    for pr in &f.projects {
        projects::create(&db.conn(), &pr.name, &pr.description, Momentum::Exploring)?;
    }
    for h in &f.history {
        captures::create(&mut db.conn(), h, "fixture")?;
    }
    pipeline::run_until_idle(&db, p).await?;
    let capture = captures::create(&mut db.conn(), &f.capture, "fixture")?;
    pipeline::run_until_idle(&db, p).await?;

    let conn = db.conn();
    let view = captures::view(&conn, captures::get(&conn, &capture.id)?.expect("exists"))?;
    let mut failures = Vec::new();
    let e = &f.expect;

    // Invariants that hold for every fixture.
    if view.capture.text != f.capture {
        failures.push("raw capture text changed".into());
    }
    if let Some(err) = &view.last_error {
        failures.push(format!("processing error: {err}"));
    }

    let atoms: Vec<_> = view.atoms.iter().map(|a| &a.atom).collect();
    let produced = view
        .atoms
        .iter()
        .map(|a| {
            let links: Vec<_> = a.links.iter().map(|l| l.project_name.as_str()).collect();
            format!(
                "[{}] {}{}",
                a.atom.atom_type.as_str(),
                a.atom.text,
                if links.is_empty() {
                    String::new()
                } else {
                    format!("  → {}", links.join(", "))
                }
            )
        })
        .collect();

    if let Some(min) = e.min_atoms
        && atoms.len() < min
    {
        failures.push(format!("expected ≥ {min} atoms, got {}", atoms.len()));
    }
    if let Some(max) = e.max_atoms
        && atoms.len() > max
    {
        failures.push(format!("expected ≤ {max} atoms, got {}", atoms.len()));
    }
    for t in &e.forbid_types {
        if atoms.iter().any(|a| a.atom_type == *t) {
            failures.push(format!("forbidden type `{}` produced", t.as_str()));
        }
    }
    for want in &e.atoms {
        let matching: Vec<_> = atoms
            .iter()
            .filter(|a| {
                let t = a.text.to_lowercase();
                want.contains_any
                    .iter()
                    .any(|k| t.contains(&k.to_lowercase()))
            })
            .collect();
        if matching.is_empty() {
            failures.push(format!("no atom mentions any of {:?}", want.contains_any));
        } else if !matching.iter().any(|a| want.types.contains(&a.atom_type)) {
            let got: Vec<_> = matching.iter().map(|a| a.atom_type.as_str()).collect();
            let wanted: Vec<_> = want.types.iter().map(|t| t.as_str()).collect();
            failures.push(format!(
                "atom about {:?} typed {got:?}, expected one of {wanted:?}",
                want.contains_any
            ));
        }
    }
    let linked: Vec<&str> = view
        .atoms
        .iter()
        .flat_map(|a| a.links.iter().map(|l| l.project_name.as_str()))
        .collect();
    for name in &e.links_to {
        if !linked.contains(&name.as_str()) {
            failures.push(format!("not linked to project `{name}`"));
        }
    }
    if e.no_links && !linked.is_empty() {
        failures.push(format!("expected no project links, got {linked:?}"));
    }
    if e.no_new_project_suggestions && !view.pending_suggestions.is_empty() {
        failures.push(format!(
            "unexpected new-project suggestion(s): {:?}",
            view.pending_suggestions
                .iter()
                .map(|s| &s.name)
                .collect::<Vec<_>>()
        ));
    }
    Ok(FixtureResult {
        id: f.id.clone(),
        failures,
        produced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::mock::{MockAnalyzer, MockEmbedder};
    use std::sync::Arc;

    /// Contract test: every golden fixture passes against the deterministic
    /// processor, proving the pipeline plumbing and the harness itself.
    #[tokio::test]
    async fn golden_fixtures_pass_with_mock() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let fixtures = load_fixtures(&dir).unwrap();
        assert_eq!(fixtures.len(), 8);
        let p = Processors {
            analyzer: Arc::new(MockAnalyzer),
            embedder: Some(Arc::new(MockEmbedder)),
        };
        let mut failed = Vec::new();
        for f in &fixtures {
            let r = run_fixture(f, &p).await.unwrap();
            if !r.passed() {
                failed.push(format!(
                    "{}: {:?}\n  produced: {:#?}",
                    r.id, r.failures, r.produced
                ));
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }
}
