//! Provider-independent AI contracts (D-009, D-014).
//!
//! `Analyzer` produces structured interpretations; `Embedder` produces
//! vectors. They are separate so each can come from a different provider.

pub mod mock;
pub mod openrouter;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::{AtomType, ThreadSynthesis};

pub const ANALYZE_PROMPT_VERSION: &str = "analyze_capture.v1";
pub const LINK_PROMPT_VERSION: &str = "link_projects.v1";
pub const SYNTHESIZE_PROMPT_VERSION: &str = "synthesize_thread.v1";
pub const SCHEMA_VERSION: &str = "1";

/// Hard cap so a runaway response can't flood a capture with atoms.
pub const MAX_ATOMS_PER_CAPTURE: usize = 12;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedAtom {
    pub text: String,
    #[serde(rename = "type")]
    pub atom_type: AtomType,
    pub confidence: f64,
    /// Verbatim excerpt of the capture this atom came from.
    pub quote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureAnalysis {
    pub atoms: Vec<ExtractedAtom>,
    pub needs_clarification: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AnalyzeInput {
    pub text: String,
    /// Existing project names, to help preserve proper nouns. Never a
    /// request to classify.
    pub known_projects: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkAtom {
    /// Short key (`a1`, `a2`, …) so the model never has to echo UUIDs.
    pub key: String,
    pub text: String,
    #[serde(rename = "type")]
    pub atom_type: AtomType,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkCandidate {
    /// Short key (`p1`, `p2`, …).
    pub key: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct LinkInput {
    pub atoms: Vec<LinkAtom>,
    pub candidates: Vec<LinkCandidate>,
    /// (atom key, project key) pairs the user already rejected.
    pub rejected: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkSuggestion {
    pub atom: String,
    pub project: String,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewProjectSuggestion {
    pub atom: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkOutput {
    pub links: Vec<LinkSuggestion>,
    pub new_projects: Vec<NewProjectSuggestion>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SynthesisEntry {
    pub captured_at: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct SynthesisInput {
    pub project_name: String,
    pub project_description: String,
    /// Oldest first.
    pub entries: Vec<SynthesisEntry>,
}

/// Provenance for a processor call (docs/AI_PIPELINE.md "Prompt/version discipline").
#[derive(Debug, Clone, PartialEq)]
pub struct RunMeta {
    pub provider: String,
    /// The model that actually served the request.
    pub model: String,
    pub prompt_version: Option<String>,
    pub schema_version: Option<String>,
}

#[async_trait]
pub trait Analyzer: Send + Sync {
    fn provider(&self) -> &str;
    async fn analyze_capture(&self, input: &AnalyzeInput) -> Result<(CaptureAnalysis, RunMeta)>;
    async fn link_projects(&self, input: &LinkInput) -> Result<(LinkOutput, RunMeta)>;
    async fn synthesize(&self, input: &SynthesisInput) -> Result<(ThreadSynthesis, RunMeta)>;
}

#[async_trait]
pub trait Embedder: Send + Sync {
    /// Stable identifier; embeddings are keyed by it (C7).
    fn model_id(&self) -> String;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

#[derive(Clone)]
pub struct Processors {
    pub analyzer: Arc<dyn Analyzer>,
    /// `None` when no embedding model is configured: semantic features
    /// degrade gracefully to lexical-only.
    pub embedder: Option<Arc<dyn Embedder>>,
}
