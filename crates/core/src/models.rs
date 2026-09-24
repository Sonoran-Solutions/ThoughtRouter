//! Domain types shared with the UI. Every type here derives `TS`; running
//! `cargo test -p thoughtrouter-core` regenerates `src/bindings/*.ts`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! string_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        #[serde(rename_all = "snake_case")]
        #[ts(export)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $s),+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some($name::$variant),)+ _ => None }
            }
        }

        impl rusqlite::types::ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(self.as_str().into())
            }
        }

        impl rusqlite::types::FromSql for $name {
            fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
                let s = v.as_str()?;
                $name::parse(s).ok_or_else(|| rusqlite::types::FromSqlError::Other(
                    format!("invalid {} value: {s}", stringify!($name)).into()))
            }
        }
    };
}

string_enum!(
    /// Initial atom vocabulary (docs/PRODUCT.md). Keep it small.
    AtomType {
        Spark => "spark",
        Project => "project",
        Feature => "feature",
        Task => "task",
        Question => "question",
        Research => "research",
        Reference => "reference",
        Decision => "decision",
        Problem => "problem",
        Someday => "someday",
    }
);

string_enum!(
    Momentum {
        Spark => "spark",
        Exploring => "exploring",
        Ready => "ready",
        Active => "active",
        Blocked => "blocked",
        Dormant => "dormant",
        Finished => "finished",
    }
);

string_enum!(Origin { Ai => "ai", User => "user" });

string_enum!(AtomStatus { Active => "active", Rejected => "rejected", Superseded => "superseded" });

string_enum!(LinkStatus { Suggested => "suggested", Confirmed => "confirmed", Rejected => "rejected" });

string_enum!(
    /// UX.md processing states. A model failure is never a capture failure.
    ProcessingState {
        Saved => "saved",
        Analyzing => "analyzing",
        Processed => "processed",
        NeedsRetry => "needs_retry",
    }
);

string_enum!(
    ResurfaceResponse {
        Interesting => "interesting",
        NotNow => "not_now",
        MakeActive => "make_active",
        Dismiss => "dismiss",
    }
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Capture {
    pub id: String,
    /// Exactly what the user submitted. Never rewritten.
    pub text: String,
    pub source: String,
    pub captured_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Atom {
    pub id: String,
    pub capture_id: String,
    pub run_id: Option<String>,
    pub text: String,
    pub atom_type: AtomType,
    pub confidence: Option<f64>,
    pub quote: Option<String>,
    /// UTF-16 offsets into the capture text.
    pub span_start: Option<i64>,
    pub span_end: Option<i64>,
    pub origin: Origin,
    pub status: AtomStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LinkView {
    pub atom_id: String,
    pub project_id: String,
    pub project_name: String,
    pub status: LinkStatus,
    pub origin: Origin,
    pub confidence: Option<f64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AtomView {
    pub atom: Atom,
    /// Suggested and confirmed links (rejected links are hidden but kept).
    pub links: Vec<LinkView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CaptureView {
    pub capture: Capture,
    pub state: ProcessingState,
    pub last_error: Option<String>,
    pub atoms: Vec<AtomView>,
    pub pending_suggestions: Vec<ProjectSuggestion>,
    /// `provider/model` of the latest successful analysis, for provenance.
    pub interpreted_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub momentum: Momentum,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectSummary {
    pub project: Project,
    pub confirmed_atoms: i64,
    pub suggested_atoms: i64,
    pub last_activity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectSuggestion {
    pub id: String,
    pub atom_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TimelineEntry {
    pub capture: Capture,
    /// Atoms of this capture linked (suggested or confirmed) to the project.
    pub atoms: Vec<AtomView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ThreadSynthesis {
    pub summary: String,
    pub open_questions: Vec<String>,
    pub possible_next_actions: Vec<String>,
    pub decisions: Vec<String>,
    pub changed_assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SynthesisView {
    pub body: ThreadSynthesis,
    pub source_capture_ids: Vec<String>,
    pub model: Option<String>,
    pub created_at: String,
    /// Newer linked captures exist than the synthesis was based on.
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectPage {
    pub project: Project,
    pub timeline: Vec<TimelineEntry>,
    pub synthesis: Option<SynthesisView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RelatedAtom {
    pub atom: Atom,
    pub capture: Capture,
    pub similarity: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SearchKind {
    Capture,
    Atom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MatchKind {
    Lexical,
    Semantic,
    Both,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SearchResult {
    pub kind: SearchKind,
    pub id: String,
    pub capture_id: String,
    pub text: String,
    /// Lexical snippet with `[` `]` markers around hits, when available.
    pub snippet: Option<String>,
    pub atom_type: Option<AtomType>,
    pub captured_at: String,
    pub matched: MatchKind,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub semantic_used: bool,
    /// Why semantic search was skipped (offline, no embedder, ...).
    pub semantic_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ScoreTerm {
    pub name: String,
    /// Weighted contribution to the score.
    pub value: f64,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResurfaceCard {
    pub event_id: String,
    pub atom: Atom,
    pub capture: Capture,
    pub projects: Vec<LinkView>,
    pub score: f64,
    /// Human-readable reason, from the largest positive term.
    pub reason: String,
    pub terms: Vec<ScoreTerm>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RespondOutcome {
    /// Project whose momentum was set to active (make_active).
    pub activated_project: Option<Project>,
    /// make_active on an atom with no project: UI should offer to create one.
    pub needs_project: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecurringItem {
    pub atom: Atom,
    pub mentions: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcessingStatus {
    pub enabled: bool,
    pub provider: String,
    /// Set when processing can't run (missing key/model, disabled).
    pub blocked_reason: Option<String>,
    pub queued: i64,
    pub running: i64,
    pub failed: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HomeData {
    pub recent: Vec<CaptureView>,
    pub active_projects: Vec<ProjectSummary>,
    pub recurring: Vec<RecurringItem>,
    pub pending_suggestions: Vec<ProjectSuggestion>,
    pub processing: ProcessingStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CountRow {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Stats {
    pub total_captures: i64,
    /// Last 8 ISO weeks, oldest first, labelled by week start (YYYY-MM-DD).
    pub captures_per_week: Vec<CountRow>,
    pub total_atoms: i64,
    /// Atoms the user edited, added or rejected.
    pub corrected_atoms: i64,
    pub atom_correction_rate: Option<f64>,
    pub links_by_status: Vec<CountRow>,
    pub link_confirm_rate: Option<f64>,
    pub resurfacing_responses: Vec<CountRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExportResult {
    pub directory: String,
    pub files: Vec<String>,
}
