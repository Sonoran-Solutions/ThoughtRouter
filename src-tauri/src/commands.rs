//! Tauri commands. Errors cross the IPC boundary as strings.

use std::fmt::Display;
use std::path::PathBuf;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use thoughtrouter_core::models::*;
use thoughtrouter_core::secrets::{SettingsView, build_processors};
use thoughtrouter_core::settings::{self, Settings, embedding_model_id};
use thoughtrouter_core::util::Rng;
use thoughtrouter_core::{
    atoms, captures, embeddings, export, jobs, pipeline, projects, resurface,
    search as core_search, stats as core_stats,
};

use crate::AppState;

type R<T> = Result<T, String>;

fn err(e: impl Display) -> String {
    format!("{e:#}")
}

// Not `anyhow`-aware Display for anyhow::Error needs alternate format.
fn aerr(e: anyhow::Error) -> String {
    format!("{e:#}")
}

const RECENT_ON_HOME: i64 = 5;

// ---- captures -------------------------------------------------------------

#[tauri::command]
pub fn save_capture(state: State<AppState>, text: String) -> R<CaptureView> {
    let view = {
        let mut conn = state.db.conn();
        let c = captures::create(&mut conn, &text, captures::SOURCE_DESKTOP).map_err(aerr)?;
        captures::view(&conn, c).map_err(aerr)?
    };
    state.kick_worker();
    Ok(view)
}

#[tauri::command]
pub fn list_captures(
    state: State<AppState>,
    before: Option<String>,
    limit: Option<i64>,
) -> R<Vec<CaptureView>> {
    let conn = state.db.conn();
    let list = captures::list(&conn, before.as_deref(), limit.unwrap_or(30)).map_err(aerr)?;
    captures::views(&conn, list).map_err(aerr)
}

#[tauri::command]
pub fn get_capture(state: State<AppState>, id: String) -> R<CaptureView> {
    let conn = state.db.conn();
    let c = captures::get(&conn, &id)
        .map_err(aerr)?
        .ok_or("capture not found")?;
    captures::view(&conn, c).map_err(aerr)
}

#[tauri::command]
pub fn trash_capture(state: State<AppState>, id: String) -> R<()> {
    captures::trash(&state.db.conn(), &id).map_err(aerr)
}

#[tauri::command]
pub fn restore_capture(state: State<AppState>, id: String) -> R<()> {
    captures::restore(&state.db.conn(), &id).map_err(aerr)
}

#[tauri::command]
pub fn purge_capture(state: State<AppState>, id: String) -> R<()> {
    captures::purge(&mut state.db.conn(), &id).map_err(aerr)
}

#[tauri::command]
pub fn list_trash(state: State<AppState>) -> R<Vec<Capture>> {
    captures::list_deleted(&state.db.conn()).map_err(aerr)
}

#[tauri::command]
pub fn reprocess_capture(state: State<AppState>, id: String) -> R<()> {
    pipeline::reprocess(&state.db.conn(), &id).map_err(aerr)?;
    state.kick_worker();
    Ok(())
}

#[tauri::command]
pub fn reprocess_all(state: State<AppState>) -> R<usize> {
    let n = pipeline::reprocess_all(&state.db.conn()).map_err(aerr)?;
    state.kick_worker();
    Ok(n)
}

#[tauri::command]
pub fn retry_failed(state: State<AppState>, capture_id: Option<String>) -> R<usize> {
    let n = jobs::retry_failed(&state.db.conn(), capture_id.as_deref()).map_err(aerr)?;
    state.kick_worker();
    Ok(n)
}

// ---- atom corrections ----------------------------------------------------------

#[tauri::command]
pub fn set_atom_type(state: State<AppState>, atom_id: String, atom_type: AtomType) -> R<()> {
    atoms::set_type(&state.db.conn(), &atom_id, atom_type).map_err(aerr)
}

#[tauri::command]
pub fn set_atom_text(state: State<AppState>, atom_id: String, text: String) -> R<()> {
    let conn = state.db.conn();
    atoms::set_text(&conn, &atom_id, &text).map_err(aerr)?;
    // Text changed: its embedding/neighbours are stale.
    if let Some(a) = atoms::get(&conn, &atom_id).map_err(aerr)? {
        conn.execute(
            "DELETE FROM embeddings WHERE object_type = 'atom' AND object_id = ?1",
            [&atom_id],
        )
        .map_err(err)?;
        jobs::enqueue(&conn, jobs::JobType::EmbedCapture, &a.capture_id).map_err(aerr)?;
    }
    drop(conn);
    state.kick_worker();
    Ok(())
}

#[tauri::command]
pub fn reject_atom(state: State<AppState>, atom_id: String) -> R<()> {
    atoms::set_status(&state.db.conn(), &atom_id, AtomStatus::Rejected).map_err(aerr)
}

