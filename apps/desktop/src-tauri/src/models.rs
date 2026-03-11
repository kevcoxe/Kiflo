use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    pub api_base_url: String,
    pub auto_paste: bool,
    pub restore_clipboard: bool,
    pub preferred_mode: String,
    pub save_history: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "Option+Space".to_string(),
            api_base_url: "http://127.0.0.1:8000".to_string(),
            auto_paste: true,
            restore_clipboard: true,
            preferred_mode: "default".to_string(),
            save_history: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    pub id: String,
    pub created_at: String,
    pub app_name: Option<String>,
    pub mode: String,
    pub audio_duration_ms: i64,
    pub raw_text: String,
    pub clean_text: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewHistoryRecord {
    pub app_name: Option<String>,
    pub mode: String,
    pub audio_duration_ms: i64,
    pub raw_text: String,
    pub clean_text: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    pub microphone: String,
    pub accessibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSnapshot {
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedAudio {
    pub audio_base64: String,
    pub duration_ms: u32,
    pub sample_rate: u32,
}
