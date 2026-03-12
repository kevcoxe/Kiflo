use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    pub auto_paste: bool,
    pub pause_background_audio: bool,
    pub recording_cue_volume: i64,
    pub restore_clipboard: bool,
    pub interaction_mode: String,
    pub preferred_mode: String,
    pub save_history: bool,
    pub hide_dock_icon: bool,
    pub show_hud: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "Option+Space".to_string(),
            auto_paste: true,
            pause_background_audio: true,
            recording_cue_volume: 75,
            restore_clipboard: true,
            interaction_mode: "hold".to_string(),
            preferred_mode: "default".to_string(),
            save_history: true,
            hide_dock_icon: false,
            show_hud: true,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictationMode {
    Default,
    Email,
    Chat,
    Notes,
    BulletList,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    pub raw_text: String,
    pub language: String,
    pub duration_ms: i64,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResponse {
    pub clean_text: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub installed: bool,
    pub downloading: bool,
    pub progress: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HudPhase {
    Hidden,
    Recording,
    Processing,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HudState {
    pub visible: bool,
    pub phase: HudPhase,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            visible: false,
            phase: HudPhase::Hidden,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HudMeterPayload {
    pub level: f64,
}
