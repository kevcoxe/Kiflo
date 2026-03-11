#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod dictation;
mod media;
mod models;
mod permissions;
mod recording;

use std::{fs, path::PathBuf, sync::Mutex};

use models::{
    CleanupResponse, ClipboardSnapshot, DictationMode, HistoryRecord, ModelStatus,
    NewHistoryRecord, PermissionStatus, RecordedAudio, Settings, TranscriptionResponse,
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WindowEvent,
};

const MENU_ITEM_OPEN: &str = "open";
const MENU_ITEM_TOGGLE_RECORDING: &str = "toggle-recording";
const MENU_ITEM_QUIT: &str = "quit";
const TRAY_TOGGLE_RECORDING_EVENT: &str = "tray-toggle-recording";

struct AppState {
    database_path: PathBuf,
    app_data_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    recording: Mutex<Option<recording::RecordingHandle>>,
    model_runtime: Mutex<dictation::ModelRuntimeState>,
}

impl AppState {
    fn new(database_path: PathBuf, app_data_dir: PathBuf, resource_dir: Option<PathBuf>) -> Self {
        Self {
            database_path,
            app_data_dir,
            resource_dir,
            recording: Mutex::new(None),
            model_runtime: Mutex::new(dictation::ModelRuntimeState::default()),
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
fn transcribe_audio(
    state: State<'_, AppState>,
    audio_base64: String,
    mode: DictationMode,
    app_name: Option<String>,
) -> Result<TranscriptionResponse, String> {
    dictation::transcribe_audio(
        state.inner().app_data_dir.as_path(),
        state.inner().resource_dir.as_deref(),
        &audio_base64,
        mode,
        app_name,
    )
}

#[tauri::command]
fn cleanup_text(
    raw_text: String,
    mode: DictationMode,
    app_name: Option<String>,
    preserve_raw: Option<bool>,
) -> Result<CleanupResponse, String> {
    let _ = app_name;
    Ok(dictation::cleanup_text(
        &raw_text,
        mode,
        preserve_raw.unwrap_or(false),
    ))
}

#[tauri::command]
fn get_model_status(state: State<'_, AppState>) -> Result<ModelStatus, String> {
    dictation::get_model_status(
        state.inner().app_data_dir.as_path(),
        state.inner().resource_dir.as_deref(),
        &state.inner().model_runtime,
    )
}

#[tauri::command]
fn download_model(state: State<'_, AppState>) -> Result<(), String> {
    dictation::download_model(
        state.inner().app_data_dir.as_path(),
        state.inner().resource_dir.as_deref(),
        &state.inner().model_runtime,
    )
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

fn show_main_window(app: &AppHandle) -> Result<(), String> {
    app.show().map_err(|error| error.to_string())?;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window is not available.".to_string())?;

    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;

    Ok(())
}

fn tray_icon_image() -> Option<Image<'static>> {
    #[cfg(target_os = "macos")]
    {
        Image::from_bytes(include_bytes!("../icons/source/tray-icon-template.png")).ok()
    }

    #[cfg(not(target_os = "macos"))]
    {
        Image::from_bytes(include_bytes!("../icons/source/tray-icon-color.png")).ok()
    }
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_ITEM_OPEN, "Open Tator", true, None::<&str>)?;
    let toggle = MenuItem::with_id(
        app,
        MENU_ITEM_TOGGLE_RECORDING,
        "Toggle Dictation",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, MENU_ITEM_QUIT, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &toggle, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("Tator")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_ITEM_OPEN => {
                let _ = show_main_window(app);
            }
            MENU_ITEM_TOGGLE_RECORDING => {
                let _ = app.emit(TRAY_TOGGLE_RECORDING_EVENT, ());
            }
            MENU_ITEM_QUIT => {
                app.exit(0);
            }
            _ => {}
        });

    if let Some(icon) = tray_icon_image() {
        tray = tray.icon(icon);

        #[cfg(target_os = "macos")]
        {
            tray = tray.icon_as_template(true);
        }
    } else if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let resource_dir = app.path().resource_dir().ok();
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("tator.sqlite3");
            db::initialize(&database_path).map_err(std::io::Error::other)?;
            app.manage(AppState::new(database_path, app_data_dir, resource_dir));
            create_tray(&app.handle()).map_err(std::io::Error::other)?;
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
            transcribe_audio,
            cleanup_text,
            get_model_status,
            download_model,
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
