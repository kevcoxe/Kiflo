use std::{
    ffi::{c_char, c_void, CStr, CString},
    process::Command,
    ptr, thread,
    time::Duration,
};

use arboard::Clipboard;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, Stream, StreamConfig,
};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::models::{ClipboardSnapshot, FocusedEditorContext, PermissionStatus};

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXUIElementCreateSystemWide() -> *const c_void;
    fn AXUIElementCopyAttributeValue(
        element: *const c_void,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
    static kAXTrustedCheckOptionPrompt: *const c_void;
    fn AXIsProcessTrustedWithOptions(the_dict: *const c_void) -> u8;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> *const c_void;
    fn CFStringGetLength(the_string: *const c_void) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(
        the_string: *const c_void,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> u8;
    fn CFRelease(cf: *const c_void);
}

const SUPPORTED_EDITOR_NAMES: &[&str] = &["Cursor", "Visual Studio Code", "Code - Insiders"];
const POSITIVE_COMPOSER_CUES: &[&str] = &[
    "chat",
    "composer",
    "prompt",
    "assistant",
    "copilot",
    "message",
    "ask",
    "agent",
];
const NEGATIVE_COMPOSER_CUES: &[&str] =
    &["editor", "terminal", "search", "rename", "find", "explorer"];
const TEXT_INPUT_ROLE_CUES: &[&str] = &["axtextarea", "axtextfield", "axsearchfield", "axcombobox"];
const TEXT_INPUT_VALUE_CUES: &[&str] = &["text area", "text field", "editable text", "input"];

#[cfg(target_os = "macos")]
const AX_ERROR_SUCCESS: i32 = 0;
#[cfg(target_os = "macos")]
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[cfg(target_os = "macos")]
struct OwnedCfType(*const c_void);

#[cfg(target_os = "macos")]
impl OwnedCfType {
    fn new(value: *const c_void) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }

    fn as_ptr(&self) -> *const c_void {
        self.0
    }
}

