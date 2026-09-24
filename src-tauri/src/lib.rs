//! Tauri shell: wires `thoughtrouter-core` to the webview (D-014). Commands
//! are thin wrappers; all behaviour lives in the core crate.

mod commands;
mod keychain;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use thoughtrouter_core::secrets::SecretStore;
use thoughtrouter_core::{Db, captures, export, settings, worker::Worker};
use tokio::sync::Notify;

pub struct AppState {
    pub db: Arc<Db>,
    pub secrets: Arc<dyn SecretStore>,
    pub worker_notify: Arc<Notify>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn kick_worker(&self) {
        self.worker_notify.notify_one();
    }
}

/// Event emitted when background processing changes a capture/project.
pub const EVT_PROCESSING: &str = "processing-updated";
/// Event emitted when the global hotkey asks for the capture box.
pub const EVT_FOCUS_CAPTURE: &str = "focus-capture";

fn data_dir(app: &tauri::App) -> anyhow::Result<PathBuf> {
    // Override for development/testing so real data is never touched.
    if let Ok(d) = std::env::var("THOUGHTROUTER_DATA_DIR") {
        return Ok(PathBuf::from(d));
    }
    Ok(app.path().app_data_dir()?)
}

pub fn register_shortcut(app: &tauri::AppHandle, accelerator: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if accelerator.trim().is_empty() {
        return Ok(());
    }
    gs.register(accelerator.trim())
        .map_err(|e| format!("could not register global shortcut {accelerator}: {e}"))
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let dir = data_dir(app)?;
    let db = Arc::new(Db::open(&dir.join("thoughtrouter.db"))?);
    let s = settings::get(&db.conn())?;

    // B3: a backup on every start, before anything else touches the data.
    match export::backup(
        &db.conn(),
        &dir.join("backups"),
        s.backups_to_keep.max(1) as usize,
    ) {
        Ok(p) => eprintln!("backup written to {}", p.display()),
        Err(e) => eprintln!("startup backup failed: {e:#}"),
    }
    if let Err(e) = captures::purge_trash_older_than(&mut db.conn(), s.trash_retention_days) {
        eprintln!("trash purge failed: {e:#}");
    }

    let secrets: Arc<dyn SecretStore> = Arc::new(keychain::KeychainSecrets::default());
    let notify = Arc::new(Notify::new());
    let handle = app.handle().clone();
    let worker = Worker {
        db: db.clone(),
        secrets: secrets.clone(),
        notify: notify.clone(),
        on_update: Arc::new(move |target: &str| {
            let _ = handle.emit(EVT_PROCESSING, target.to_string());
        }),
    };
    tauri::async_runtime::spawn(worker.run());

    app.manage(AppState {
        db,
        secrets,
        worker_notify: notify,
        data_dir: dir,
    });

    if let Err(e) = register_shortcut(app.handle(), &s.global_shortcut) {
        eprintln!("{e}");
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed
                        && let Some(w) = app.get_webview_window("main")
                    {
                        let _ = w.unminimize();
                        let _ = w.show();
                        let _ = w.set_focus();
                        let _ = app.emit(EVT_FOCUS_CAPTURE, ());
                    }
                })
                .build(),
        )
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            commands::save_capture,
            commands::list_captures,
            commands::get_capture,
            commands::trash_capture,
            commands::restore_capture,
            commands::purge_capture,
            commands::list_trash,
            commands::reprocess_capture,
            commands::reprocess_all,
            commands::retry_failed,
            commands::set_atom_type,
            commands::set_atom_text,
            commands::reject_atom,
            commands::add_atom,
            commands::search,
            commands::related,
            commands::list_projects,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::project_page,
            commands::summarize_project,
            commands::confirm_link,
            commands::reject_link,
            commands::accept_suggestion,
            commands::dismiss_suggestion,
            commands::resurface,
            commands::respond_resurface,
            commands::create_project_from_atom,
            commands::home,
            commands::stats,
            commands::processing_status,
            commands::get_settings,
            commands::save_settings,
            commands::set_api_key,
            commands::clear_api_key,
            commands::export_markdown,
            commands::export_json,
            commands::backup_now,
            commands::import_json,
            commands::reveal_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ThoughtRouter");
}
