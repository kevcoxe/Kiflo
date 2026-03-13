#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod dictation;
mod hud;
mod media;
mod models;
mod permissions;
mod recording;

use std::{env, fs, path::PathBuf, sync::Mutex};

use hud::{create_hud_window, hide_hud, set_hud_phase, show_hud};
use models::{
    CleanupResponse, ClipboardSnapshot, DictationMode, HistoryRecord, HudPhase, HudState,
    ModelStatus, NewHistoryRecord, PermissionStatus, RecordedAudio, Settings,
    TranscriptionResponse,
};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    ActivationPolicy, AppHandle, Emitter, Manager, State, WindowEvent,
};

const TRAY_ID: &str = "main-tray";
const MENU_ITEM_OPEN: &str = "open";
const MENU_ITEM_TOGGLE_RECORDING: &str = "toggle-recording";
const MENU_ITEM_MODE_SUBMENU: &str = "mode-submenu";
const MENU_ITEM_MODE_DEFAULT: &str = "mode-default";
const MENU_ITEM_MODE_CHAT: &str = "mode-chat";
const MENU_ITEM_MODE_LIST: &str = "mode-list";
const MENU_ITEM_SETTINGS_SUBMENU: &str = "settings-submenu";
const MENU_ITEM_PAUSE_BACKGROUND_AUDIO: &str = "pause-background-audio";
const MENU_ITEM_SHOW_HUD: &str = "show-hud";
const MENU_ITEM_INTERACTION_SUBMENU: &str = "interaction-submenu";
const MENU_ITEM_INTERACTION_HOLD: &str = "interaction-hold";
const MENU_ITEM_INTERACTION_TOGGLE: &str = "interaction-toggle";
const MENU_ITEM_QUIT: &str = "quit";
const TRAY_TOGGLE_RECORDING_EVENT: &str = "tray-toggle-recording";
const TRAY_MODE_CHANGED_EVENT: &str = "tray-mode-changed";
const TRAY_SETTINGS_CHANGED_EVENT: &str = "tray-settings-changed";

struct AppState {
    database_path: PathBuf,
    app_data_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    recording: Mutex<Option<recording::RecordingHandle>>,
    model_runtime: Mutex<dictation::ModelRuntimeState>,
    hud_state: Mutex<HudState>,
}

impl AppState {
    fn new(database_path: PathBuf, app_data_dir: PathBuf, resource_dir: Option<PathBuf>) -> Self {
        Self {
            database_path,
            app_data_dir,
            resource_dir,
            recording: Mutex::new(None),
            model_runtime: Mutex::new(dictation::ModelRuntimeState::default()),
            hud_state: Mutex::new(HudState::default()),
        }
    }
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    db::get_settings(state.inner().database_path.as_path())
}

