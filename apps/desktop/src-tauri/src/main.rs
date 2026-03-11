#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod media;
mod models;
mod permissions;
mod recording;

use std::{fs, path::PathBuf, sync::Mutex};

use models::{
    ClipboardSnapshot, HistoryRecord, NewHistoryRecord, PermissionStatus, RecordedAudio, Settings,
};
use tauri::{AppHandle, Manager, State};

struct AppState {
    database_path: PathBuf,
    recording: Mutex<Option<recording::RecordingHandle>>,
}

impl AppState {
    fn new(database_path: PathBuf) -> Self {
        Self {
            database_path,
            recording: Mutex::new(None),
        }
    }
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    db::get_settings(state.inner().database_path.as_path())
}

#[tauri::command]
fn update_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    db::update_settings(state.inner().database_path.as_path(), &settings)
}

#[tauri::command]
fn list_history(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<HistoryRecord>, String> {
    db::list_history(state.inner().database_path.as_path(), limit.unwrap_or(10))
}

#[tauri::command]
fn create_history_record(
    state: State<'_, AppState>,
    record: NewHistoryRecord,
) -> Result<HistoryRecord, String> {
    db::create_history_record(state.inner().database_path.as_path(), &record)
}

#[tauri::command]
fn get_permission_status() -> PermissionStatus {
    permissions::permission_status()
}

#[tauri::command]
fn start_recording(
    state: State<'_, AppState>,
    pause_background_audio: Option<bool>,
) -> Result<(), String> {
    recording::start_recording(
        &state.inner().recording,
        pause_background_audio.unwrap_or(true),
    )
}

#[tauri::command]
fn stop_recording(state: State<'_, AppState>) -> Result<RecordedAudio, String> {
    recording::stop_recording(&state.inner().recording)
}

#[tauri::command]
fn get_frontmost_app() -> Result<Option<String>, String> {
    permissions::frontmost_app()
}

#[tauri::command]
fn capture_clipboard() -> Result<ClipboardSnapshot, String> {
    permissions::capture_clipboard()
}

#[tauri::command]
fn set_clipboard_text(text: String) -> Result<(), String> {
    permissions::set_clipboard_text(text)
}

#[tauri::command]
fn restore_clipboard(snapshot: ClipboardSnapshot) -> Result<(), String> {
    permissions::restore_clipboard(snapshot)
}

#[tauri::command]
fn simulate_paste() -> Result<(), String> {
    permissions::simulate_paste()
}

#[tauri::command]
fn quit_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("tator.sqlite3");
            db::initialize(&database_path).map_err(std::io::Error::other)?;
            app.manage(AppState::new(database_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            list_history,
            create_history_record,
            get_permission_status,
            start_recording,
            stop_recording,
            get_frontmost_app,
            capture_clipboard,
            set_clipboard_text,
            restore_clipboard,
            simulate_paste,
            quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tator desktop");
}
