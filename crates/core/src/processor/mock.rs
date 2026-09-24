//! Deterministic processors for tests, CI and trying the app without a key.
//! Heuristic, not smart: good enough to exercise every pipeline path.

use anyhow::Result;
use async_trait::async_trait;

use super::*;
use crate::models::{AtomType, ThreadSynthesis};

pub const PROVIDER: &str = "mock";
pub const EMBED_DIMS: usize = 256;

pub struct MockAnalyzer;
pub struct MockEmbedder;

fn meta(prompt: &str) -> RunMeta {
    RunMeta {
        provider: PROVIDER.into(),
        model: "mock-heuristic-v1".into(),
        prompt_version: Some(prompt.into()),
        schema_version: Some(SCHEMA_VERSION.into()),
    }
}

/// Splits into sentences, keeping the exact source slices (so quotes match).
pub fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        if matches!(ch, '.' | '!' | '?' | '\n') {
            let end = i + ch.len_utf8();
            let s = text[start..end].trim();
            if s.chars().filter(|c| c.is_alphanumeric()).count() >= 3 {
                out.push(s);
            }
            start = end;
        }
    }
    let s = text[start..].trim();
    if s.chars().filter(|c| c.is_alphanumeric()).count() >= 3 {
        out.push(s);
    }
    out
}

pub fn classify(sentence: &str) -> AtomType {
    let s = sentence.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| s.contains(w));
    if has(&["someday", "one day", "not before", "eventually"]) && !s.ends_with('?') {
        AtomType::Someday
    } else if s.trim_end().ends_with('?') {
        AtomType::Question
    } else if has(&[
        "decided",
        "decision",
        "actually keep",
        "going with",
        "we will",
        "i will keep",
    ]) {
        AtomType::Decision
    } else if has(&[
        "research",
        "investigate",
        "figure out",
        "look into",
        "reverse engineer",
    ]) {
        AtomType::Research
    } else if has(&["http://", "https://"]) {
        AtomType::Reference
    } else if has(&[
        "bug",
        "broken",
        "keeps",
        "issue",
        "problem",
        "disconnect",
        "crash",
    ]) {
        AtomType::Problem
    } else if has(&[
        "need to",
        "todo",
        "remember to",
        "i should",
        "change the",
        "fix ",
    ]) {
        AtomType::Task
    } else if has(&[
        "i want",
        "build",
        "worth building",
        "control center",
        "app that",
    ]) {
        AtomType::Project
    } else if has(&["could", "feature", "support for", "plugin"]) {
        AtomType::Feature
    } else {
        AtomType::Spark
    }
}

fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect()
}

#[async_trait]
impl Analyzer for MockAnalyzer {
    fn provider(&self) -> &str {
        PROVIDER
    }

    async fn analyze_capture(&self, input: &AnalyzeInput) -> Result<(CaptureAnalysis, RunMeta)> {
        let atoms = sentences(&input.text)
            .into_iter()
            .map(|s| ExtractedAtom {
                text: s.to_string(),
                atom_type: classify(s),
                confidence: 0.5,
                quote: Some(s.to_string()),
            })
            .collect();
        Ok((
            CaptureAnalysis {
                atoms,
                needs_clarification: false,
                notes: vec![],
            },
            meta(ANALYZE_PROMPT_VERSION),
        ))
    }

    async fn link_projects(&self, input: &LinkInput) -> Result<(LinkOutput, RunMeta)> {
        let mut links = Vec::new();
        let mut new_projects = Vec::new();
        for atom in &input.atoms {
            let atom_tokens = tokens(&atom.text);
            let mut linked = false;
            for cand in &input.candidates {
                let name_tokens = tokens(&cand.name);
                if name_tokens.is_empty() {
                    continue;
                }
                let hits = name_tokens
                    .iter()
                    .filter(|t| atom_tokens.contains(t))
                    .count();
                if hits > 0
                    && !input
                        .rejected
                        .iter()
                        .any(|(a, p)| a == &atom.key && p == &cand.key)
                {
                    linked = true;
                    links.push(LinkSuggestion {
                        atom: atom.key.clone(),
                        project: cand.key.clone(),
                        confidence: 0.55 + 0.4 * hits as f64 / name_tokens.len() as f64,
                        reason: format!("Mentions \"{}\"", cand.name),
                    });
                }
            }
            if !linked && atom.atom_type == AtomType::Project {
                let name: String = atom
                    .text
                    .split_whitespace()
                    .take(6)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim_end_matches(['.', '!', '?', ','])
                    .to_string();
                new_projects.push(NewProjectSuggestion {
                    atom: atom.key.clone(),
                    name,
                    description: atom.text.clone(),
                });
            }
        }
        Ok((
            LinkOutput {
                links,
                new_projects,
            },
            meta(LINK_PROMPT_VERSION),
        ))
    }

    async fn synthesize(&self, input: &SynthesisInput) -> Result<(ThreadSynthesis, RunMeta)> {
        let mut body = ThreadSynthesis {
            summary: format!(
                "{} captures about {} (mock summary).",
                input.entries.len(),
                input.project_name
            ),
            open_questions: vec![],
            possible_next_actions: vec![],
            decisions: vec![],
            changed_assumptions: vec![],
        };
        for e in &input.entries {
            for s in sentences(&e.text) {
                match classify(s) {
                    AtomType::Question | AtomType::Research => body.open_questions.push(s.into()),
                    AtomType::Task => body.possible_next_actions.push(s.into()),
                    AtomType::Decision => body.decisions.push(s.into()),
                    _ => {}
                }
            }
        }
        Ok((body, meta(SYNTHESIZE_PROMPT_VERSION)))
    }
}

/// Feature-hashed bag of words (plus stems): lexical, but deterministic and
/// good enough to exercise similarity code paths.
pub fn hash_embed(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; EMBED_DIMS];
    for t in tokens(text) {
        let stem: String = t.chars().take(5).collect();
        for (w, weight) in [(t.as_str(), 1.0f32), (stem.as_str(), 0.5)] {
            // FNV-1a
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in w.bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            v[(h % EMBED_DIMS as u64) as usize] += weight;
        }
    }
    v
}

#[async_trait]
impl Embedder for MockEmbedder {
    fn model_id(&self) -> String {
        format!("mock/hash-{EMBED_DIMS}")
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| hash_embed(t)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_are_exact_slices() {
        let text = "Need to fix the SSD. Also, a Linux app?\nRandom";
        let s = sentences(text);
        assert_eq!(
            s,
            vec!["Need to fix the SSD.", "Also, a Linux app?", "Random"]
        );
        assert!(s.iter().all(|x| text.contains(x)));
    }

    #[test]
    fn questions_stay_questions() {
        assert_eq!(
            classify("Could DualDex eventually have a plugin system?"),
            AtomType::Question
        );
    }
}