#[tauri::command]
fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let saved = db::update_settings(state.inner().database_path.as_path(), &settings)?;
    sync_tray_menu(&app, &saved)?;
    Ok(saved)
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
    app: AppHandle,
    state: State<'_, AppState>,
    pause_background_audio: Option<bool>,
) -> Result<(), String> {
    recording::start_recording(
        &state.inner().recording,
        pause_background_audio.unwrap_or(true),
        &app,
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
fn open_microphone_settings() -> Result<(), String> {
    permissions::open_microphone_settings()
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    permissions::open_accessibility_settings()
}

#[tauri::command]
fn request_microphone_access() -> Result<(), String> {
    permissions::request_microphone_access()
}

#[tauri::command]
fn request_accessibility_access() -> Result<(), String> {
    permissions::request_accessibility_access()
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

#[tauri::command]
fn get_hud_state(state: State<'_, AppState>) -> Result<HudState, String> {
    hud::get_hud_state(&state.inner().hud_state)
}

#[tauri::command]
fn show_hud_command(
    app: AppHandle,
    state: State<'_, AppState>,
    phase: HudPhase,
) -> Result<(), String> {
    show_hud(&app, &state.inner().hud_state, phase)
}

#[tauri::command]
fn set_hud_phase_command(
    app: AppHandle,
    state: State<'_, AppState>,
    phase: HudPhase,
) -> Result<(), String> {
    set_hud_phase(&app, &state.inner().hud_state, phase)
}

#[tauri::command]
fn hide_hud_command(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    hide_hud(&app, &state.inner().hud_state)
}

fn tray_mode_menu_items(
    app: &AppHandle,
    preferred_mode: &str,
) -> tauri::Result<[CheckMenuItem<tauri::Wry>; 3]> {
    Ok([
        CheckMenuItem::with_id(
            app,
            MENU_ITEM_MODE_DEFAULT,
            "default",
            true,
            preferred_mode == "default",
            None::<&str>,
        )?,
        CheckMenuItem::with_id(
            app,
            MENU_ITEM_MODE_CHAT,
            "chat",
            true,
            preferred_mode == "chat",
            None::<&str>,
        )?,
        CheckMenuItem::with_id(
            app,
            MENU_ITEM_MODE_LIST,
            "bullet_list",
            true,
            preferred_mode == "bullet_list",
            None::<&str>,
        )?,
    ])
}

fn interaction_mode_for_menu_item(menu_id: &str) -> Option<&'static str> {
    match menu_id {
        MENU_ITEM_INTERACTION_HOLD => Some("hold"),
        MENU_ITEM_INTERACTION_TOGGLE => Some("toggle"),
        _ => None,
    }
}

fn build_tray_menu(app: &AppHandle, settings: &Settings) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, MENU_ITEM_OPEN, "Open Tator", true, None::<&str>)?;
    let toggle = MenuItem::with_id(
        app,
        MENU_ITEM_TOGGLE_RECORDING,
        "Toggle Dictation",
        true,
        None::<&str>,
    )?;
    let [default_mode, chat_mode, list_mode] = tray_mode_menu_items(app, &settings.preferred_mode)?;
    let mode_menu = Submenu::with_id_and_items(
        app,
        MENU_ITEM_MODE_SUBMENU,
        "Mode",
        true,
        &[&default_mode, &chat_mode, &list_mode],
    )?;
    let pause_background_audio = CheckMenuItem::with_id(
        app,
        MENU_ITEM_PAUSE_BACKGROUND_AUDIO,
        "Pause Media",
        true,
        settings.pause_background_audio,
        None::<&str>,
    )?;
    let show_hud = CheckMenuItem::with_id(
        app,
        MENU_ITEM_SHOW_HUD,
        "Show HUD",
        true,
        settings.show_hud,
        None::<&str>,
    )?;
    let interaction_hold = CheckMenuItem::with_id(
        app,
        MENU_ITEM_INTERACTION_HOLD,
        "Hold",
        true,
        settings.interaction_mode == "hold",
        None::<&str>,
    )?;
    let interaction_toggle = CheckMenuItem::with_id(
        app,
        MENU_ITEM_INTERACTION_TOGGLE,
        "Toggle",
        true,
        settings.interaction_mode == "toggle",
        None::<&str>,
    )?;
    let interaction_mode_menu = Submenu::with_id_and_items(
        app,
        MENU_ITEM_INTERACTION_SUBMENU,
        "Shortcut Mode",
        true,
        &[&interaction_hold, &interaction_toggle],
    )?;
    let settings_menu = Submenu::with_id_and_items(
        app,
        MENU_ITEM_SETTINGS_SUBMENU,
        "Settings",
        true,
        &[&pause_background_audio, &show_hud, &interaction_mode_menu],
    )?;
    let separator_before_mode = PredefinedMenuItem::separator(app)?;
    let separator_before_settings = PredefinedMenuItem::separator(app)?;
    let separator_before_quit = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_ITEM_QUIT, "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &open,
            &toggle,
            &separator_before_mode,
            &mode_menu,
            &separator_before_settings,
            &settings_menu,
            &separator_before_quit,
            &quit,
        ],
    )
}

fn preferred_mode_for_menu_item(menu_id: &str) -> Option<&'static str> {
    match menu_id {
        MENU_ITEM_MODE_DEFAULT => Some("default"),
        MENU_ITEM_MODE_CHAT => Some("chat"),
        MENU_ITEM_MODE_LIST => Some("bullet_list"),
        _ => None,
    }
}

fn sync_tray_menu(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "Main tray icon is not available.".to_string())?;
    let menu = build_tray_menu(app, settings).map_err(|error| error.to_string())?;
    tray.set_menu(Some(menu)).map_err(|error| error.to_string())
}

