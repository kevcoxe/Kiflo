use std::{
    env, fs,
    io::Cursor,
    os::unix::fs::PermissionsExt,
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
    env_vars: Vec<(String, String)>,
}

struct BundledPythonRuntime {
    executable: PathBuf,
    python_home: PathBuf,
    site_packages: PathBuf,
}

pub fn transcribe_audio(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
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
        AsrProviderKind::MlxWhisper => {
            transcribe_with_helper(app_data_dir, resource_dir, &audio, duration_ms)
        }
    }
}

pub fn cleanup_text(raw_text: &str, mode: DictationMode, preserve_raw: bool) -> CleanupResponse {
    let text = normalize_text(raw_text, preserve_raw);
    let clean_text = if matches!(mode, DictationMode::BulletList) {
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
    resource_dir: Option<&Path>,
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
    let runtime_error = helper_runtime_availability_error(app_data_dir, resource_dir);
    let error = state.last_error.clone().or(runtime_error).or(install_error);

    Ok(ModelStatus {
        installed: model_is_installed(app_data_dir),
        downloading: state.downloading,
        progress: None,
        error,
    })
}

pub fn download_model(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    runtime: &Mutex<ModelRuntimeState>,
) -> Result<(), String> {
    if selected_provider() == AsrProviderKind::Mock {
        return Ok(());
    }

    if let Some(error) = helper_runtime_availability_error(app_data_dir, resource_dir) {
        return Err(error);
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
    resource_dir: Option<&Path>,
    audio: &[u8],
    duration_ms: i64,
) -> Result<TranscriptionResponse, String> {
    let model_dir = ensure_model_installed(app_data_dir)?;
    let helper_script = ensure_helper_script(app_data_dir)?;
    let runner = resolve_helper_runner(app_data_dir, resource_dir, &helper_script)?;
    let temp_audio_path = env::temp_dir().join(format!("tator-audio-{}.wav", Uuid::new_v4()));
    fs::write(&temp_audio_path, audio)
        .map_err(|error| format!("Unable to write temporary audio: {error}"))?;

    let mut command = Command::new(&runner.program);
    command
        .args(&runner.base_args)
        .arg("transcribe")
        .arg("--audio-path")
        .arg(&temp_audio_path)
        .arg("--model-path")
        .arg(&model_dir)
        .arg("--language")
        .arg(MODEL_LANGUAGE);
    for (key, value) in &runner.env_vars {
        command.env(key, value);
    }

    let output = command.output();

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
        DictationMode::Chat => "running five minutes late be there soon".to_string(),
        DictationMode::BulletList => "make a list eggs milk bread and apples".to_string(),
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

fn resolve_helper_runner(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    helper_script: &Path,
) -> Result<HelperRunner, String> {
    if let Ok(executable) = env::var("TATOR_MLX_HELPER_EXECUTABLE") {
        return Ok(HelperRunner {
            program: executable,
            base_args: Vec::new(),
            env_vars: Vec::new(),
        });
    }

    let bundled_helper = app_data_dir
        .join("runtime")
        .join("bin")
        .join("tator-mlx-helper");
    if bundled_helper.exists() {
        return Ok(HelperRunner {
            program: bundled_helper.to_string_lossy().into_owned(),
            base_args: Vec::new(),
            env_vars: Vec::new(),
        });
    }

    if let Some(runtime) = ensure_bundled_python_runtime(app_data_dir, resource_dir)? {
        return Ok(HelperRunner {
            program: runtime.executable.to_string_lossy().into_owned(),
            base_args: vec![helper_script.to_string_lossy().into_owned()],
            env_vars: vec![
                (
                    "PYTHONHOME".to_string(),
                    runtime.python_home.to_string_lossy().into_owned(),
                ),
                (
                    "PYTHONPATH".to_string(),
                    runtime.site_packages.to_string_lossy().into_owned(),
                ),
                ("PYTHONDONTWRITEBYTECODE".to_string(), "1".to_string()),
                ("PYTHONNOUSERSITE".to_string(), "1".to_string()),
            ],
        });
    }

    if let Ok(executable) = env::var("TATOR_PYTHON_EXECUTABLE") {
        return Ok(HelperRunner {
            program: executable,
            base_args: vec![helper_script.to_string_lossy().into_owned()],
            env_vars: Vec::new(),
        });
    }

    let dev_python = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../api/.venv/bin/python");
    if dev_python.exists() {
        return Ok(HelperRunner {
            program: dev_python.to_string_lossy().into_owned(),
            base_args: vec![helper_script.to_string_lossy().into_owned()],
            env_vars: Vec::new(),
        });
    }

    Ok(HelperRunner {
        program: "python3".to_string(),
        base_args: vec![helper_script.to_string_lossy().into_owned()],
        env_vars: Vec::new(),
    })
}

fn helper_error_message(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let combined = if !stderr.is_empty() {
        stderr.clone()
    } else {
        stdout.clone()
    };

    if combined.contains("No module named 'mlx_whisper'")
        || combined.contains("No module named \"mlx_whisper\"")
    {
        return "The Python runtime used for local transcription does not have `mlx_whisper` installed. Run `uv sync --project apps/api --extra dev`, then relaunch with `TATOR_PYTHON_EXECUTABLE=/absolute/path/to/apps/api/.venv/bin/python` if the app does not auto-detect that environment.".to_string();
    }

    if combined.contains("No module named 'huggingface_hub'")
        || combined.contains("No module named \"huggingface_hub\"")
    {
        return "The Python runtime used for local transcription is missing `huggingface_hub`. Recreate the helper environment with `uv sync --project apps/api --extra dev`, then point `TATOR_PYTHON_EXECUTABLE` at `apps/api/.venv/bin/python`.".to_string();
    }

    if combined.contains("No module named 'numpy'")
        || combined.contains("No module named \"numpy\"")
    {
        return "The local transcription helper fell back to a host Python that does not include the packaged runtime. Launch the built `Tator.app` bundle from `tauri build`, or rebuild so the app includes `Contents/Resources/runtime`.".to_string();
    }

    if combined.contains("No module named 'httpx'")
        || combined.contains("No module named \"httpx\"")
    {
        return "The packaged MLX runtime is incomplete. Rebuild the desktop app so `Contents/Resources/runtime` includes the Hugging Face client dependencies required by `mlx_whisper`.".to_string();
    }

    if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "The local transcription helper failed without an error message.".to_string()
    }
}

fn helper_runtime_availability_error(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
) -> Option<String> {
    if env::var("TATOR_MLX_HELPER_EXECUTABLE").is_ok()
        || env::var("TATOR_PYTHON_EXECUTABLE").is_ok()
    {
        return None;
    }

    let bundled_helper = app_data_dir
        .join("runtime")
        .join("bin")
        .join("tator-mlx-helper");
    if bundled_helper.exists() {
        return None;
    }

    if bundled_runtime_is_installed(app_data_dir) {
        return None;
    }

    if resource_dir
        .map(resource_runtime_dir)
        .is_some_and(|path| bundled_runtime_source_ready(&path))
    {
        return None;
    }

    let dev_python = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../api/.venv/bin/python");
    if dev_python.exists() {
        return None;
    }

    Some(
        "This build does not include the packaged MLX runtime. Launch the built `Tator.app` bundle from `tauri build`, or rebuild with `npm run prepare:mlx-runtime` before `tauri build`. As a fallback, set `TATOR_PYTHON_EXECUTABLE` to a Python 3.12 environment that has `mlx_whisper` installed.".to_string(),
    )
}

fn ensure_bundled_python_runtime(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
) -> Result<Option<BundledPythonRuntime>, String> {
    let install_dir = bundled_runtime_install_dir(app_data_dir);
    let source_dir = resource_dir
        .map(resource_runtime_dir)
        .filter(|path| bundled_runtime_source_ready(path));

    if bundled_runtime_target_ready(&install_dir)
        && !source_dir
            .as_ref()
            .is_some_and(|source| bundled_runtime_manifest_changed(&install_dir, source))
    {
        return Ok(Some(bundled_runtime_from_install_dir(install_dir)));
    }

    let Some(source_dir) = source_dir else {
        return Ok(None);
    };

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|error| {
            format!(
                "Unable to replace the packaged MLX runtime at `{}`: {error}",
                install_dir.display()
            )
        })?;
    }

    copy_directory_recursive(&source_dir, &install_dir)?;
    ensure_executable(
        &install_dir
            .join("python-home")
            .join("bin")
            .join("python3.12"),
    )?;

    Ok(Some(bundled_runtime_from_install_dir(install_dir)))
}