#[tauri::command]
pub fn add_atom(
    state: State<AppState>,
    capture_id: String,
    text: String,
    atom_type: AtomType,
) -> R<Atom> {
    let a = {
        let conn = state.db.conn();
        let a = atoms::add_user_atom(&conn, &capture_id, &text, atom_type).map_err(aerr)?;
        jobs::enqueue(&conn, jobs::JobType::EmbedCapture, &capture_id).map_err(aerr)?;
        a
    };
    state.kick_worker();
    Ok(a)
}

// ---- search & related ------------------------------------------------------------

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> R<SearchResponse> {
    let s = settings::get(&state.db.conn()).map_err(aerr)?;
    let (embedder, reason) = match build_processors(&s, &*state.secrets) {
        Ok(p) => match p.embedder {
            Some(e) => (Some(e), None),
            None => (
                None,
                Some("No embedding model set; showing word matches only.".to_string()),
            ),
        },
        Err(why) => (None, Some(format!("{why} Showing word matches only."))),
    };
    core_search::search(&state.db, embedder, reason, &query, 50)
        .await
        .map_err(aerr)
}

#[tauri::command]
pub fn related(state: State<AppState>, capture_id: String) -> R<Vec<RelatedAtom>> {
    let conn = state.db.conn();
    let s = settings::get(&conn).map_err(aerr)?;
    match embedding_model_id(&s) {
        Some(m) => embeddings::related_for_capture(&conn, &capture_id, &m, 6).map_err(aerr),
        None => Ok(vec![]),
    }
}

// ---- projects ----------------------------------------------------------------------

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> R<Vec<ProjectSummary>> {
    projects::summaries(&state.db.conn(), None).map_err(aerr)
}

#[tauri::command]
pub fn create_project(
    state: State<AppState>,
    name: String,
    description: String,
    momentum: Momentum,
) -> R<Project> {
    let p = projects::create(&state.db.conn(), &name, &description, momentum).map_err(aerr)?;
    state.kick_worker();
    Ok(p)
}

#[tauri::command]
pub fn update_project(
    state: State<AppState>,
    id: String,
    name: String,
    description: String,
    momentum: Momentum,
) -> R<Project> {
    let p = projects::update(&state.db.conn(), &id, &name, &description, momentum).map_err(aerr)?;
    state.kick_worker();
    Ok(p)
}

#[tauri::command]
pub fn delete_project(state: State<AppState>, id: String) -> R<()> {
    projects::delete(&state.db.conn(), &id).map_err(aerr)
}

#[tauri::command]
pub fn project_page(state: State<AppState>, id: String) -> R<ProjectPage> {
    projects::page(&state.db.conn(), &id).map_err(aerr)
}

#[tauri::command]
pub async fn summarize_project(state: State<'_, AppState>, id: String) -> R<SynthesisView> {
    let s = settings::get(&state.db.conn()).map_err(aerr)?;
    let p = build_processors(&s, &*state.secrets)?;
    pipeline::synthesize_project(&state.db, &p, &id)
        .await
        .map_err(aerr)
}

#[tauri::command]
pub fn confirm_link(state: State<AppState>, atom_id: String, project_id: String) -> R<()> {
    projects::confirm_link(&state.db.conn(), &atom_id, &project_id).map_err(aerr)
}

#[tauri::command]
pub fn reject_link(state: State<AppState>, atom_id: String, project_id: String) -> R<()> {
    projects::reject_link(&state.db.conn(), &atom_id, &project_id).map_err(aerr)
}

#[tauri::command]
pub fn accept_suggestion(state: State<AppState>, id: String, name: Option<String>) -> R<Project> {
    let p = projects::accept_suggestion(&state.db.conn(), &id, name.as_deref()).map_err(aerr)?;
    state.kick_worker();
    Ok(p)
}

#[tauri::command]
pub fn dismiss_suggestion(state: State<AppState>, id: String) -> R<()> {
    projects::dismiss_suggestion(&state.db.conn(), &id).map_err(aerr)
}

// ---- resurfacing & home ---------------------------------------------------------------

#[tauri::command]
pub fn resurface(state: State<AppState>) -> R<Option<ResurfaceCard>> {
    let conn = state.db.conn();
    let s = settings::get(&conn).map_err(aerr)?;
    resurface::pick(&conn, &s, &mut Rng::from_time(), chrono::Utc::now()).map_err(aerr)
}

#[tauri::command]
pub fn respond_resurface(
    state: State<AppState>,
    event_id: String,
    response: ResurfaceResponse,
) -> R<RespondOutcome> {
    resurface::respond(&state.db.conn(), &event_id, response, chrono::Utc::now()).map_err(aerr)
}

#[tauri::command]
pub fn create_project_from_atom(
    state: State<AppState>,
    atom_id: String,
    name: String,
) -> R<Project> {
    let p = resurface::create_project_from_atom(&state.db.conn(), &atom_id, &name).map_err(aerr)?;
    state.kick_worker();
    Ok(p)
}