#[cfg(target_os = "macos")]
impl Drop for OwnedCfType {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CFRelease(self.0);
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct AccessibilitySnapshot {
    role_values: Vec<String>,
    text_values: Vec<String>,
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

pub fn simulate_tab() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    enigo
        .key(Key::Tab, Direction::Click)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn simulate_text_input(text: String) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    enigo.text(&text).map_err(|error| error.to_string())
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

pub fn focused_editor_context() -> Result<FocusedEditorContext, String> {
    #[cfg(target_os = "macos")]
    {
        return focused_editor_context_macos();
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(FocusedEditorContext::default())
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
fn focused_editor_context_macos() -> Result<FocusedEditorContext, String> {
    let system = OwnedCfType::new(unsafe { AXUIElementCreateSystemWide() })
        .ok_or_else(|| "Unable to access the macOS accessibility system.".to_string())?;
    let focused_app = ax_copy_attribute_value(system.as_ptr(), "AXFocusedApplication");
    let app_name = focused_app
        .as_ref()
        .and_then(|app| ax_copy_attribute_string(app.as_ptr(), "AXTitle"))
        .or_else(|| frontmost_app().ok().flatten());

    let mut snapshot = AccessibilitySnapshot::default();
    let mut depth = 0;
    let mut visited = Vec::new();
    let mut current = ax_copy_attribute_value(system.as_ptr(), "AXFocusedUIElement");

    while let Some(element) = current {
        let current_ptr = element.as_ptr() as usize;
        if visited.contains(&current_ptr) {
            break;
        }
        visited.push(current_ptr);
        collect_accessibility_strings(element.as_ptr(), &mut snapshot);
        depth += 1;
        if depth >= 6 {
            break;
        }
        current = ax_copy_attribute_value(element.as_ptr(), "AXParent");
    }

    Ok(classify_focused_editor_context(app_name, snapshot))
}

#[cfg(target_os = "macos")]
fn classify_focused_editor_context(
    app_name: Option<String>,
    snapshot: AccessibilitySnapshot,
) -> FocusedEditorContext {
    let is_supported_editor = app_name
        .as_deref()
        .map(|name| SUPPORTED_EDITOR_NAMES.contains(&name))
        .unwrap_or(false);
    let role_blob = snapshot.role_values.join(" ").to_lowercase();
    let text_blob = snapshot.text_values.join(" ").to_lowercase();
    let is_text_input_like = TEXT_INPUT_ROLE_CUES
        .iter()
        .any(|cue| role_blob.contains(cue))
        || TEXT_INPUT_VALUE_CUES
            .iter()
            .any(|cue| text_blob.contains(cue));
    let matched_positive_cues: Vec<String> = POSITIVE_COMPOSER_CUES
        .iter()
        .filter(|cue| role_blob.contains(**cue) || text_blob.contains(**cue))
        .map(|cue| cue.to_string())
        .collect();
    let matched_negative_cues: Vec<String> = NEGATIVE_COMPOSER_CUES
        .iter()
        .filter(|cue| role_blob.contains(**cue) || text_blob.contains(**cue))
        .map(|cue| cue.to_string())
        .collect();
    let positive_hits = matched_positive_cues.len();
    let negative_hits = matched_negative_cues.len();
    let mut confidence = 0.0;

    if is_supported_editor {
        confidence += 0.35;
    }
    if is_text_input_like {
        confidence += 0.30;
    }
    confidence += 0.15 * positive_hits.min(2) as f64;
    confidence -= 0.25 * negative_hits.min(2) as f64;
    confidence = confidence.clamp(0.0, 1.0);

    FocusedEditorContext {
        app_name,
        is_supported_editor,
        is_text_input_like,
        is_composer: is_supported_editor
            && is_text_input_like
            && negative_hits == 0
            && confidence >= 0.60,
        confidence,
        matched_positive_cues,
        matched_negative_cues,
        raw_role_values: snapshot.role_values,
        raw_text_values: snapshot.text_values,
    }
}

#[cfg(target_os = "macos")]
fn collect_accessibility_strings(element: *const c_void, snapshot: &mut AccessibilitySnapshot) {
    for attribute_name in ["AXRole", "AXSubrole", "AXRoleDescription"] {
        if let Some(value) = ax_copy_attribute_string(element, attribute_name) {
            snapshot.role_values.push(value.clone());
            snapshot.text_values.push(value);
        }
    }

    for attribute_name in [
        "AXTitle",
        "AXDescription",
        "AXIdentifier",
        "AXPlaceholderValue",
        "AXHelp",
    ] {
        if let Some(value) = ax_copy_attribute_string(element, attribute_name) {
            snapshot.text_values.push(value);
        }
    }
}

#[cfg(target_os = "macos")]
fn ax_copy_attribute_value(element: *const c_void, attribute_name: &str) -> Option<OwnedCfType> {
    let attribute = cf_string(attribute_name)?;
    let mut value: *const c_void = ptr::null();
    let error = unsafe { AXUIElementCopyAttributeValue(element, attribute.as_ptr(), &mut value) };
    if error != AX_ERROR_SUCCESS {
        return None;
    }
    OwnedCfType::new(value)
}

#[cfg(target_os = "macos")]
fn ax_copy_attribute_string(element: *const c_void, attribute_name: &str) -> Option<String> {
    let value = ax_copy_attribute_value(element, attribute_name)?;
    cf_type_to_string(value.as_ptr())
}

#[cfg(target_os = "macos")]
fn cf_string(value: &str) -> Option<OwnedCfType> {
    let c_string = CString::new(value).ok()?;
    let string = unsafe {
        CFStringCreateWithCString(ptr::null(), c_string.as_ptr(), CF_STRING_ENCODING_UTF8)
    };
    OwnedCfType::new(string)
}

#[cfg(target_os = "macos")]
fn cf_type_to_string(value: *const c_void) -> Option<String> {
    let type_id = unsafe { CFGetTypeID(value) };
    let string_type_id = unsafe { CFStringGetTypeID() };
    if type_id != string_type_id {
        return None;
    }

    let length = unsafe { CFStringGetLength(value) };
    let capacity =
        unsafe { CFStringGetMaximumSizeForEncoding(length, CF_STRING_ENCODING_UTF8) } + 1;
    let mut buffer = vec![0_u8; capacity.max(1) as usize];
    let success = unsafe {
        CFStringGetCString(
            value,
            buffer.as_mut_ptr().cast::<c_char>(),
            capacity,
            CF_STRING_ENCODING_UTF8,
        )
    };
    if success == 0 {
        return None;
    }

    unsafe { CStr::from_ptr(buffer.as_ptr().cast::<c_char>()) }
        .to_str()
        .ok()
        .map(|text| text.trim().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_focused_context_detects_supported_editor_composers() {
        let context = classify_focused_editor_context(
            Some("Cursor".to_string()),
            AccessibilitySnapshot {
                role_values: vec!["AXTextArea".to_string()],
                text_values: vec![
                    "AXTextArea".to_string(),
                    "Ask Copilot".to_string(),
                    "Chat composer".to_string(),
                ],
            },
        );

        assert!(context.is_supported_editor);
        assert!(context.is_composer);
        assert!(context.confidence >= 0.75);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_focused_context_detects_cursor_agent_panel() {
        // Cursor v0.45+ rebranded "Composer" to "Agent"
        let context = classify_focused_editor_context(
            Some("Cursor".to_string()),
            AccessibilitySnapshot {
                role_values: vec!["AXTextArea".to_string()],
                text_values: vec![
                    "AXTextArea".to_string(),
                    "Ask Agent".to_string(),
                    "Chat Agent".to_string(),
                ],
            },
        );

        assert!(context.is_supported_editor);
        assert!(context.is_composer);
        assert!(context.confidence >= 0.75);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_focused_context_rejects_code_editor_fields() {
        let context = classify_focused_editor_context(
            Some("Visual Studio Code".to_string()),
            AccessibilitySnapshot {
                role_values: vec!["AXTextArea".to_string()],
                text_values: vec![
                    "AXTextArea".to_string(),
                    "Editor".to_string(),
                    "Source code".to_string(),
                ],
            },
        );

        assert!(context.is_supported_editor);
        assert!(!context.is_composer);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_focused_context_detects_cursor_agent_with_generic_ax_labels() {
        // Cursor's Agent panel exposes only generic AX labels — no named cues like "agent"
        // or "chat" appear in the AX tree. Detection must rely on supported-editor +
        // text-input + absence of negative cues alone.
        let context = classify_focused_editor_context(
            Some("Cursor".to_string()),
            AccessibilitySnapshot {
                role_values: vec!["AXTextArea".to_string(), "text entry area".to_string(), "AXGroup".to_string(), "group".to_string()],
                text_values: vec!["AXTextArea".to_string(), "text entry area".to_string(), "AXGroup".to_string(), "group".to_string()],
            },
        );

        assert!(context.is_supported_editor);
        assert!(context.is_text_input_like);
        assert_eq!(context.matched_positive_cues.len(), 0);
        assert_eq!(context.matched_negative_cues.len(), 0);
        assert!(context.is_composer);
        assert!(context.confidence >= 0.60);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn classify_focused_context_rejects_unsupported_apps() {
        let context = classify_focused_editor_context(
            Some("Notes".to_string()),
            AccessibilitySnapshot {
                role_values: vec!["AXTextArea".to_string()],
                text_values: vec!["Ask assistant".to_string()],
            },
        );

        assert!(!context.is_supported_editor);
        assert!(!context.is_composer);
    }
}