fn resource_runtime_dir(resource_dir: &Path) -> PathBuf {
    resource_dir.join("runtime")
}

fn bundled_runtime_install_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("runtime").join("bundled-python")
}

fn bundled_runtime_manifest_path(root: &Path) -> PathBuf {
    root.join("manifest.json")
}

fn bundled_runtime_source_ready(root: &Path) -> bool {
    bundled_runtime_manifest_path(root).is_file()
        && root
            .join("python-home")
            .join("bin")
            .join("python3.12")
            .is_file()
        && root.join("site-packages").join("mlx_whisper").is_dir()
}

fn bundled_runtime_target_ready(root: &Path) -> bool {
    bundled_runtime_source_ready(root)
}

fn bundled_runtime_is_installed(app_data_dir: &Path) -> bool {
    bundled_runtime_target_ready(&bundled_runtime_install_dir(app_data_dir))
}

fn bundled_runtime_from_install_dir(root: PathBuf) -> BundledPythonRuntime {
    BundledPythonRuntime {
        executable: root.join("python-home").join("bin").join("python3.12"),
        python_home: root.join("python-home"),
        site_packages: root.join("site-packages"),
    }
}

fn bundled_runtime_manifest_changed(installed_root: &Path, source_root: &Path) -> bool {
    fs::read(bundled_runtime_manifest_path(installed_root)).ok()
        != fs::read(bundled_runtime_manifest_path(source_root)).ok()
}

