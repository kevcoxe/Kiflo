use std::sync::Mutex;

use crate::models::{HudPhase, HudState};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelHandle, PanelLevel, StyleMask,
    WebviewWindowExt,
};

pub const HUD_WINDOW_LABEL: &str = "hud";
pub const HUD_STATE_EVENT: &str = "hud-state-changed";
pub const HUD_METER_EVENT: &str = "hud-meter";

const HUD_WIDTH: f64 = 272.0;
const HUD_HEIGHT: f64 = 92.0;
const HUD_MARGIN_TOP: i32 = 18;
const HUD_MARGIN_RIGHT: i32 = 18;

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(HudPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            becomes_key_only_if_needed: true,
            hides_on_deactivate: false,
            is_floating_panel: true
        }
    })
}

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
    configure_macos_hud_window(&hud_window)?;
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
            show_hud_window(app, &hud_window)?;
        } else {
            hide_hud_window(app, &hud_window)?;
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
fn configure_macos_hud_window(window: &WebviewWindow) -> Result<(), String> {
    let panel = if let Ok(panel) = window.app_handle().get_webview_panel(HUD_WINDOW_LABEL) {
        panel
    } else {
        window
            .to_panel::<HudPanel>()
            .map_err(|error| error.to_string())?
    };

    apply_macos_hud_panel_state(&panel);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn configure_macos_hud_window(_: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_macos_hud_panel_state(panel: &PanelHandle<tauri::Wry>) {
    panel.set_level(hud_panel_level());
    panel.set_style_mask(hud_style_mask().into());
    panel.set_collection_behavior(hud_collection_behavior().into());
    panel.set_floating_panel(true);
    panel.set_becomes_key_only_if_needed(true);
    panel.set_hides_on_deactivate(false);
    panel.set_ignores_mouse_events(true);
    panel.set_accepts_mouse_moved_events(false);
    panel.set_has_shadow(false);
    panel.set_opaque(false);
    panel.set_transparent(true);
}

#[cfg(target_os = "macos")]
fn show_hud_window(app: &AppHandle, hud_window: &WebviewWindow) -> Result<(), String> {
    hud_window
        .set_visible_on_all_workspaces(true)
        .map_err(|error| error.to_string())?;

    if let Ok(panel) = app.get_webview_panel(HUD_WINDOW_LABEL) {
        apply_macos_hud_panel_state(&panel);
        panel.show();
        return Ok(());
    }

    hud_window
        .set_always_on_top(true)
        .map_err(|error| error.to_string())?;
    hud_window.show().map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn show_hud_window(_: &AppHandle, hud_window: &WebviewWindow) -> Result<(), String> {
    hud_window
        .set_always_on_top(true)
        .map_err(|error| error.to_string())?;
    hud_window
        .set_visible_on_all_workspaces(true)
        .map_err(|error| error.to_string())?;
    hud_window.show().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn hide_hud_window(app: &AppHandle, hud_window: &WebviewWindow) -> Result<(), String> {
    if let Ok(panel) = app.get_webview_panel(HUD_WINDOW_LABEL) {
        panel.hide();
        return Ok(());
    }

    hud_window.hide().map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn hide_hud_window(_: &AppHandle, hud_window: &WebviewWindow) -> Result<(), String> {
    hud_window.hide().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn hud_collection_behavior() -> CollectionBehavior {
    CollectionBehavior::new()
        .can_join_all_spaces()
        .stationary()
        .full_screen_auxiliary()
        .ignores_cycle()
}

#[cfg(target_os = "macos")]
fn hud_style_mask() -> StyleMask {
    StyleMask::empty().borderless().nonactivating_panel()
}

#[cfg(target_os = "macos")]
fn hud_panel_level() -> i64 {
    PanelLevel::Status.value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn hud_collection_behavior_adds_fullscreen_panel_flags() {
        let behavior = hud_collection_behavior().value();

        assert!(behavior
            .contains(tauri_nspanel::objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces));
        assert!(behavior.contains(
            tauri_nspanel::objc2_app_kit::NSWindowCollectionBehavior::FullScreenAuxiliary
        ));
        assert!(
            behavior.contains(tauri_nspanel::objc2_app_kit::NSWindowCollectionBehavior::Stationary)
        );
        assert!(behavior
            .contains(tauri_nspanel::objc2_app_kit::NSWindowCollectionBehavior::IgnoresCycle));
    }
}
