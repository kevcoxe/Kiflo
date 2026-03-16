use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SpeechModel {
    BaseEn,
    SmallEn,
}

impl Default for SpeechModel {
    fn default() -> Self {
        Self::BaseEn
    }
}

impl SpeechModel {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::BaseEn => "base_en",
            Self::SmallEn => "small_en",
        }
    }

    pub fn from_db_value(value: &str) -> Self {
        match value {
            "small_en" => Self::SmallEn,
            _ => Self::BaseEn,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    pub auto_paste: bool,
    pub auto_backtrack: bool,
    pub spoken_press_enter_command: bool,
    pub pause_background_audio: bool,
    pub warm_model_on_launch: bool,
    pub speech_model: SpeechModel,
    pub recording_cue_volume: i64,
    pub restore_clipboard: bool,
    pub interaction_mode: String,
    pub preferred_mode: String,
    pub typing_style: String,
    pub custom_dictionary: String,
    pub save_history: bool,
    pub hide_dock_icon: bool,
    pub show_hud: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey().to_string(),
            auto_paste: true,
            auto_backtrack: false,
            spoken_press_enter_command: false,
            pause_background_audio: true,
            warm_model_on_launch: true,
            speech_model: SpeechModel::BaseEn,
            recording_cue_volume: 75,
            restore_clipboard: true,
            interaction_mode: "hold".to_string(),
            preferred_mode: "default".to_string(),
            typing_style: "formal".to_string(),
            custom_dictionary: String::new(),
            save_history: true,
            hide_dock_icon: false,
            show_hud: true,
        }
    }
}

#[cfg(target_os = "windows")]
fn default_hotkey() -> &'static str {
    "Alt+Space"
}

#[cfg(not(target_os = "windows"))]
fn default_hotkey() -> &'static str {
    "Option+Space"
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
pub struct HistoryWordMetric {
    pub word: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMetrics {
    pub total_sessions: i64,
    pub total_words_spoken: i64,
    pub total_speaking_time_ms: i64,
    pub estimated_typing_time_saved_ms: i64,
    pub average_words_per_session: f64,
    pub top_words: Vec<HistoryWordMetric>,
    pub top_meaningful_words: Vec<HistoryWordMetric>,
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
    Chat,
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
    pub bundled_models: Vec<SpeechModel>,
    pub active_model: SpeechModel,
    pub installed_models: Vec<SpeechModel>,
    pub download_target: Option<SpeechModel>,
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