fn processing_status_inner(state: &AppState) -> R<ProcessingStatus> {
    let conn = state.db.conn();
    let s = settings::get(&conn).map_err(aerr)?;
    let (queued, running, failed) = jobs::counts(&conn).map_err(aerr)?;
    let blocked_reason = build_processors(&s, &*state.secrets).err();
    Ok(ProcessingStatus {
        enabled: s.processing_enabled,
        provider: match s.provider {
            settings::ProviderKind::Openrouter => "openrouter".into(),
            settings::ProviderKind::Mock => "mock".into(),
        },
        blocked_reason,
        queued,
        running,
        failed,
    })
}

#[tauri::command]
pub fn processing_status(state: State<AppState>) -> R<ProcessingStatus> {
    processing_status_inner(&state)
}

#[tauri::command]
pub fn home(state: State<AppState>) -> R<HomeData> {
    let processing = processing_status_inner(&state)?;
    let conn = state.db.conn();
    let s = settings::get(&conn).map_err(aerr)?;
    let recent = captures::list(&conn, None, RECENT_ON_HOME).map_err(aerr)?;
    Ok(HomeData {
        recent: captures::views(&conn, recent).map_err(aerr)?,
        active_projects: projects::summaries(&conn, Some(&[Momentum::Active, Momentum::Ready]))
            .map_err(aerr)?,
        recurring: resurface::recurring(&conn, &s, 5).map_err(aerr)?,
        pending_suggestions: projects::pending_suggestions(&conn).map_err(aerr)?,
        processing,
    })
}

#[tauri::command]
pub fn stats(state: State<AppState>) -> R<Stats> {
    core_stats::compute(&state.db.conn()).map_err(aerr)
}

// ---- settings ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> R<SettingsView> {
    let s = settings::get(&state.db.conn()).map_err(aerr)?;
    Ok(SettingsView {
        embedding_model_id: embedding_model_id(&s),
        settings: s,
        api_key_source: state.secrets.key_source(),
        data_dir: state.data_dir.display().to_string(),
    })
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<AppState>, next: Settings) -> R<SettingsView> {
    {
        let conn = state.db.conn();
        let old = settings::get(&conn).map_err(aerr)?;
        settings::save(&conn, &next).map_err(aerr)?;
        if embedding_model_id(&old) != embedding_model_id(&next)
            && embedding_model_id(&next).is_some()
        {
            pipeline::reembed_all(&conn).map_err(aerr)?;
        }
        jobs::wake_all(&conn).map_err(aerr)?;
        if old.global_shortcut != next.global_shortcut {
            crate::register_shortcut(&app, &next.global_shortcut)?;
        }
    }
    state.kick_worker();
    get_settings(state)
}

#[tauri::command]
pub fn set_api_key(state: State<AppState>, key: String) -> R<()> {
    if key.trim().is_empty() {
        return Err("API key is empty".into());
    }
    state.secrets.set_api_key(&key).map_err(aerr)?;
    jobs::wake_all(&state.db.conn()).map_err(aerr)?;
    state.kick_worker();
    Ok(())
}

#[tauri::command]
pub fn clear_api_key(state: State<AppState>) -> R<()> {
    state.secrets.clear_api_key().map_err(aerr)
}

// ---- data safety ----------------------------------------------------------------------

fn exports_dir(state: &AppState) -> PathBuf {
    state.data_dir.join("exports")
}

#[tauri::command]
pub fn export_markdown(state: State<AppState>) -> R<ExportResult> {
    export::export_markdown(&state.db.conn(), &exports_dir(&state)).map_err(aerr)
}

#[tauri::command]
pub fn export_json(state: State<AppState>) -> R<ExportResult> {
    export::export_json(&state.db.conn(), &exports_dir(&state)).map_err(aerr)
}

#[tauri::command]
pub fn backup_now(state: State<AppState>) -> R<String> {
    let conn = state.db.conn();
    let s = settings::get(&conn).map_err(aerr)?;
    export::backup(
        &conn,
        &state.data_dir.join("backups"),
        s.backups_to_keep.max(1) as usize,
    )
    .map(|p| p.display().to_string())
    .map_err(aerr)
}

#[tauri::command]
pub async fn import_json(app: AppHandle, state: State<'_, AppState>) -> R<Option<String>> {
    // The native dialog blocks; keep it off the async runtime's workers.
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("ThoughtRouter JSON export", &["json"])
            .blocking_pick_file()
    })
    .await
    .map_err(err)?;
    let Some(file) = picked else { return Ok(None) };
    let path = file.into_path().map_err(err)?;
    let summary = export::import_json(&mut state.db.conn(), &path).map_err(aerr)?;
    state.kick_worker();
    Ok(Some(format!(
        "Imported {} captures and {} projects.",
        summary.captures_added, summary.projects_added
    )))
}

#[tauri::command]
pub fn reveal_path(app: AppHandle, state: State<AppState>, path: String) -> R<()> {
    let p = PathBuf::from(&path);
    // Only reveal things inside the app's own data directory.
    if !p.starts_with(&state.data_dir) {
        return Err("can only reveal files inside the ThoughtRouter data folder".into());
    }
    app.opener().reveal_item_in_dir(p).map_err(err)
}
