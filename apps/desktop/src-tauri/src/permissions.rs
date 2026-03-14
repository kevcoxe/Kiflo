use std::{ffi::c_void, process::Command, ptr, thread, time::Duration};

use arboard::Clipboard;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, Stream, StreamConfig,
};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::models::{ClipboardSnapshot, PermissionStatus};

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    static kAXTrustedCheckOptionPrompt: *const c_void;
    fn AXIsProcessTrustedWithOptions(the_dict: *const c_void) -> u8;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFRelease(cf: *const c_void);
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
        clipboard
            .set_text(text)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    enigo
        .key(paste_modifier_key(), Direction::Press)
        .map_err(|error| error.to_string())?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|error| error.to_string())?;
    enigo
        .key(paste_modifier_key(), Direction::Release)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn simulate_enter() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    enigo
        .key(Key::Return, Direction::Click)
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

pub fn open_microphone_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        open_system_settings_uri(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        )
    }

    #[cfg(target_os = "windows")]
    {
        open_system_settings_uri("ms-settings:privacy-microphone")
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        Err("Opening microphone settings is only supported on macOS and Windows.".to_string())
    }
}

pub fn request_microphone_access() -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No default microphone was found.".to_string())?;
    let supported_config = device
        .default_input_config()
        .map_err(|error| error.to_string())?;
    let stream_config: StreamConfig = supported_config.config();
    let channels = usize::from(stream_config.channels);

    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => build_probe_stream_f32(&device, &stream_config, channels)?,
        SampleFormat::I16 => build_probe_stream_i16(&device, &stream_config, channels)?,
        SampleFormat::U16 => build_probe_stream_u16(&device, &stream_config, channels)?,
        format => return Err(format!("Unsupported sample format: {format:?}")),
    };

    stream.play().map_err(|error| error.to_string())?;
    thread::sleep(Duration::from_millis(200));
    stream.pause().map_err(|error| error.to_string())
}

pub fn request_accessibility_access() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            ptr::null(),
            ptr::null(),
        );

        if options.is_null() {
            return Err("Unable to prepare the accessibility permission request.".to_string());
        }

        let trusted = AXIsProcessTrustedWithOptions(options) != 0;
        CFRelease(options);

        if trusted {
            Ok(())
        } else {
            open_accessibility_settings()
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

fn microphone_status() -> String {
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        return "denied".to_string();
    };

    match device.default_input_config() {
        Ok(_) => "granted".to_string(),
        Err(error) => {
            let message = error.to_string().to_ascii_lowercase();
            if message.contains("permission")
                || message.contains("denied")
                || message.contains("not authorized")
            {
                "denied".to_string()
            } else {
                "unknown".to_string()
            }
        }
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
        "granted".to_string()
    }
}

pub fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        open_system_settings_uri(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        )
    }

    #[cfg(target_os = "windows")]
    {
        open_system_settings_uri("ms-settings:easeofaccess-keyboard")
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        Err("Opening accessibility settings is only supported on macOS and Windows.".to_string())
    }
}

#[cfg(target_os = "macos")]
fn open_system_settings_uri(uri: &str) -> Result<(), String> {
    Command::new("open")
        .arg(uri)
        .status()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_system_settings_uri(uri: &str) -> Result<(), String> {
    Command::new("cmd")
        .args(["/C", "start", "", uri])
        .status()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn open_system_settings_uri(_: &str) -> Result<(), String> {
    Err("Opening system settings is not supported on this platform.".to_string())
}

#[cfg(target_os = "macos")]
fn paste_modifier_key() -> Key {
    Key::Meta
}

#[cfg(not(target_os = "macos"))]
fn paste_modifier_key() -> Key {
    Key::Control
}

fn build_probe_stream_f32(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                for _ in data.chunks(channels) {}
            },
            move |_error| {},
            None,
        )
        .map_err(|error| error.to_string())
}

fn build_probe_stream_i16(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[i16], _| {
                for _ in data.chunks(channels) {}
            },
            move |_error| {},
            None,
        )
        .map_err(|error| error.to_string())
}

fn build_probe_stream_u16(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
) -> Result<Stream, String> {
    device
        .build_input_stream(
            config,
            move |data: &[u16], _| {
                for _ in data.chunks(channels) {}
            },
            move |_error| {},
            None,
        )
        .map_err(|error| error.to_string())
}
