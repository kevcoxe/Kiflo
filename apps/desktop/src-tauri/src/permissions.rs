use std::process::Command;

use arboard::Clipboard;
use cpal::traits::HostTrait;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::models::{ClipboardSnapshot, PermissionStatus};

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

pub fn permission_status() -> PermissionStatus {
    PermissionStatus {
        microphone: microphone_status(),
        accessibility: accessibility_status(),
    }
}

pub fn capture_clipboard() -> Result<ClipboardSnapshot, String> {
    let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
    let text = clipboard.get_text().ok();
    Ok(ClipboardSnapshot { text })
}

pub fn set_clipboard_text(text: String) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
    clipboard.set_text(text).map_err(|error| error.to_string())
}

pub fn restore_clipboard(snapshot: ClipboardSnapshot) -> Result<(), String> {
    if let Some(text) = snapshot.text {
        let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
        clipboard.set_text(text).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    enigo
        .key(Key::Meta, Direction::Press)
        .map_err(|error| error.to_string())?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|error| error.to_string())?;
    enigo
        .key(Key::Meta, Direction::Release)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn frontmost_app() -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(
                r#"tell application "System Events" to get name of first application process whose frontmost is true"#,
            )
            .output()
            .map_err(|error| error.to_string())?;

        if !output.status.success() {
            return Ok(None);
        }

        let app_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if app_name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(app_name))
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(None)
    }
}

fn microphone_status() -> String {
    if cpal::default_host().default_input_device().is_some() {
        "granted".to_string()
    } else {
        "unknown".to_string()
    }
}

fn accessibility_status() -> String {
    #[cfg(target_os = "macos")]
    unsafe {
        if AXIsProcessTrusted() != 0 {
            "granted".to_string()
        } else {
            "denied".to_string()
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        "unknown".to_string()
    }
}
