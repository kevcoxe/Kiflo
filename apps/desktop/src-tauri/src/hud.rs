use std::sync::Mutex;

use crate::models::{HudPhase, HudState};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const HUD_WINDOW_LABEL: &str = "hud";
pub const HUD_STATE_EVENT: &str = "hud-state-changed";
pub const HUD_METER_EVENT: &str = "hud-meter";

const HUD_WIDTH: f64 = 272.0;
const HUD_HEIGHT: f64 = 92.0;
const HUD_MARGIN_TOP: i32 = 18;
const HUD_MARGIN_RIGHT: i32 = 18;

pub fn create_hud_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(HUD_WINDOW_LABEL).is_some() {
        return Ok(());
    }

    let hud_window =
        WebviewWindowBuilder::new(app, HUD_WINDOW_LABEL, WebviewUrl::App("index.html".into()))
            .title("Tator HUD")
            .visible(false)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .resizable(false)
            .skip_taskbar(true)
            .focused(false)
            .focusable(false)
            .shadow(false)
            .visible_on_all_workspaces(true)
            .inner_size(HUD_WIDTH, HUD_HEIGHT)
            .build()
            .map_err(|error| error.to_string())?;

    hud_window
        .set_ignore_cursor_events(true)
        .map_err(|error| error.to_string())?;
    hud_window
        .set_focusable(false)
        .map_err(|error| error.to_string())?;
    apply_macos_fullscreen_auxiliary(&hud_window)?;
    Ok(())
}

pub fn get_hud_state(state: &Mutex<HudState>) -> Result<HudState, String> {
    state
        .lock()
        .map(|state| state.clone())
        .map_err(|_| "HUD state is poisoned".to_string())
}

pub fn show_hud(app: &AppHandle, state: &Mutex<HudState>, phase: HudPhase) -> Result<(), String> {
    update_hud(
        app,
        state,
        HudState {
            visible: true,
            phase,
        },
    )
}

pub fn set_hud_phase(
    app: &AppHandle,
    state: &Mutex<HudState>,
    phase: HudPhase,
) -> Result<(), String> {
    let visible = !matches!(phase, HudPhase::Hidden);
    update_hud(app, state, HudState { visible, phase })
}

pub fn hide_hud(app: &AppHandle, state: &Mutex<HudState>) -> Result<(), String> {
    update_hud(app, state, HudState::default())
}

fn update_hud(
    app: &AppHandle,
    state: &Mutex<HudState>,
    next_state: HudState,
) -> Result<(), String> {
    create_hud_window(app)?;

    {
        let mut guard = state
            .lock()
            .map_err(|_| "HUD state is poisoned".to_string())?;
        *guard = next_state.clone();
    }

    if let Some(hud_window) = app.get_webview_window(HUD_WINDOW_LABEL) {
        hud_window
            .emit(HUD_STATE_EVENT, next_state.clone())
            .map_err(|error| error.to_string())?;

        if next_state.visible {
            position_hud_window(app, &hud_window)?;
            hud_window
                .set_always_on_top(true)
                .map_err(|error| error.to_string())?;
            hud_window
                .set_visible_on_all_workspaces(true)
                .map_err(|error| error.to_string())?;
            hud_window.show().map_err(|error| error.to_string())?;
        } else {
            hud_window.hide().map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn position_hud_window(app: &AppHandle, hud_window: &WebviewWindow) -> Result<(), String> {
    let cursor_position = app.cursor_position().map_err(|error| error.to_string())?;
    let monitor = hud_window
        .monitor_from_point(cursor_position.x, cursor_position.y)
        .map_err(|error| error.to_string())?
        .or_else(|| hud_window.current_monitor().ok().flatten())
        .or_else(|| hud_window.primary_monitor().ok().flatten())
        .ok_or_else(|| "No active monitor is available for the HUD.".to_string())?;

    let window_size = hud_window.outer_size().map_err(|error| error.to_string())?;
    let position = hud_position_for_monitor(&monitor, window_size.width, window_size.height);

    hud_window
        .set_position(position)
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn hud_position_for_monitor(
    monitor: &Monitor,
    window_width: u32,
    _window_height: u32,
) -> PhysicalPosition<i32> {
    let work_area = monitor.work_area();
    let x =
        work_area.position.x + work_area.size.width as i32 - window_width as i32 - HUD_MARGIN_RIGHT;
    let y = work_area.position.y + HUD_MARGIN_TOP;
    PhysicalPosition::new(x.max(work_area.position.x), y)
}

#[cfg(target_os = "macos")]
fn apply_macos_fullscreen_auxiliary(window: &WebviewWindow) -> Result<(), String> {
    use objc2_app_kit::NSWindow;

    let ns_window = window.ns_window().map_err(|error| error.to_string())? as usize;
    window
        .run_on_main_thread(move || unsafe {
            let ns_window = &*(ns_window as *mut NSWindow);
            let behavior = hud_collection_behavior(ns_window.collectionBehavior());
            ns_window.setCollectionBehavior(behavior);
        })
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn apply_macos_fullscreen_auxiliary(_: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn hud_collection_behavior(
    current: objc2_app_kit::NSWindowCollectionBehavior,
) -> objc2_app_kit::NSWindowCollectionBehavior {
    use objc2_app_kit::NSWindowCollectionBehavior;

    current
        | NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn hud_collection_behavior_adds_space_and_fullscreen_flags() {
        use objc2_app_kit::NSWindowCollectionBehavior;

        let behavior = hud_collection_behavior(NSWindowCollectionBehavior::Stationary);

        assert!(behavior.contains(NSWindowCollectionBehavior::CanJoinAllSpaces));
        assert!(behavior.contains(NSWindowCollectionBehavior::FullScreenAuxiliary));
        assert!(behavior.contains(NSWindowCollectionBehavior::Stationary));
    }
}