fn copy_directory_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "Unable to create runtime directory `{}`: {error}",
            destination.display()
        )
    })?;

    for entry in fs::read_dir(source).map_err(|error| {
        format!(
            "Unable to read bundled runtime directory `{}`: {error}",
            source.display()
        )
    })? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            copy_directory_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                format!(
                    "Unable to copy bundled runtime asset `{}`: {error}",
                    source_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn ensure_executable(path: &Path) -> Result<(), String> {
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("Unable to inspect `{}`: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("Unable to mark `{}` as executable: {error}", path.display()))
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
        let status = get_model_status(Path::new("/tmp"), None, &runtime).unwrap();
        env::remove_var("TATOR_INTERNAL_ASR_PROVIDER");
        assert!(status.installed);
        assert!(!status.downloading);
    }

    #[test]
    fn helper_error_message_maps_missing_mlx_whisper_to_setup_instructions() {
        let message =
            helper_error_message(b"ModuleNotFoundError: No module named 'mlx_whisper'", b"");
        assert!(message.contains("does not have `mlx_whisper` installed"));
        assert!(message.contains("uv sync --project apps/api --extra dev"));
        assert!(message.contains("TATOR_PYTHON_EXECUTABLE"));
    }

    #[test]
    fn helper_error_message_maps_missing_numpy_to_bundle_guidance() {
        let message = helper_error_message(b"ModuleNotFoundError: No module named 'numpy'", b"");
        assert!(message.contains("host Python"));
        assert!(message.contains("Tator.app"));
    }

    #[test]
    fn helper_error_message_maps_missing_httpx_to_incomplete_runtime_guidance() {
        let message = helper_error_message(b"ModuleNotFoundError: No module named 'httpx'", b"");
        assert!(message.contains("runtime is incomplete"));
        assert!(message.contains("Contents/Resources/runtime"));
    }
}