fn update_preferred_mode_from_tray(app: &AppHandle, preferred_mode: &str) -> Result<(), String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "App state is not available.".to_string())?;
    let mut settings = db::get_settings(state.database_path.as_path())?;
    settings.preferred_mode = preferred_mode.to_string();
    let saved = db::update_settings(state.database_path.as_path(), &settings)?;
    sync_tray_menu(app, &saved)?;
    app.emit(TRAY_MODE_CHANGED_EVENT, saved.preferred_mode)
        .map_err(|error| error.to_string())
}

fn update_setting_from_tray<F>(app: &AppHandle, mut updater: F) -> Result<(), String>
where
    F: FnMut(&mut Settings),
{
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "App state is not available.".to_string())?;
    let mut settings = db::get_settings(state.database_path.as_path())?;
    updater(&mut settings);
    let saved = db::update_settings(state.database_path.as_path(), &settings)?;
    sync_tray_menu(app, &saved)?;
    app.emit(TRAY_SETTINGS_CHANGED_EVENT, saved)
        .map_err(|error| error.to_string())
}

fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if should_hide_dock_icon_on_close(app) {
        set_dock_visibility_for_window_state(app, true)?;
    }

    app.show().map_err(|error| error.to_string())?;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window is not available.".to_string())?;

    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;

    Ok(())
}

fn should_hide_dock_icon_on_close(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|state| db::get_settings(state.database_path.as_path()).ok())
        .map(|settings| settings.hide_dock_icon)
        .unwrap_or(false)
}

fn set_dock_visibility_for_window_state(
    app: &AppHandle,
    window_is_visible: bool,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let policy = if window_is_visible {
            ActivationPolicy::Regular
        } else {
            ActivationPolicy::Accessory
        };

        app.set_activation_policy(policy)
            .map_err(|error| error.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        let _ = window_is_visible;
    }

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
    let settings = app
        .try_state::<AppState>()
        .and_then(|state| db::get_settings(state.database_path.as_path()).ok())
        .unwrap_or_default();
    let menu = build_tray_menu(app, &settings)?;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
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
            MENU_ITEM_PAUSE_BACKGROUND_AUDIO => {
                let _ = update_setting_from_tray(app, |settings| {
                    settings.pause_background_audio = !settings.pause_background_audio;
                });
            }
            MENU_ITEM_SHOW_HUD => {
                let _ = update_setting_from_tray(app, |settings| {
                    settings.show_hud = !settings.show_hud;
                });
            }
            menu_id => {
                if let Some(preferred_mode) = preferred_mode_for_menu_item(menu_id) {
                    let _ = update_preferred_mode_from_tray(app, preferred_mode);
                } else if let Some(interaction_mode) = interaction_mode_for_menu_item(menu_id) {
                    let _ = update_setting_from_tray(app, |settings| {
                        settings.interaction_mode = interaction_mode.to_string();
                    });
                }
            }
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

fn resolve_resource_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .resource_dir()
        .ok()
        .filter(|path| path.exists())
        .or_else(|| {
            env::current_exe().ok().and_then(|path| {
                path.parent()
                    .and_then(|parent| parent.parent())
                    .map(|contents| contents.join("Resources"))
                    .filter(|candidate| candidate.exists())
            })
        })
}

fn main() {
    let builder =
        tauri::Builder::default().plugin(tauri_plugin_global_shortcut::Builder::new().build());

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                if should_hide_dock_icon_on_close(&window.app_handle()) {
                    let _ = set_dock_visibility_for_window_state(&window.app_handle(), false);
                }
            }
        })
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let resource_dir = resolve_resource_dir(app.handle());
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("tator.sqlite3");
            db::initialize(&database_path).map_err(std::io::Error::other)?;
            app.manage(AppState::new(database_path, app_data_dir, resource_dir));
            create_tray(&app.handle()).map_err(std::io::Error::other)?;
            create_hud_window(&app.handle()).map_err(std::io::Error::other)?;
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
            open_microphone_settings,
            open_accessibility_settings,
            request_microphone_access,
            request_accessibility_access,
            get_frontmost_app,
            capture_clipboard,
            set_clipboard_text,
            restore_clipboard,
            simulate_paste,
            quit_app,
            get_hud_state,
            show_hud_command,
            set_hud_phase_command,
            hide_hud_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tator desktop");
}
