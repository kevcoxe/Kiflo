use std::{
    env,
    ffi::OsString,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use hound::WavReader;
use serde::Deserialize;
use uuid::Uuid;

use crate::models::{CleanupResponse, DictationMode, ModelStatus, TranscriptionResponse};

const HELPER_SCRIPT: &str = include_str!("../resources/python/tator_mlx_helper.py");
const MODEL_LANGUAGE: &str = "en";
const MODEL_NAME: &str = "whisper-tiny.en-mlx";
const MODEL_CONFIG_BYTES: &[u8] = include_bytes!(
    "../../../../models--mlx-community--whisper-tiny.en-mlx/blobs/61ae64eae9676a5a89f3069a90d7109b45ce9447"
);
const MODEL_WEIGHTS_BYTES: &[u8] = include_bytes!(
    "../../../../models--mlx-community--whisper-tiny.en-mlx/blobs/4627203f0807d6496dda0d200e893b04ff24c98e73aa51c485c06203a0330b17"
);
const MODEL_README_BYTES: &[u8] = include_bytes!(
    "../../../../models--mlx-community--whisper-tiny.en-mlx/blobs/d6e15b759c0ee16b72be3aa70c3d74e62e6c9992"
);
const MOCK_PROVIDER: &str = "mock";
const NATIVE_PROVIDER: &str = "native";
const MLX_PROVIDER: &str = "mlx_whisper";
const QUESTION_OPENERS: &[&str] = &[
    "can", "could", "would", "will", "should", "is", "are", "do", "does", "did",
];

#[derive(Debug, Default)]
pub struct ModelRuntimeState {
    pub downloading: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsrProviderKind {
    Mock,
    MlxWhisper,
}

#[derive(Debug, Deserialize)]
struct HelperTranscriptionResponse {
    text: String,
    language: Option<String>,
}

struct HelperRunner {
    program: String,
    base_args: Vec<String>,
}

pub fn transcribe_audio(
    app_data_dir: &Path,
    audio_base64: &str,
    mode: DictationMode,
    app_name: Option<String>,
) -> Result<TranscriptionResponse, String> {
    let audio = STANDARD
        .decode(audio_base64)
        .map_err(|error| format!("Unable to decode recorded audio: {error}"))?;
    let duration_ms = estimate_audio_duration_ms(&audio);

    match selected_provider() {
        AsrProviderKind::Mock => Ok(mock_transcription(mode, app_name, duration_ms)),
        AsrProviderKind::MlxWhisper => transcribe_with_helper(app_data_dir, &audio, duration_ms),
    }
}

pub fn cleanup_text(raw_text: &str, mode: DictationMode, preserve_raw: bool) -> CleanupResponse {
    let text = normalize_text(raw_text, preserve_raw);
    let clean_text = if matches!(mode, DictationMode::Notes | DictationMode::BulletList) {
        format_bullets(&text).unwrap_or_else(|| format_sentence(&text, mode))
    } else {
        format_sentence(&text, mode)
    };

    CleanupResponse {
        clean_text,
        provider: NATIVE_PROVIDER.to_string(),
    }
}

pub fn get_model_status(
    app_data_dir: &Path,
    runtime: &Mutex<ModelRuntimeState>,
) -> Result<ModelStatus, String> {
    let state = runtime
        .lock()
        .map_err(|_| "Model status lock poisoned.".to_string())?;
    if selected_provider() == AsrProviderKind::Mock {
        return Ok(ModelStatus {
            installed: true,
            downloading: state.downloading,
            progress: None,
            error: state.last_error.clone(),
        });
    }

    let install_error = ensure_model_installed(app_data_dir).err();
    let error = state
        .last_error
        .clone()
        .or_else(ffmpeg_availability_error)
        .or(install_error);

    Ok(ModelStatus {
        installed: model_is_installed(app_data_dir),
        downloading: state.downloading,
        progress: None,
        error,
    })
}

pub fn download_model(
    app_data_dir: &Path,
    runtime: &Mutex<ModelRuntimeState>,
) -> Result<(), String> {
    if selected_provider() == AsrProviderKind::Mock {
        return Ok(());
    }

    {
        let mut state = runtime
            .lock()
            .map_err(|_| "Model status lock poisoned.".to_string())?;
        state.downloading = true;
        state.last_error = None;
    }

    let result = ensure_model_installed(app_data_dir).map(|_| ());

    let mut state = runtime
        .lock()
        .map_err(|_| "Model status lock poisoned.".to_string())?;
    state.downloading = false;
    state.last_error = result.as_ref().err().cloned();

    result
}

fn transcribe_with_helper(
    app_data_dir: &Path,
    audio: &[u8],
    duration_ms: i64,
) -> Result<TranscriptionResponse, String> {
    let model_dir = ensure_model_installed(app_data_dir)?;
    let helper_script = ensure_helper_script(app_data_dir)?;
    let runner = resolve_helper_runner(app_data_dir, &helper_script);
    let temp_audio_path = env::temp_dir().join(format!("tator-audio-{}.wav", Uuid::new_v4()));
    fs::write(&temp_audio_path, audio)
        .map_err(|error| format!("Unable to write temporary audio: {error}"))?;

    let output = Command::new(&runner.program)
        .args(&runner.base_args)
        .arg("transcribe")
        .arg("--audio-path")
        .arg(&temp_audio_path)
        .arg("--model-path")
        .arg(&model_dir)
        .arg("--language")
        .arg(MODEL_LANGUAGE)
        .env("PATH", helper_path_env())
        .output();

    let _ = fs::remove_file(&temp_audio_path);

    let output = output.map_err(|error| {
        format!(
            "Unable to launch the local transcription helper using `{}`: {error}",
            runner.program
        )
    })?;

    if !output.status.success() {
        return Err(helper_error_message(&output.stderr, &output.stdout));
    }

    let response: HelperTranscriptionResponse =
        serde_json::from_slice(&output.stdout).map_err(|error| {
            format!("The local transcription helper returned invalid JSON: {error}")
        })?;
    let raw_text = response.text.trim().to_string();
    if raw_text.is_empty() {
        return Err(
            "No speech was detected in the recording. Try speaking louder or checking the selected microphone."
                .to_string(),
        );
    }

    Ok(TranscriptionResponse {
        raw_text,
        language: response
            .language
            .unwrap_or_else(|| MODEL_LANGUAGE.to_string()),
        duration_ms,
        provider: MLX_PROVIDER.to_string(),
    })
}

fn selected_provider() -> AsrProviderKind {
    match env::var("TATOR_INTERNAL_ASR_PROVIDER").ok().as_deref() {
        Some("mlx_whisper") => AsrProviderKind::MlxWhisper,
        Some("mock") => AsrProviderKind::Mock,
        _ if cfg!(debug_assertions) => AsrProviderKind::Mock,
        _ => AsrProviderKind::MlxWhisper,
    }
}

fn mock_transcription(
    mode: DictationMode,
    app_name: Option<String>,
    duration_ms: i64,
) -> TranscriptionResponse {
    let raw_text = match mode {
        DictationMode::Default => app_name
            .map(|name| format!("hey can you send the update to rebekah in {name}"))
            .unwrap_or_else(|| {
                "hey can you send the update to rebekah tomorrow afternoon".to_string()
            }),
        DictationMode::Email => {
            "hi rebekah just following up on the timeline for next week".to_string()
        }
        DictationMode::Chat => "running five minutes late be there soon".to_string(),
        DictationMode::Notes | DictationMode::BulletList => {
            "make a list eggs milk bread and apples".to_string()
        }
    };

    TranscriptionResponse {
        raw_text,
        language: MODEL_LANGUAGE.to_string(),
        duration_ms,
        provider: MOCK_PROVIDER.to_string(),
    }
}

fn normalize_text(raw_text: &str, preserve_raw: bool) -> String {
    let compact = collapse_whitespace(raw_text);
    if preserve_raw {
        return compact;
    }

    let mut cleaned = Vec::new();
    let tokens: Vec<&str> = compact.split_whitespace().collect();
    let mut index = 0;

    while index < tokens.len() {
        let word = normalize_word(tokens[index]);
        if is_filler_word(&word) {
            index += 1;
            continue;
        }

        if word == "you"
            && tokens
                .get(index + 1)
                .map(|next| normalize_word(next) == "know")
                .unwrap_or(false)
        {
            index += 2;
            continue;
        }

        cleaned.push(tokens[index]);
        index += 1;
    }

    collapse_whitespace(&cleaned.join(" "))
        .trim_matches(|character| character == ' ' || character == ',')
        .to_string()
}

fn format_bullets(text: &str) -> Option<String> {
    let candidate = strip_list_prefix(text);
    if candidate.is_empty() {
        return None;
    }

    let items = if !candidate.contains(',') && candidate.to_lowercase().contains(" and ") {
        candidate
            .replace(" and ", " ")
            .replace(" And ", " ")
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        split_list_items(&candidate)
    };

    let cleaned = items
        .into_iter()
        .map(|item| {
            sentence_case(item.trim_matches(|character| character == ' ' || character == '.'))
        })
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

    if cleaned.len() < 2 {
        return None;
    }

    Some(
        cleaned
            .into_iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn format_sentence(text: &str, mode: DictationMode) -> String {
    let mut sentence = sentence_case(text);
    if sentence.is_empty() {
        return sentence;
    }

    if !sentence.ends_with(['.', '!', '?']) {
        let opener = sentence
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_lowercase();
        let punctuation = if matches!(mode, DictationMode::Chat)
            || !QUESTION_OPENERS.contains(&opener.as_str())
        {
            '.'
        } else {
            '?'
        };
        sentence.push(punctuation);
    }

    sentence
}

fn sentence_case(text: &str) -> String {
    let compact = collapse_whitespace(text);
    let mut characters = compact.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_word(word: &str) -> String {
    word.trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_lowercase()
}

fn is_filler_word(word: &str) -> bool {
    matches!(word, "erm" | "ah")
        || (word.starts_with("um") && word.chars().skip(2).all(|character| character == 'm'))
        || (word.starts_with("uh") && word.chars().skip(2).all(|character| character == 'h'))
}

fn strip_list_prefix(text: &str) -> String {
    let mut candidate = collapse_whitespace(text);
    for prefix in [
        "make a bullet list of ",
        "make bullet list of ",
        "make a list of ",
        "make list of ",
        "make a list ",
        "make list ",
        "create a bullet list of ",
        "create bullet list of ",
        "create a list of ",
        "create list of ",
        "create a list ",
        "create list ",
        "write a bullet list of ",
        "write bullet list of ",
        "write a list of ",
        "write list of ",
        "write a list ",
        "write list ",
        "notes ",
        "note ",
        "bullets ",
        "bullet ",
    ] {
        if let Some(stripped) = strip_prefix_case_insensitive(&candidate, prefix) {
            candidate = stripped;
            break;
        }
    }

    candidate
}

fn strip_prefix_case_insensitive(value: &str, prefix: &str) -> Option<String> {
    if value.len() < prefix.len() {
        return None;
    }

    let candidate = &value[..prefix.len()];
    if candidate.eq_ignore_ascii_case(prefix) {
        Some(value[prefix.len()..].to_string())
    } else {
        None
    }
}

fn split_list_items(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    for segment in text.split(',') {
        for part in segment.split(" and ") {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                items.push(trimmed.to_string());
            }
        }
    }
    items
}

fn estimate_audio_duration_ms(audio_bytes: &[u8]) -> i64 {
    if audio_bytes.is_empty() {
        return 0;
    }

    match WavReader::new(Cursor::new(audio_bytes)) {
        Ok(reader) => {
            let spec = reader.spec();
            if spec.sample_rate == 0 {
                0
            } else {
                ((reader.duration() as f64 / f64::from(spec.sample_rate)) * 1000.0).round() as i64
            }
        }
        Err(_) => ((audio_bytes.len() / 32).min(30_000)) as i64,
    }
}

fn model_directory(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join(MODEL_NAME)
}

fn model_is_installed(app_data_dir: &Path) -> bool {
    let model_dir = model_directory(app_data_dir);
    if !model_dir.is_dir() {
        return false;
    }

    model_dir.join("config.json").is_file()
        && (model_dir.join("weights.safetensors").is_file()
            || model_dir.join("weights.npz").is_file())
}

fn ensure_model_installed(app_data_dir: &Path) -> Result<PathBuf, String> {
    let model_dir = model_directory(app_data_dir);
    if model_dir.is_dir()
        && model_dir.join("config.json").is_file()
        && (model_dir.join("weights.safetensors").is_file()
            || model_dir.join("weights.npz").is_file())
    {
        return Ok(model_dir);
    }

    fs::create_dir_all(&model_dir)
        .map_err(|error| format!("Unable to create the model directory: {error}"))?;
    write_embedded_asset(&model_dir.join("config.json"), MODEL_CONFIG_BYTES)?;
    write_embedded_asset(&model_dir.join("weights.npz"), MODEL_WEIGHTS_BYTES)?;
    write_embedded_asset(&model_dir.join("README.md"), MODEL_README_BYTES)?;

    Ok(model_dir)
}

fn write_embedded_asset(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let should_write = match fs::metadata(path) {
        Ok(metadata) => metadata.len() != bytes.len() as u64,
        Err(_) => true,
    };

    if !should_write {
        return Ok(());
    }

    fs::write(path, bytes).map_err(|error| {
        format!(
            "Unable to write bundled model asset `{}`: {error}",
            path.display()
        )
    })
}

fn ensure_helper_script(app_data_dir: &Path) -> Result<PathBuf, String> {
    let runtime_dir = app_data_dir.join("runtime");
    fs::create_dir_all(&runtime_dir)
        .map_err(|error| format!("Unable to prepare the helper runtime directory: {error}"))?;
    let script_path = runtime_dir.join("tator_mlx_helper.py");
    fs::write(&script_path, HELPER_SCRIPT)
        .map_err(|error| format!("Unable to write the local transcription helper: {error}"))?;
    Ok(script_path)
}

fn resolve_helper_runner(app_data_dir: &Path, helper_script: &Path) -> HelperRunner {
    if let Ok(executable) = env::var("TATOR_MLX_HELPER_EXECUTABLE") {
        return HelperRunner {
            program: executable,
            base_args: Vec::new(),
        };
    }

    let bundled_helper = app_data_dir
        .join("runtime")
        .join("bin")
        .join("tator-mlx-helper");
    if bundled_helper.exists() {
        return HelperRunner {
            program: bundled_helper.to_string_lossy().into_owned(),
            base_args: Vec::new(),
        };
    }

    if let Ok(executable) = env::var("TATOR_PYTHON_EXECUTABLE") {
        return HelperRunner {
            program: executable,
            base_args: vec![helper_script.to_string_lossy().into_owned()],
        };
    }

    let dev_python = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../api/.venv/bin/python");
    if dev_python.exists() {
        return HelperRunner {
            program: dev_python.to_string_lossy().into_owned(),
            base_args: vec![helper_script.to_string_lossy().into_owned()],
        };
    }

    HelperRunner {
        program: "python3".to_string(),
        base_args: vec![helper_script.to_string_lossy().into_owned()],
    }
}

fn helper_error_message(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "The local transcription helper failed without an error message.".to_string()
    }
}

fn helper_path_env() -> OsString {
    let mut paths = env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();

    for directory in ffmpeg_search_directories() {
        if !paths.iter().any(|existing| existing == &directory) {
            paths.push(directory);
        }
    }

    env::join_paths(paths).unwrap_or_else(|_| env::var_os("PATH").unwrap_or_default())
}

fn ffmpeg_availability_error() -> Option<String> {
    if resolve_ffmpeg_path().is_some() {
        None
    } else {
        Some(
            "ffmpeg is not available to the packaged app. Install it with Homebrew or bundle it into the app runtime."
                .to_string(),
        )
    }
}

fn resolve_ffmpeg_path() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("TATOR_FFMPEG_EXECUTABLE") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Some(path);
        }
    }

    for directory in ffmpeg_search_directories() {
        let candidate = directory.join("ffmpeg");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

fn ffmpeg_search_directories() -> Vec<PathBuf> {
    let mut directories = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/local/bin"),
    ];
    directories.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    directories
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_formats_sentences() {
        let result = cleanup_text(
            "um can you send the update tomorrow",
            DictationMode::Default,
            false,
        );
        assert_eq!(result.clean_text, "Can you send the update tomorrow?");
    }

    #[test]
    fn cleanup_formats_chat_messages() {
        let result = cleanup_text(
            "running five minutes late be there soon",
            DictationMode::Chat,
            false,
        );
        assert_eq!(
            result.clean_text,
            "Running five minutes late be there soon."
        );
    }

    #[test]
    fn cleanup_formats_bullet_lists() {
        let result = cleanup_text(
            "make a list eggs milk bread and apples",
            DictationMode::BulletList,
            false,
        );
        assert_eq!(result.clean_text, "- Eggs\n- Milk\n- Bread\n- Apples");
    }

    #[test]
    fn cleanup_preserves_raw_text_when_requested() {
        let result = cleanup_text(" um hello there ", DictationMode::Default, true);
        assert_eq!(result.clean_text, "Um hello there.");
    }

    #[test]
    fn cleanup_can_return_empty_text_for_empty_input() {
        let result = cleanup_text("   ", DictationMode::Default, false);
        assert_eq!(result.clean_text, "");
    }

    #[test]
    fn mock_status_reports_ready() {
        let runtime = Mutex::new(ModelRuntimeState::default());
        env::set_var("TATOR_INTERNAL_ASR_PROVIDER", "mock");
        let status = get_model_status(Path::new("/tmp"), &runtime).unwrap();
        env::remove_var("TATOR_INTERNAL_ASR_PROVIDER");
        assert!(status.installed);
        assert!(!status.downloading);
    }
}
