//! ThoughtRouter application core (D-014).
//!
//! Owns persistence, the processing job queue, AI adapters, search and
//! resurfacing. The Tauri shell (`src-tauri`) is a thin command layer over
//! this crate, so everything here is testable without a GUI.

pub mod atoms;
pub mod captures;
pub mod db;
pub mod embeddings;
pub mod eval;
pub mod export;
pub mod jobs;
pub mod models;
pub mod pipeline;
pub mod processor;
pub mod projects;
pub mod resurface;
pub mod search;
pub mod secrets;
pub mod settings;
pub mod stats;
pub mod util;
pub mod worker;

pub use db::Db;

/// Recorded on every processor run for provenance.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
