use std::{
    env,
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use hound::{SampleFormat, WavReader};
use reqwest::blocking::Client;
use sha1::{Digest, Sha1};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{
    models::{
        CleanupInsertionSegment, CleanupResponse, DictationMode, ModelStatus, SpeechModel,
        TranscriptionResponse,
    },
    whisper_models::{
        all_models, bundled_models, find_model, is_known_model_file, WhisperModelSpec,
    },
};

const MODEL_LANGUAGE: &str = "en";
const MODEL_ASSET_DIR: &str = "whisper";
const MODEL_INSTALL_DIR: &str = "whisper.cpp";
const MOCK_PROVIDER: &str = "mock";
const NATIVE_PROVIDER: &str = "native";
const WHISPER_CPP_PROVIDER: &str = "whisper_cpp";
const TARGET_SAMPLE_RATE: u32 = 16_000;
const QUESTION_OPENERS: &[&str] = &[
    "can", "could", "would", "will", "should", "is", "are", "do", "does", "did",
];
const EXCITED_OPENERS: &[&str] = &[
    "awesome",
    "congrats",
    "congratulations",
    "excellent",
    "great",
    "hooray",
    "love",
    "nice",
    "perfect",
    "thanks",
    "thank",
    "woo",
    "yay",
];
const EXCITED_PREFIXES: &[&str] = &["let's go", "sounds good", "so excited", "thank you"];
const FILE_MENTION_PLACEHOLDER_PREFIX: &str = "__TATOR_FILE_";
const FILE_INTENT_CUES: &[&str] = &["open", "edit", "read", "attach", "use", "check"];
const FILE_INTENT_SKIP_WORDS: &[&str] = &["a", "an", "the", "this", "that", "my", "your", "our"];
const FILE_MENTION_BOUNDARY_WORDS: &[&str] = &[
    "and", "or", "please", "now", "today", "tomorrow", "in", "on", "from", "with", "at", "for",
];
const FILE_SEPARATOR_WORDS: &[(&str, &str)] = &[
    ("slash", "/"),
    ("dash", "-"),
    ("hyphen", "-"),
    ("underscore", "_"),
];
const FILE_EXTENSION_WORDS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "json", "md", "mdx", "py", "rs", "go", "java", "c", "h", "cpp",
    "css", "scss", "html", "yaml", "yml", "toml", "swift", "kt", "rb", "php", "sh",
];
const SPOKEN_CORRECTION_ANCHORS: &[&str] = &[
    "a", "an", "the", "some", "my", "your", "our", "to", "at", "on", "in", "for", "with", "from",
];
const VALUE_LIKE_TOKENS: &[&str] = &[
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "thirty",
    "forty",
    "fifty",
    "sixty",
    "seventy",
    "eighty",
    "ninety",
    "hundred",
    "thousand",
    "today",
    "tomorrow",
    "tonight",
    "yesterday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "morning",
    "afternoon",
    "evening",
    "night",
    "noon",
    "midnight",
    "week",
    "month",
    "year",
    "next",
    "last",
    "this",
    "coming",
    "am",
    "pm",
];
const VALUE_LIKE_CONNECTORS: &[&str] = &["and", "or"];

/// Wraps `WhisperContext` (which holds a raw C pointer) so it can be stored
/// inside `Mutex<ModelRuntimeState>`.  The underlying whisper.cpp context is
/// read-only after creation; all mutable inference state lives in the
/// per-call `WhisperState` object.  It is therefore safe to move across
/// thread boundaries.
struct SendableWhisperContext(WhisperContext);

// SAFETY: WhisperContext wraps a *mut whisper_context that is read-only after
// creation.  whisper.cpp's model data is not mutated during inference; each
// inference call creates its own `whisper_state`.  Moving the context to
// another thread is safe.
unsafe impl Send for SendableWhisperContext {}

#[derive(Default)]
pub struct ModelRuntimeState {
    pub downloading: bool,
    pub download_target: Option<SpeechModel>,
    pub last_error: Option<String>,
    /// Cached whisper context so the model file is not reloaded on every call.
    cached_context: Option<SendableWhisperContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsrProviderKind {
    Mock,
    WhisperCpp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypingStyle {
    VeryCasual,
    Formal,
    Excited,
}

#[derive(Debug, Clone)]
struct ExtractedMention {
    placeholder: String,
    query: String,
    fallback_text: String,
}

#[derive(Debug, Default)]
struct MentionTemplate {
    template: String,
    mentions: Vec<ExtractedMention>,
}

impl TypingStyle {
    fn from_setting(value: &str) -> Self {
        match value {
            "very_casual" => Self::VeryCasual,
            "excited" => Self::Excited,
            _ => Self::Formal,
        }
    }
}

pub fn transcribe_audio(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    runtime: &Mutex<ModelRuntimeState>,
    speech_model: SpeechModel,
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
        AsrProviderKind::WhisperCpp => transcribe_with_whisper_cpp(
            app_data_dir,
            resource_dir,
            runtime,
            speech_model,
            &audio,
            duration_ms,
        ),
    }
}

pub fn cleanup_text(
    raw_text: &str,
    mode: DictationMode,
    preserve_raw: bool,
    auto_backtrack: bool,
    custom_dictionary: &str,
    typing_style: &str,
) -> CleanupResponse {
    let style = TypingStyle::from_setting(typing_style);
    let normalized = normalize_text(raw_text, preserve_raw);
    let corrected = if preserve_raw || !auto_backtrack {
        normalized
    } else {
        apply_spoken_corrections(&normalized)
    };
    let text = apply_custom_dictionary(&corrected, custom_dictionary);
    let mention_template = extract_file_mentions(&text);
    let formatted_template = match mode {
        DictationMode::BulletList => format_bullets(&mention_template.template, style)
            .unwrap_or_else(|| format_sentence(&mention_template.template, mode, style)),
        DictationMode::Auto => format_auto(&mention_template.template, style),
        _ => format_sentence(&mention_template.template, mode, style),
    };
    let insertion_plan = build_insertion_plan(&formatted_template, &mention_template.mentions);
    let clean_text = apply_mentions_to_text(&formatted_template, &mention_template.mentions);

    CleanupResponse {
        clean_text,
        insertion_plan,
        provider: NATIVE_PROVIDER.to_string(),
    }
}

pub fn get_model_status(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    runtime: &Mutex<ModelRuntimeState>,
    active_model: SpeechModel,
) -> Result<ModelStatus, String> {
    let state = runtime
        .lock()
        .map_err(|_| "Model status lock poisoned.".to_string())?;
    let installed_models = available_models(app_data_dir, resource_dir);

    let error = state
        .last_error
        .clone()
        .or_else(|| model_availability_error(app_data_dir, resource_dir, active_model));

    Ok(ModelStatus {
        installed: installed_models.contains(&active_model),
        bundled_models: bundled_models().iter().map(|spec| spec.id).collect(),
        active_model,
        installed_models,
        download_target: state.download_target,
        downloading: state.downloading,
        progress: None,
        error,
    })
}

pub fn download_model(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    runtime: &Mutex<ModelRuntimeState>,
    model: SpeechModel,
) -> Result<(), String> {
    {
        let mut state = runtime
            .lock()
            .map_err(|_| "Model status lock poisoned.".to_string())?;
        state.downloading = true;
        state.download_target = Some(model);
        state.last_error = None;
    }

    let result = ensure_model_downloaded(app_data_dir, resource_dir, model).map(|_| ());

    let mut state = runtime
        .lock()
        .map_err(|_| "Model status lock poisoned.".to_string())?;
    state.downloading = false;
    state.download_target = None;
    state.last_error = result.as_ref().err().cloned();

    result
}

pub fn prewarm_model(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    runtime: &Mutex<ModelRuntimeState>,
    speech_model: SpeechModel,
) -> Result<(), String> {
    if selected_provider() != AsrProviderKind::WhisperCpp {
        return Ok(());
    }

    if let Some(error) = model_availability_error(app_data_dir, resource_dir, speech_model) {
        let mut state = runtime
            .lock()
            .map_err(|_| "Model status lock poisoned.".to_string())?;
        state.last_error = Some(error.clone());
        return Err(error);
    }

    let result = ensure_model_available(app_data_dir, resource_dir, speech_model)
        .and_then(|model_path| load_whisper_context(&model_path));

    let mut state = runtime
        .lock()
        .map_err(|_| "Model status lock poisoned.".to_string())?;
    match result {
        Ok(context) => {
            state.cached_context = Some(SendableWhisperContext(context));
            state.last_error = None;
            Ok(())
        }
        Err(error) => {
            state.last_error = Some(error.clone());
            Err(error)
        }
    }
}

fn transcribe_with_whisper_cpp(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    runtime: &Mutex<ModelRuntimeState>,
    speech_model: SpeechModel,
    audio: &[u8],
    duration_ms: i64,
) -> Result<TranscriptionResponse, String> {
    let model_path = ensure_model_available(app_data_dir, resource_dir, speech_model)?;
    let samples = decode_audio_for_whisper(audio)?;

    // Take the cached context out while we run inference (so the lock is not
    // held during the potentially long transcription call).
    let cached = {
        runtime
            .lock()
            .map_err(|_| "Model status lock poisoned.".to_string())?
            .cached_context
            .take()
    };

    let context = match cached {
        Some(ctx) => ctx,
        None => SendableWhisperContext(load_whisper_context(&model_path)?),
    };

    let raw_text = transcribe_with_model(&context.0, &samples, MODEL_LANGUAGE);

    // Return the context to the cache and clear any previous error.
    {
        let mut state = runtime
            .lock()
            .map_err(|_| "Model status lock poisoned.".to_string())?;
        if state.cached_context.is_none() {
            state.cached_context = Some(context);
        }
        state.last_error = None;
    }

    let raw_text = raw_text?;

    // Whisper emits special tokens like [BLANK_AUDIO], [MUSIC], [NOISE] for
    // non-speech audio. Strip them out before checking if anything was said.
    let raw_text = strip_whisper_special_tokens(&raw_text).trim().to_string();
    if raw_text.is_empty() {
        return Err(
            "No speech was detected in the recording. Try speaking louder or checking the selected microphone."
                .to_string(),
        );
    }

    Ok(TranscriptionResponse {
        raw_text,
        language: MODEL_LANGUAGE.to_string(),
        duration_ms,
        provider: WHISPER_CPP_PROVIDER.to_string(),
    })
}

fn decode_audio_for_whisper(audio_bytes: &[u8]) -> Result<Vec<f32>, String> {
    let mut reader = WavReader::new(Cursor::new(audio_bytes))
        .map_err(|error| format!("Unable to decode recorded audio as WAV: {error}"))?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return Err("Recorded audio did not include a valid sample rate.".to_string());
    }
    if spec.channels == 0 {
        return Err("Recorded audio did not include any channels.".to_string());
    }

    let interleaved = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .map(|sample| {
                sample
                    .map(|value| f32::from(value) / f32::from(i16::MAX))
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Unable to read recorded PCM samples: {error}"))?,
        (SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .map(|sample| {
                sample
                    .map(|value| value.clamp(-1.0, 1.0))
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Unable to read recorded PCM samples: {error}"))?,
        _ => {
            return Err(format!(
                "Recorded audio format {:?}/{}-bit is not supported yet.",
                spec.sample_format, spec.bits_per_sample
            ))
        }
    };

    let mono = downmix_to_mono(&interleaved, usize::from(spec.channels));
    Ok(resample_audio(&mono, spec.sample_rate, TARGET_SAMPLE_RATE))
}

fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }

    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
        .collect()
}

fn resample_audio(samples: &[f32], input_sample_rate: u32, output_sample_rate: u32) -> Vec<f32> {
    if samples.is_empty() || input_sample_rate == 0 || input_sample_rate == output_sample_rate {
        return samples.to_vec();
    }

    let output_len = ((samples.len() as u64 * output_sample_rate as u64) / input_sample_rate as u64)
        .max(1) as usize;
    let mut output = Vec::with_capacity(output_len);

    for index in 0..output_len {
        let position = index as f64 * input_sample_rate as f64 / output_sample_rate as f64;
        let lower_index = position.floor() as usize;
        let upper_index = (lower_index + 1).min(samples.len().saturating_sub(1));
        let fraction = (position - lower_index as f64) as f32;
        let lower = samples[lower_index];
        let upper = samples[upper_index];
        output.push(lower + ((upper - lower) * fraction));
    }

    output
}

fn strip_whisper_special_tokens(text: &str) -> String {
    // Whisper outputs bracketed non-speech tokens (e.g. [BLANK_AUDIO], [MUSIC],
    // [NOISE]) when it hears silence or non-speech audio. Remove them so they
    // are never pasted into the user's editor.
    let mut result = String::with_capacity(text.len());
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

fn transcribe_with_model(
    context: &WhisperContext,
    samples: &[f32],
    language: &str,
) -> Result<String, String> {
    let mut state = context
        .create_state()
        .map_err(|error| format!("Unable to create the selected whisper.cpp state: {error}"))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(language));
    params.set_translate(false);
    params.set_no_context(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    // Use all available cores up to 8; beyond that whisper.cpp sees diminishing returns.
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
        .min(8);
    params.set_n_threads(n_threads);

    state.full(params, samples).map_err(|error| {
        format!("The selected whisper.cpp model failed to transcribe audio: {error}")
    })?;

    let mut transcript = String::new();
    for segment in state.as_iter() {
        let text = segment.to_str_lossy().map_err(|error| {
            format!("The selected whisper.cpp segment text was invalid UTF-8: {error}")
        })?;
        transcript.push_str(text.as_ref());
    }

    Ok(transcript)
}

fn load_whisper_context(model_path: &Path) -> Result<WhisperContext, String> {
    let model_path = model_path.to_string_lossy().into_owned();
    let mut params = WhisperContextParameters::default();
    // Request GPU (Metal on macOS/Apple Silicon).  Falls back to CPU silently
    // if the whisper-rs build does not include GPU support.
    params.use_gpu(true);
    WhisperContext::new_with_params(&model_path, params).map_err(
        |error| format!("Unable to load the selected whisper.cpp model `{model_path}`: {error}"),
    )
}

fn selected_provider() -> AsrProviderKind {
    match env::var("TATOR_INTERNAL_ASR_PROVIDER").ok().as_deref() {
        Some("mlx_whisper") | Some("whisper_cpp") => AsrProviderKind::WhisperCpp,
        Some("mock") => AsrProviderKind::Mock,
        _ if cfg!(debug_assertions) => AsrProviderKind::Mock,
        _ => AsrProviderKind::WhisperCpp,
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
        DictationMode::Auto => "make an outline with one problem two solutions three pricing keep it under 100 words".to_string(),
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

fn apply_spoken_corrections(text: &str) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return String::new();
    }

    let mut corrected = Vec::with_capacity(tokens.len());
    let mut index = 0;
    let mut saw_cue = false;

    while index < tokens.len() {
        if let Some(cue_length) = spoken_correction_cue_length(&tokens, index) {
            saw_cue = true;
            let replacement_start = index + cue_length;
            let replacement_end = next_spoken_correction_cue_index(&tokens, replacement_start);
            let replacement_tokens = tokens[replacement_start..replacement_end]
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>();

            if replacement_tokens.is_empty()
                || !replace_trailing_phrase(&mut corrected, &replacement_tokens)
            {
                corrected.extend(
                    tokens[index..replacement_end]
                        .iter()
                        .map(|token| (*token).to_string()),
                );
            }

            index = replacement_end;
            continue;
        }

        corrected.push(tokens[index].to_string());
        index += 1;
    }

    if !saw_cue {
        return text.to_string();
    }

    collapse_whitespace(&corrected.join(" "))
}

fn spoken_correction_cue_length(tokens: &[&str], index: usize) -> Option<usize> {
    let word = tokens.get(index).map(|token| normalize_word(token))?;
    if word == "actually" {
        return Some(1);
    }

    if word == "i"
        && tokens
            .get(index + 1)
            .map(|token| normalize_word(token) == "mean")
            .unwrap_or(false)
    {
        return Some(2);
    }

    None
}

fn next_spoken_correction_cue_index(tokens: &[&str], start: usize) -> usize {
    for index in start..tokens.len() {
        if spoken_correction_cue_length(tokens, index).is_some() {
            return index;
        }
    }

    tokens.len()
}

fn replace_trailing_phrase(current: &mut Vec<String>, replacement: &[String]) -> bool {
    if current.is_empty() || replacement.is_empty() {
        return false;
    }

    let span = if phrase_is_value_like(replacement) {
        find_trailing_value_span(current)
    } else {
        find_anchor_span(current).or_else(|| find_trailing_content_span(current, replacement))
    };

    let Some((start, end)) = span else {
        return false;
    };

    current.splice(start..end, replacement.iter().cloned());
    true
}

fn find_trailing_value_span(tokens: &[String]) -> Option<(usize, usize)> {
    let mut start = tokens.len();
    while start > 0 && token_is_value_like_component(&tokens[start - 1]) {
        start -= 1;
    }

    if start == tokens.len() || !has_concrete_value_token(&tokens[start..]) {
        None
    } else {
        Some((start, tokens.len()))
    }
}

fn find_anchor_span(tokens: &[String]) -> Option<(usize, usize)> {
    for index in (0..tokens.len()).rev() {
        if !token_is_anchor(&tokens[index]) {
            continue;
        }

        let trailing = tokens.len().saturating_sub(index + 1);
        if (1..=3).contains(&trailing) && index >= tokens.len().saturating_sub(4) {
            return Some((index + 1, tokens.len()));
        }

        return None;
    }

    None
}

fn find_trailing_content_span(tokens: &[String], replacement: &[String]) -> Option<(usize, usize)> {
    let desired = replacement
        .iter()
        .filter(|token| token_is_content_word(token))
        .count()
        .clamp(1, 3);
    let mut start = tokens.len();
    let mut content_words = 0;

    while start > 0 {
        let previous = start - 1;
        if token_is_anchor(&tokens[previous]) {
            break;
        }

        start = previous;
        if token_is_content_word(&tokens[start]) {
            content_words += 1;
            if content_words >= desired {
                break;
            }
        }
    }

    if content_words == 0 {
        None
    } else {
        Some((start, tokens.len()))
    }
}

fn phrase_is_value_like(tokens: &[String]) -> bool {
    !tokens.is_empty()
        && tokens
            .iter()
            .all(|token| token_is_value_like_component(token))
        && has_concrete_value_token(tokens)
}

fn has_concrete_value_token(tokens: &[String]) -> bool {
    tokens.iter().any(|token| token_is_concrete_value(token))
}

fn token_is_value_like_component(token: &str) -> bool {
    let normalized = normalize_word(token);
    !normalized.is_empty()
        && (token_is_concrete_value(token) || VALUE_LIKE_CONNECTORS.contains(&normalized.as_str()))
}

fn token_is_concrete_value(token: &str) -> bool {
    let normalized = normalize_word(token);
    !normalized.is_empty()
        && (looks_numeric_like(&normalized) || VALUE_LIKE_TOKENS.contains(&normalized.as_str()))
}

fn looks_numeric_like(word: &str) -> bool {
    word.chars().any(|character| character.is_ascii_digit())
        || word.contains(':')
        || word.ends_with("am")
        || word.ends_with("pm")
}

fn token_is_anchor(token: &str) -> bool {
    let normalized = normalize_word(token);
    SPOKEN_CORRECTION_ANCHORS.contains(&normalized.as_str())
}

fn token_is_content_word(token: &str) -> bool {
    let normalized = normalize_word(token);
    !normalized.is_empty()
        && !token_is_anchor(token)
        && !VALUE_LIKE_CONNECTORS.contains(&normalized.as_str())
}

fn apply_custom_dictionary(text: &str, custom_dictionary: &str) -> String {
    parse_custom_dictionary(custom_dictionary)
        .into_iter()
        .fold(text.to_string(), |current, entry| {
            replace_phrase_case_insensitive(&current, &entry.from, &entry.to)
        })
}

fn parse_custom_dictionary(custom_dictionary: &str) -> Vec<DictionaryEntry> {
    custom_dictionary
        .lines()
        .filter_map(parse_dictionary_entry)
        .collect()
}

fn parse_dictionary_entry(line: &str) -> Option<DictionaryEntry> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let (from, to) = trimmed
        .split_once("=>")
        .or_else(|| trimmed.split_once("->"))
        .or_else(|| trimmed.split_once('='))?;

    let from = collapse_whitespace(from).trim().to_string();
    let to = collapse_whitespace(to).trim().to_string();
    if from.is_empty() || to.is_empty() {
        return None;
    }

    Some(DictionaryEntry { from, to })
}

fn replace_phrase_case_insensitive(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let mut index = 0;

    while index < text.len() {
        if phrase_matches_at(text, index, from) {
            result.push_str(to);
            index += from.len();
            continue;
        }

        let character = text[index..]
            .chars()
            .next()
            .expect("index always points to a valid character boundary");
        result.push(character);
        index += character.len_utf8();
    }

    result
}

fn phrase_matches_at(text: &str, index: usize, from: &str) -> bool {
    let Some(candidate) = text.get(index..index + from.len()) else {
        return false;
    };

    if !candidate.eq_ignore_ascii_case(from) {
        return false;
    }

    let previous = text[..index].chars().next_back();
    let next = text[index + from.len()..].chars().next();

    is_dictionary_boundary(previous) && is_dictionary_boundary(next)
}

fn is_dictionary_boundary(character: Option<char>) -> bool {
    character
        .map(|value| !value.is_alphanumeric())
        .unwrap_or(true)
}

#[derive(Debug, PartialEq, Eq)]
struct DictionaryEntry {
    from: String,
    to: String,
}

fn extract_file_mentions(text: &str) -> MentionTemplate {
    let compact = collapse_whitespace(text);
    if compact.is_empty() {
        return MentionTemplate::default();
    }

    let tokens: Vec<&str> = compact.split_whitespace().collect();
    let mut template_tokens = Vec::with_capacity(tokens.len());
    let mut mentions = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        if let Some((query, consumed)) = try_parse_explicit_file_query(&tokens, index)
            .or_else(|| try_parse_bare_basename(&tokens, index))
        {
            let placeholder = mention_placeholder(mentions.len());
            template_tokens.push(placeholder.clone());
            mentions.push(ExtractedMention {
                placeholder,
                fallback_text: query.clone(),
                query,
            });
            index += consumed;
            continue;
        }

        template_tokens.push(tokens[index].to_string());
        index += 1;
    }

    MentionTemplate {
        template: collapse_whitespace(&template_tokens.join(" ")),
        mentions,
    }
}

fn try_parse_explicit_file_query(tokens: &[&str], start: usize) -> Option<(String, usize)> {
    let mut index = start;
    let mut query = String::new();
    let mut consumed_any = false;
    let mut expect_fragment = true;
    let mut has_file_cue = false;

    while index < tokens.len() {
        if expect_fragment {
            if !consumed_any {
                if let Some(literal) = literal_file_fragment(tokens[index]) {
                    has_file_cue = literal_has_file_cue(&literal);
                    query.push_str(&literal);
                    consumed_any = true;
                    index += 1;
                    expect_fragment = false;
                    continue;
                }
            }

            let fragment = simple_file_fragment(tokens[index])?;
            query.push_str(&fragment);
            consumed_any = true;
            index += 1;
            expect_fragment = false;
            continue;
        }

        let word = normalize_word(tokens[index]);
        if let Some(separator) = spoken_separator(&word) {
            query.push_str(separator);
            has_file_cue = true;
            index += 1;
            expect_fragment = true;
            continue;
        }

        if word == "dot" {
            let extension = tokens.get(index + 1).map(|token| normalize_word(token))?;
            if is_known_file_extension(&extension) {
                query.push('.');
                query.push_str(&extension);
                has_file_cue = true;
                index += 2;
                expect_fragment = false;
                continue;
            }
        }

        break;
    }

    (consumed_any && !expect_fragment && has_file_cue).then_some((query, index - start))
}

fn try_parse_bare_basename(tokens: &[&str], start: usize) -> Option<(String, usize)> {
    if !has_file_intent(tokens, start) {
        return None;
    }

    let basename = simple_file_fragment(tokens[start])?;
    let next_word = tokens.get(start + 1).map(|token| normalize_word(token));
    if basename.len() < 2
        || !basename
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        || literal_has_file_cue(&basename)
        || next_word
            .as_deref()
            .map(|word| !word.is_empty() && !FILE_MENTION_BOUNDARY_WORDS.contains(&word))
            .unwrap_or(false)
    {
        return None;
    }

    Some((basename, 1))
}

fn simple_file_fragment(token: &str) -> Option<String> {
    let trimmed = token
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_string();
    (!trimmed.is_empty()
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then_some(trimmed)
}

fn literal_file_fragment(token: &str) -> Option<String> {
    let trimmed = token
        .trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '.' | '/' | '-' | '_')
        })
        // A trailing period is a sentence-ending period, not a path separator.
        // Strip it so that "word." is never mistaken for a file reference.
        .trim_end_matches('.')
        .to_string();
    if trimmed.is_empty() || !literal_has_file_cue(&trimmed) {
        return None;
    }

    trimmed
        .chars()
        .all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '/' | '-' | '_')
        })
        .then_some(trimmed)
}

fn literal_has_file_cue(value: &str) -> bool {
    value.contains(['.', '/', '-', '_'])
}

fn spoken_separator(word: &str) -> Option<&'static str> {
    FILE_SEPARATOR_WORDS
        .iter()
        .find_map(|(spoken, separator)| (*spoken == word).then_some(*separator))
}

fn is_known_file_extension(word: &str) -> bool {
    FILE_EXTENSION_WORDS.contains(&word)
}

fn has_file_intent(tokens: &[&str], start: usize) -> bool {
    let mut previous_words = Vec::new();
    let mut index = start;

    while index > 0 && previous_words.len() < 3 {
        index -= 1;
        let word = normalize_word(tokens[index]);
        if word.is_empty() || FILE_INTENT_SKIP_WORDS.contains(&word.as_str()) {
            continue;
        }
        previous_words.push(word);
    }

    previous_words
        .first()
        .map(|word| FILE_INTENT_CUES.contains(&word.as_str()))
        .unwrap_or(false)
        || matches!(
            previous_words.as_slice(),
            [closest, next, ..] if closest == "at" && next == "look"
        )
}

fn mention_placeholder(index: usize) -> String {
    format!("{FILE_MENTION_PLACEHOLDER_PREFIX}{index}__")
}

fn build_insertion_plan(
    formatted_template: &str,
    mentions: &[ExtractedMention],
) -> Option<Vec<CleanupInsertionSegment>> {
    if mentions.is_empty() {
        return None;
    }

    let mut remaining = formatted_template;
    let mut plan = Vec::new();

    for mention in mentions {
        let placeholder_index = remaining.find(&mention.placeholder)?;
        let before = &remaining[..placeholder_index];
        if !before.is_empty() {
            plan.push(CleanupInsertionSegment::Text {
                text: before.to_string(),
            });
        }
        plan.push(CleanupInsertionSegment::FileMention {
            query: mention.query.clone(),
            fallback_text: mention.fallback_text.clone(),
        });
        remaining = &remaining[placeholder_index + mention.placeholder.len()..];
    }

    if !remaining.is_empty() {
        plan.push(CleanupInsertionSegment::Text {
            text: remaining.to_string(),
        });
    }

    Some(plan)
}

fn apply_mentions_to_text(formatted_template: &str, mentions: &[ExtractedMention]) -> String {
    if mentions.is_empty() {
        return formatted_template.to_string();
    }

    let mut remaining = formatted_template;
    let mut output = String::new();

    for mention in mentions {
        let Some(placeholder_index) = remaining.find(&mention.placeholder) else {
            return formatted_template.to_string();
        };
        output.push_str(&remaining[..placeholder_index]);
        output.push_str(&mention.fallback_text);
        remaining = &remaining[placeholder_index + mention.placeholder.len()..];
    }

    output.push_str(remaining);
    output
}

fn is_placeholder_token(token: &str) -> bool {
    token.starts_with(FILE_MENTION_PLACEHOLDER_PREFIX) && token.ends_with("__")
}

fn is_only_placeholder_text(text: &str) -> bool {
    let mut tokens = text.split_whitespace();
    matches!(
        (tokens.next(), tokens.next()),
        (Some(token), None) if is_placeholder_token(token)
    )
}

fn first_non_placeholder_word(text: &str) -> String {
    text.split_whitespace()
        .filter(|token| !is_placeholder_token(token))
        .find_map(|token| {
            let normalized = normalize_word(token);
            (!normalized.is_empty()).then_some(normalized)
        })
        .unwrap_or_default()
}

fn placeholder_free_lowercase(text: &str) -> String {
    text.split_whitespace()
        .filter(|token| !is_placeholder_token(token))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn cardinal_value(word: &str) -> Option<usize> {
    match word {
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        "eleven" => Some(11),
        "twelve" => Some(12),
        _ => None,
    }
}

fn ordinal_value(word: &str) -> Option<usize> {
    match word {
        "first" => Some(1),
        "second" => Some(2),
        "third" => Some(3),
        "fourth" => Some(4),
        "fifth" => Some(5),
        "sixth" => Some(6),
        "seventh" => Some(7),
        "eighth" => Some(8),
        "ninth" => Some(9),
        "tenth" => Some(10),
        "eleventh" => Some(11),
        "twelfth" => Some(12),
        _ => None,
    }
}

fn number_word_value(word: &str) -> Option<usize> {
    cardinal_value(word).or_else(|| ordinal_value(word))
}

fn detect_inline_numbered_list(text: &str) -> Option<(String, Vec<String>, String)> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    // Find the first token that maps to the value 1
    let start_index = tokens
        .iter()
        .position(|token| number_word_value(&normalize_word(token)) == Some(1))?;

    let prefix = tokens[..start_index].join(" ");
    let mut items: Vec<String> = Vec::new();
    let mut suffix = String::new();
    let mut index = start_index;
    let mut expected = 1usize;
    let mut current_item_words: Vec<&str> = Vec::new();

    // Skip the first number word (already confirmed == 1)
    index += 1;

    while index < tokens.len() {
        let word = normalize_word(tokens[index]);

        // Check if this word is the next expected number
        if number_word_value(&word) == Some(expected + 1) {
            // Save the current item (strip trailing period from last word)
            let item = join_item_words(&current_item_words);
            items.push(item);
            current_item_words.clear();
            expected += 1;
            index += 1;
            continue;
        }

        // Check if this word ends with a period (sentence boundary)
        if tokens[index].ends_with('.') {
            // Include this word (stripped of trailing period) in the current item
            let word_trimmed = tokens[index].trim_end_matches('.');
            if !word_trimmed.is_empty() {
                current_item_words.push(word_trimmed);
            }
            let item = join_item_words(&current_item_words);
            items.push(item);
            current_item_words.clear();
            index += 1;
            // Everything after is suffix
            suffix = tokens[index..].join(" ");
            break;
        }

        current_item_words.push(tokens[index]);
        index += 1;
    }

    // If we still have words in current_item_words, finalize last item
    if !current_item_words.is_empty() {
        items.push(join_item_words(&current_item_words));
    }

    if items.len() < 2 {
        return None;
    }

    Some((prefix, items, suffix))
}

fn join_item_words(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| w.trim_end_matches('.'))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_auto(text: &str, style: TypingStyle) -> String {
    let Some((prefix, items, suffix)) = detect_inline_numbered_list(text) else {
        return format_sentence(text, DictationMode::Default, style);
    };

    // Format prefix: sentence case, strip trailing period, append ":"
    let formatted_prefix = {
        let cased = sentence_case(&prefix);
        let stripped = cased.trim_end_matches('.').to_string();
        format!("{stripped}:")
    };

    // Format numbered items
    let formatted_items = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let formatted = format_list_item(item.trim_matches(|c| c == ' ' || c == '.'), style);
            format!("{}. {formatted}", i + 1)
        })
        .collect::<Vec<_>>()
        .join("\n");

    if suffix.is_empty() {
        format!("{formatted_prefix}\n\n{formatted_items}")
    } else {
        let formatted_suffix = format_sentence(&suffix, DictationMode::Default, style);
        format!("{formatted_prefix}\n\n{formatted_items}\n\n{formatted_suffix}")
    }
}

fn format_bullets(text: &str, style: TypingStyle) -> Option<String> {
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
            format_list_item(
                item.trim_matches(|character| character == ' ' || character == '.'),
                style,
            )
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

fn format_sentence(text: &str, mode: DictationMode, style: TypingStyle) -> String {
    let mut sentence = match style {
        TypingStyle::VeryCasual => collapse_whitespace(text),
        TypingStyle::Formal | TypingStyle::Excited => sentence_case(text),
    };
    if sentence.is_empty() {
        return sentence;
    }

    if is_only_placeholder_text(&sentence) {
        return sentence;
    }

    if matches!(style, TypingStyle::VeryCasual) {
        return sentence;
    }

    let opener = first_non_placeholder_word(&sentence);
    let is_question =
        !matches!(mode, DictationMode::Chat) && QUESTION_OPENERS.contains(&opener.as_str());

    if matches!(style, TypingStyle::Excited) {
        return excited_sentence(sentence, is_question);
    }

    // Strip a trailing period that the speech-to-text model may have already
    // added, then apply the correct terminal punctuation ourselves.  This
    // prevents "sentence.." when whisper.cpp outputs a period and we add one.
    if sentence.ends_with('.') {
        sentence.pop();
    }

    // Don't add terminal punctuation when the sentence ends with a file mention
    // placeholder — the rendered @filename already looks complete on its own.
    let ends_with_file_mention = sentence
        .split_whitespace()
        .last()
        .is_some_and(is_placeholder_token);

    if !ends_with_file_mention && !sentence.ends_with(['!', '?']) {
        let punctuation = if !is_question { '.' } else { '?' };
        sentence.push(punctuation);
    }

    sentence
}

fn format_list_item(text: &str, style: TypingStyle) -> String {
    match style {
        TypingStyle::VeryCasual => collapse_whitespace(text),
        TypingStyle::Formal | TypingStyle::Excited => sentence_case(text),
    }
}

fn excited_sentence(mut sentence: String, is_question: bool) -> String {
    if sentence.ends_with('!') {
        return sentence;
    }

    if sentence.ends_with('?') {
        return sentence;
    }

    if sentence.ends_with('.') {
        if should_use_excited_punctuation(&sentence) {
            sentence.pop();
            sentence.push('!');
        }
        return sentence;
    }

    if is_question {
        sentence.push('?');
    } else if should_use_excited_punctuation(&sentence) {
        sentence.push('!');
    } else {
        sentence.push('.');
    }

    sentence
}

fn should_use_excited_punctuation(sentence: &str) -> bool {
    let lowered = placeholder_free_lowercase(sentence);
    let opener = first_non_placeholder_word(&lowered);

    EXCITED_OPENERS.contains(&opener.as_str())
        || EXCITED_PREFIXES
            .iter()
            .any(|prefix| lowered.starts_with(prefix))
}

fn sentence_case(text: &str) -> String {
    let compact = collapse_whitespace(text);
    if compact.is_empty() {
        return compact;
    }

    let mut tokens = compact
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    for token in &mut tokens {
        if is_placeholder_token(token) {
            continue;
        }

        if let Some((index, character)) = token
            .char_indices()
            .find(|(_, character)| character.is_ascii_alphabetic())
        {
            let end = index + character.len_utf8();
            token.replace_range(index..end, &character.to_uppercase().to_string());
            break;
        }
    }

    tokens.join(" ")
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
    app_data_dir.join("models").join(MODEL_INSTALL_DIR)
}

fn model_is_installed(app_data_dir: &Path, model: SpeechModel) -> bool {
    model_path(app_data_dir, model)
        .metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn model_is_available(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    model: SpeechModel,
) -> bool {
    model_is_installed(app_data_dir, model)
        || bundled_model_source_path(resource_dir, model).is_some()
}

fn available_models(app_data_dir: &Path, resource_dir: Option<&Path>) -> Vec<SpeechModel> {
    all_models()
        .iter()
        .filter_map(|spec| {
            model_is_available(app_data_dir, resource_dir, spec.id).then_some(spec.id)
        })
        .collect()
}

fn model_path(app_data_dir: &Path, model: SpeechModel) -> PathBuf {
    model_directory(app_data_dir).join(&find_model(model).file_name)
}

fn ensure_model_available(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    model: SpeechModel,
) -> Result<PathBuf, String> {
    remove_unknown_installed_models(app_data_dir)?;

    let installed_model = model_path(app_data_dir, model);
    if model_is_installed(app_data_dir, model) {
        return Ok(installed_model);
    }

    let spec = find_model(model);
    if !spec.bundled {
        return Err(optional_model_install_error(model));
    }

    let source_model = bundled_model_source_path(resource_dir, model)
        .ok_or_else(|| missing_bundled_model_error(model))?;

    install_bundled_model(app_data_dir, &source_model, spec)
}

fn ensure_model_downloaded(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    model: SpeechModel,
) -> Result<PathBuf, String> {
    remove_unknown_installed_models(app_data_dir)?;

    if model_is_installed(app_data_dir, model) {
        return Ok(model_path(app_data_dir, model));
    }

    let spec = find_model(model);
    if spec.bundled {
        return ensure_model_available(app_data_dir, resource_dir, model);
    }

    download_model_to_install(app_data_dir, spec)
}

fn install_bundled_model(
    app_data_dir: &Path,
    source_model: &Path,
    spec: &WhisperModelSpec,
) -> Result<PathBuf, String> {
    let model_dir = model_directory(app_data_dir);
    let installed_model = model_dir.join(&spec.file_name);
    fs::create_dir_all(&model_dir)
        .map_err(|error| format!("Unable to create the model directory: {error}"))?;
    fs::copy(source_model, &installed_model).map_err(|error| {
        format!(
            "Unable to copy the bundled whisper.cpp model from `{}` to `{}`: {error}",
            source_model.display(),
            installed_model.display()
        )
    })?;

    Ok(installed_model)
}

fn download_model_to_install(
    app_data_dir: &Path,
    spec: &WhisperModelSpec,
) -> Result<PathBuf, String> {
    let client = Client::builder()
        .build()
        .map_err(|error| format!("Unable to prepare the whisper.cpp downloader: {error}"))?;
    let response = client
        .get(&spec.url)
        .send()
        .map_err(|error| {
            format!(
                "Unable to download the optional whisper.cpp model `{}`: {error}",
                spec.file_name
            )
        })?
        .error_for_status()
        .map_err(|error| {
            format!(
                "Unable to download the optional whisper.cpp model `{}`: {error}",
                spec.file_name
            )
        })?;

    install_model_from_reader(app_data_dir, spec, response)
}

fn install_model_from_reader<R: Read>(
    app_data_dir: &Path,
    spec: &WhisperModelSpec,
    mut reader: R,
) -> Result<PathBuf, String> {
    let model_dir = model_directory(app_data_dir);
    let installed_model = model_dir.join(&spec.file_name);
    let temp_path = model_dir.join(format!("{}.download", spec.file_name));

    fs::create_dir_all(&model_dir)
        .map_err(|error| format!("Unable to create the model directory: {error}"))?;
    let _ = fs::remove_file(&temp_path);

    let write_result = (|| -> Result<(), String> {
        let mut output = File::create(&temp_path)
            .map_err(|error| format!("Unable to prepare the model download file: {error}"))?;
        let mut hasher = Sha1::new();
        let mut buffer = [0_u8; 16 * 1024];

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|error| {
                format!(
                    "Unable to download the optional whisper.cpp model `{}`: {error}",
                    spec.file_name
                )
            })?;
            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
            output.write_all(&buffer[..bytes_read]).map_err(|error| {
                format!(
                    "Unable to write the optional whisper.cpp model `{}`: {error}",
                    spec.file_name
                )
            })?;
        }

        output
            .flush()
            .map_err(|error| format!("Unable to finalize the model download file: {error}"))?;

        let digest = format!("{:x}", hasher.finalize());
        if digest != spec.sha1 {
            return Err(format!(
                "Downloaded whisper.cpp model `{}` but the SHA1 did not match. Expected {}, received {}.",
                spec.file_name, spec.sha1, digest
            ));
        }

        if installed_model.exists() {
            fs::remove_file(&installed_model).map_err(|error| {
                format!(
                    "Unable to replace the existing whisper.cpp model `{}`: {error}",
                    installed_model.display()
                )
            })?;
        }

        fs::rename(&temp_path, &installed_model).map_err(|error| {
            format!(
                "Unable to install the whisper.cpp model `{}`: {error}",
                installed_model.display()
            )
        })?;

        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    Ok(installed_model)
}

fn remove_unknown_installed_models(app_data_dir: &Path) -> Result<(), String> {
    let model_dir = model_directory(app_data_dir);
    if !model_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(&model_dir)
        .map_err(|error| format!("Unable to inspect the model directory: {error}"))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Unable to inspect the model directory: {error}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.starts_with("ggml-")
            || !file_name.ends_with(".bin")
            || is_known_model_file(file_name)
        {
            continue;
        }

        fs::remove_file(&path).map_err(|error| {
            format!(
                "Unable to remove the stale whisper.cpp model `{}`: {error}",
                path.display()
            )
        })?;
    }

    Ok(())
}

fn bundled_model_source_path(resource_dir: Option<&Path>, model: SpeechModel) -> Option<PathBuf> {
    if !find_model(model).bundled {
        return None;
    }

    resource_dir
        .map(|resource_dir| resource_model_path(resource_dir, model))
        .filter(|path| path.is_file())
        .or_else(|| resolve_dev_bundled_model(model))
}

fn model_availability_error(
    app_data_dir: &Path,
    resource_dir: Option<&Path>,
    model: SpeechModel,
) -> Option<String> {
    if model_is_available(app_data_dir, resource_dir, model) {
        None
    } else if find_model(model).bundled {
        Some(missing_bundled_model_error(model))
    } else {
        Some(optional_model_install_error(model))
    }
}

fn missing_bundled_model_error(model: SpeechModel) -> String {
    format!(
        "This build does not include the bundled whisper.cpp model `{}`. Run `npm run prepare:whisper-assets` before `tauri dev` or `tauri build` so the model is packaged into the desktop app.",
        find_model(model).file_name
    )
}

fn optional_model_install_error(model: SpeechModel) -> String {
    let spec = find_model(model);
    let size_mb = (spec.size_bytes as f64 / 1_000_000.0).round() as u64;
    format!(
        "The optional whisper.cpp model `{}` is not installed yet. Download the roughly {size_mb} MB model from Settings before selecting it.",
        spec.file_name
    )
}

fn resource_model_path(resource_dir: &Path, model: SpeechModel) -> PathBuf {
    resource_dir
        .join(MODEL_ASSET_DIR)
        .join(&find_model(model).file_name)
}

fn resolve_dev_bundled_model(model: SpeechModel) -> Option<PathBuf> {
    if !find_model(model).bundled {
        return None;
    }

    [PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("bundled-whisper-model")
        .join(&find_model(model).file_name)]
    .into_iter()
    .find(|candidate| candidate.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Read};

    use uuid::Uuid;

    #[test]
    fn strip_whisper_special_tokens_removes_blank_audio() {
        assert_eq!(strip_whisper_special_tokens("[BLANK_AUDIO]"), "");
        assert_eq!(strip_whisper_special_tokens(" [BLANK_AUDIO] "), "  ");
        assert_eq!(strip_whisper_special_tokens("[MUSIC]hello[NOISE]"), "hello");
        assert_eq!(strip_whisper_special_tokens("hello world"), "hello world");
        assert_eq!(strip_whisper_special_tokens(""), "");
    }

    #[test]
    fn cleanup_formats_sentences() {
        let result = cleanup_text(
            "um can you send the update tomorrow",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Can you send the update tomorrow?");
    }

    #[test]
    fn cleanup_formats_chat_messages() {
        let result = cleanup_text(
            "running five minutes late be there soon",
            DictationMode::Chat,
            false,
            false,
            "",
            "formal",
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
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "- Eggs\n- Milk\n- Bread\n- Apples");
    }

    #[test]
    fn cleanup_preserves_raw_text_when_requested() {
        let result = cleanup_text(
            " um hello there ",
            DictationMode::Default,
            true,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Um hello there.");
    }

    #[test]
    fn cleanup_can_return_empty_text_for_empty_input() {
        let result = cleanup_text("   ", DictationMode::Default, false, false, "", "formal");
        assert_eq!(result.clean_text, "");
    }

    #[test]
    fn cleanup_applies_custom_dictionary_replacements() {
        let result = cleanup_text(
            "send the update to rebecca tomorrow",
            DictationMode::Default,
            false,
            false,
            "Rebecca => Rebekah",
            "formal",
        );
        assert_eq!(result.clean_text, "Send the update to Rebekah tomorrow.");
    }

    #[test]
    fn cleanup_replaces_whole_phrases_only() {
        let result = cleanup_text(
            "rebecca told rebeccas about new york city hall",
            DictationMode::Default,
            false,
            false,
            "Rebecca => Rebekah\nnew york => NYC",
            "formal",
        );
        assert_eq!(
            result.clean_text,
            "Rebekah told rebeccas about NYC city hall."
        );
    }

    #[test]
    fn cleanup_auto_backtrack_replaces_trailing_value_spans() {
        let result = cleanup_text(
            "let's meet at 2 actually 3",
            DictationMode::Default,
            false,
            true,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Let's meet at 3.");
    }

    #[test]
    fn cleanup_auto_backtrack_replaces_trailing_phrase_after_anchor() {
        let result = cleanup_text(
            "i want to get some chicken i mean potatoes",
            DictationMode::Default,
            false,
            true,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "I want to get some potatoes.");
    }

    #[test]
    fn cleanup_auto_backtrack_processes_multiple_corrections_left_to_right() {
        let result = cleanup_text(
            "let's meet at 2 actually 3 i mean 4",
            DictationMode::Default,
            false,
            true,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Let's meet at 4.");
    }

    #[test]
    fn cleanup_auto_backtrack_can_be_disabled() {
        let result = cleanup_text(
            "let's meet at 2 actually 3",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Let's meet at 2 actually 3.");
    }

    #[test]
    fn cleanup_auto_backtrack_is_skipped_for_preserved_raw_text() {
        let result = cleanup_text(
            "let's meet at 2 actually 3",
            DictationMode::Default,
            true,
            true,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Let's meet at 2 actually 3.");
    }

    #[test]
    fn cleanup_auto_backtrack_falls_back_when_no_replaceable_span_exists() {
        let result = cleanup_text(
            "actually potatoes",
            DictationMode::Default,
            false,
            true,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Actually potatoes.");
    }

    #[test]
    fn cleanup_auto_backtrack_runs_before_custom_dictionary() {
        let result = cleanup_text(
            "send the update to rebecca actually katherine",
            DictationMode::Default,
            false,
            true,
            "Katherine => Kathryn",
            "formal",
        );
        assert_eq!(result.clean_text, "Send the update to Kathryn.");
    }

    #[test]
    fn cleanup_formats_very_casual_style() {
        let result = cleanup_text(
            "um can you send the update tomorrow",
            DictationMode::Default,
            false,
            false,
            "",
            "very_casual",
        );
        assert_eq!(result.clean_text, "can you send the update tomorrow");
    }

    #[test]
    fn cleanup_formats_excited_style() {
        let result = cleanup_text(
            "can you send the update tomorrow",
            DictationMode::Default,
            false,
            false,
            "",
            "excited",
        );
        assert_eq!(result.clean_text, "Can you send the update tomorrow?");
    }

    #[test]
    fn cleanup_uses_exclamation_for_upbeat_excited_style() {
        let result = cleanup_text(
            "thanks for the quick turnaround",
            DictationMode::Default,
            false,
            false,
            "",
            "excited",
        );
        assert_eq!(result.clean_text, "Thanks for the quick turnaround!");
    }

    #[test]
    fn cleanup_formats_very_casual_bullet_lists() {
        let result = cleanup_text(
            "make a list eggs milk bread and apples",
            DictationMode::BulletList,
            false,
            false,
            "",
            "very_casual",
        );
        assert_eq!(result.clean_text, "- eggs\n- milk\n- bread\n- apples");
    }

    #[test]
    fn cleanup_extracts_explicit_spoken_paths_into_an_insertion_plan() {
        let result = cleanup_text(
            "please open src slash app dot tsx",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Please open src/app.tsx");
        assert_eq!(
            result.insertion_plan,
            Some(vec![
                CleanupInsertionSegment::Text {
                    text: "Please open ".to_string(),
                },
                CleanupInsertionSegment::FileMention {
                    query: "src/app.tsx".to_string(),
                    fallback_text: "src/app.tsx".to_string(),
                },
            ])
        );
    }

    #[test]
    fn cleanup_extracts_spoken_separator_file_names() {
        let result = cleanup_text(
            "use use dash dictation dash controller dot ts",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Use use-dictation-controller.ts");
        assert_eq!(
            result.insertion_plan,
            Some(vec![
                CleanupInsertionSegment::Text {
                    text: "Use ".to_string(),
                },
                CleanupInsertionSegment::FileMention {
                    query: "use-dictation-controller.ts".to_string(),
                    fallback_text: "use-dictation-controller.ts".to_string(),
                },
            ])
        );
    }

    #[test]
    fn cleanup_extracts_bare_basenames_after_file_intent_cues() {
        let result = cleanup_text(
            "open readme",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Open readme");
        assert_eq!(
            result.insertion_plan,
            Some(vec![
                CleanupInsertionSegment::Text {
                    text: "Open ".to_string(),
                },
                CleanupInsertionSegment::FileMention {
                    query: "readme".to_string(),
                    fallback_text: "readme".to_string(),
                },
            ])
        );
    }

    #[test]
    fn cleanup_supports_multiple_file_mentions() {
        let result = cleanup_text(
            "please open readme dot md and src slash app dot tsx",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Please open readme.md and src/app.tsx");
        assert_eq!(result.insertion_plan.as_ref().map(Vec::len), Some(4));
    }

    #[test]
    fn cleanup_does_not_extract_vague_multi_word_phrases() {
        let result = cleanup_text(
            "please check the auth service tomorrow",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Please check the auth service tomorrow.");
        assert!(result.insertion_plan.is_none());
    }

    #[test]
    fn cleanup_preserves_bare_file_references_without_trailing_punctuation() {
        let result = cleanup_text(
            "readme dot md",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "readme.md");
        assert_eq!(
            result.insertion_plan,
            Some(vec![CleanupInsertionSegment::FileMention {
                query: "readme.md".to_string(),
                fallback_text: "readme.md".to_string(),
            }])
        );
    }

    #[test]
    fn cleanup_does_not_double_period_when_model_adds_trailing_period() {
        // whisper.cpp often appends a period; the cleanup must not add another.
        let result = cleanup_text(
            "running five minutes late be there soon.",
            DictationMode::Chat,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(
            result.clean_text,
            "Running five minutes late be there soon."
        );
    }

    #[test]
    fn cleanup_question_opener_overrides_whisper_period() {
        // If whisper adds a period but the sentence is a question, use '?'.
        let result = cleanup_text(
            "can you send the update tomorrow.",
            DictationMode::Default,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Can you send the update tomorrow?");
    }

    #[test]
    fn model_status_reports_bundled_models_by_default() {
        let runtime = Mutex::new(ModelRuntimeState::default());
        let root = temp_root("status-base");
        let app_data_dir = root.join("app-data");
        let resource_dir = root.join("resources");
        write_bundled_model_resource(&resource_dir, SpeechModel::BaseEn, b"base model");
        write_bundled_model_resource(&resource_dir, SpeechModel::SmallEn, b"small model");

        let status = get_model_status(
            &app_data_dir,
            Some(&resource_dir),
            &runtime,
            SpeechModel::BaseEn,
        )
        .unwrap();

        assert!(status.installed);
        assert_eq!(
            status.bundled_models,
            vec![SpeechModel::BaseEn, SpeechModel::SmallEn]
        );
        assert_eq!(status.active_model, SpeechModel::BaseEn);
        assert_eq!(
            status.installed_models,
            vec![SpeechModel::BaseEn, SpeechModel::SmallEn]
        );
        assert!(status.download_target.is_none());
        assert!(!status.downloading);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn model_status_lists_small_when_present() {
        let runtime = Mutex::new(ModelRuntimeState::default());
        let root = temp_root("status-small");
        let app_data_dir = root.join("app-data");
        let resource_dir = root.join("resources");
        let installed_dir = model_directory(&app_data_dir);

        write_bundled_model_resource(&resource_dir, SpeechModel::BaseEn, b"base model");
        std::fs::create_dir_all(&installed_dir).unwrap();
        std::fs::write(
            installed_dir.join(&find_model(SpeechModel::SmallEn).file_name),
            b"small model",
        )
        .unwrap();

        let status = get_model_status(
            &app_data_dir,
            Some(&resource_dir),
            &runtime,
            SpeechModel::BaseEn,
        )
        .unwrap();

        assert_eq!(
            status.installed_models,
            vec![SpeechModel::BaseEn, SpeechModel::SmallEn]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn selected_provider_accepts_whisper_cpp_and_legacy_mlx_names() {
        env::set_var("TATOR_INTERNAL_ASR_PROVIDER", "whisper_cpp");
        assert_eq!(selected_provider(), AsrProviderKind::WhisperCpp);

        env::set_var("TATOR_INTERNAL_ASR_PROVIDER", "mlx_whisper");
        assert_eq!(selected_provider(), AsrProviderKind::WhisperCpp);

        env::remove_var("TATOR_INTERNAL_ASR_PROVIDER");
    }

    #[test]
    fn bundled_model_availability_error_reports_missing_model_assets() {
        let app_data_dir = Path::new("/tmp/tator-missing-model-app-data");
        let message = model_availability_error(app_data_dir, None, SpeechModel::BaseEn);

        if resolve_dev_bundled_model(SpeechModel::BaseEn).is_some() {
            assert!(message.is_none());
        } else {
            let message = message.unwrap();
            assert!(message.contains("bundled whisper.cpp model"));
            assert!(message.contains("prepare:whisper-assets"));
        }
    }

    #[test]
    fn bundled_small_model_reports_missing_model_assets() {
        let message = model_availability_error(
            Path::new("/tmp/tator-missing-small-model"),
            None,
            SpeechModel::SmallEn,
        );

        if resolve_dev_bundled_model(SpeechModel::SmallEn).is_some() {
            assert!(message.is_none());
        } else {
            let message = message.unwrap();
            assert!(message.contains("bundled whisper.cpp model"));
            assert!(message.contains(&find_model(SpeechModel::SmallEn).file_name));
        }
    }

    #[test]
    fn bundled_model_source_path_prefers_packaged_resources() {
        let root = temp_root("bundled-source");
        let resource_dir = root.join("resources");
        write_bundled_model_resource(&resource_dir, SpeechModel::BaseEn, b"base model");

        let source_path =
            bundled_model_source_path(Some(&resource_dir), SpeechModel::BaseEn).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        assert!(source_path.ends_with(&find_model(SpeechModel::BaseEn).file_name));
        assert!(source_path.to_string_lossy().contains(MODEL_ASSET_DIR));
    }

    #[test]
    fn ensure_model_available_keeps_supported_models_and_prunes_legacy_files() {
        let root = temp_root("model-prune");
        let app_data_dir = root.join("app-data");
        let resource_dir = root.join("resources");
        let installed_dir = model_directory(&app_data_dir);

        write_bundled_model_resource(&resource_dir, SpeechModel::BaseEn, b"base model");
        std::fs::create_dir_all(&installed_dir).unwrap();
        std::fs::write(
            installed_dir.join(&find_model(SpeechModel::SmallEn).file_name),
            b"small model",
        )
        .unwrap();
        std::fs::write(installed_dir.join("ggml-tiny.en.bin"), b"tiny model").unwrap();

        let installed_path =
            ensure_model_available(&app_data_dir, Some(&resource_dir), SpeechModel::BaseEn)
                .unwrap();

        assert!(installed_path.ends_with(&find_model(SpeechModel::BaseEn).file_name));
        assert!(installed_path.exists());
        assert!(installed_dir
            .join(&find_model(SpeechModel::SmallEn).file_name)
            .exists());
        assert!(!installed_dir.join("ggml-tiny.en.bin").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_model_from_reader_adds_small_without_removing_base() {
        let root = temp_root("reader-install");
        let app_data_dir = root.join("app-data");
        let installed_dir = model_directory(&app_data_dir);
        let small_bytes = b"small model bytes";
        let small_spec = test_spec(SpeechModel::SmallEn, small_bytes);

        std::fs::create_dir_all(&installed_dir).unwrap();
        std::fs::write(
            installed_dir.join(&find_model(SpeechModel::BaseEn).file_name),
            b"base model",
        )
        .unwrap();

        let installed_path =
            install_model_from_reader(&app_data_dir, &small_spec, Cursor::new(small_bytes))
                .unwrap();

        assert!(installed_dir
            .join(&find_model(SpeechModel::BaseEn).file_name)
            .exists());
        assert!(installed_path.exists());
        assert!(installed_path.ends_with(&find_model(SpeechModel::SmallEn).file_name));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_model_from_reader_cleans_temp_file_on_failure() {
        let root = temp_root("reader-failure");
        let app_data_dir = root.join("app-data");
        let installed_dir = model_directory(&app_data_dir);
        let spec = test_spec(SpeechModel::SmallEn, b"small model bytes");
        let temp_path = installed_dir.join(format!("{}.download", spec.file_name));

        let error =
            install_model_from_reader(&app_data_dir, &spec, BrokenReader::default()).unwrap_err();

        assert!(error.contains("Unable to download the optional whisper.cpp model"));
        assert!(!temp_path.exists());
        assert!(!installed_dir.join(&spec.file_name).exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resample_audio_downsamples_to_target_rate() {
        let samples = vec![0.0_f32, 0.5, -0.5, 1.0];
        let resampled = resample_audio(&samples, 32_000, 16_000);
        assert_eq!(resampled.len(), 2);
        assert_eq!(resampled[0], 0.0);
    }

    #[test]
    fn decode_audio_for_whisper_downmixes_and_resamples() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: 32_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            for sample in [0_i16, 1000, -1000, 2000, 0, 500, -500, 1000] {
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();
        }

        let samples = decode_audio_for_whisper(&cursor.into_inner()).unwrap();
        assert_eq!(samples.len(), 2);
        assert!(samples.iter().all(|sample| (-1.0..=1.0).contains(sample)));
    }

    fn temp_root(label: &str) -> PathBuf {
        env::temp_dir().join(format!("tator-{label}-{}", Uuid::new_v4()))
    }

    fn write_bundled_model_resource(resource_dir: &Path, model: SpeechModel, bytes: &[u8]) {
        let bundled_dir = resource_dir.join(MODEL_ASSET_DIR);
        std::fs::create_dir_all(&bundled_dir).unwrap();
        std::fs::write(bundled_dir.join(&find_model(model).file_name), bytes).unwrap();
    }

    fn test_spec(model: SpeechModel, bytes: &[u8]) -> WhisperModelSpec {
        let mut spec = find_model(model).clone();
        spec.sha1 = sha1_hex(bytes);
        spec
    }

    fn sha1_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[derive(Default)]
    struct BrokenReader {
        emitted_chunk: bool,
    }

    impl Read for BrokenReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.emitted_chunk {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "simulated failure",
                ));
            }

            let chunk = b"partial";
            buf[..chunk.len()].copy_from_slice(chunk);
            self.emitted_chunk = true;
            Ok(chunk.len())
        }
    }

    #[test]
    fn detect_inline_numbered_list_parses_cardinal_with_suffix() {
        let result = detect_inline_numbered_list(
            "make an outline with one problem two solutions three pricing. keep it under 100 words",
        );
        assert!(result.is_some());
        let (prefix, items, suffix) = result.unwrap();
        assert_eq!(prefix, "make an outline with");
        assert_eq!(items, vec!["problem", "solutions", "pricing"]);
        assert_eq!(suffix, "keep it under 100 words");
    }

    #[test]
    fn detect_inline_numbered_list_returns_none_for_single_number() {
        let result = detect_inline_numbered_list("one thing");
        assert!(result.is_none());
    }

    #[test]
    fn detect_inline_numbered_list_returns_none_for_no_numbers() {
        let result = detect_inline_numbered_list("hello world");
        assert!(result.is_none());
    }

    #[test]
    fn detect_inline_numbered_list_parses_ordinal_form() {
        let result =
            detect_inline_numbered_list("first problem second solution third pricing");
        assert!(result.is_some());
        let (prefix, items, suffix) = result.unwrap();
        assert_eq!(prefix, "");
        assert_eq!(items, vec!["problem", "solution", "pricing"]);
        assert_eq!(suffix, "");
    }

    #[test]
    fn detect_inline_numbered_list_handles_no_suffix() {
        let result = detect_inline_numbered_list("one eggs two milk three bread");
        assert!(result.is_some());
        let (prefix, items, suffix) = result.unwrap();
        assert_eq!(prefix, "");
        assert_eq!(items, vec!["eggs", "milk", "bread"]);
        assert_eq!(suffix, "");
    }

    #[test]
    fn cleanup_auto_mode_formats_inline_numbered_list() {
        let result = cleanup_text(
            "make an outline with one problem two solutions three pricing. keep it under 100 words",
            DictationMode::Auto,
            false,
            false,
            "",
            "formal",
        );
        assert!(result.clean_text.contains("1. Problem"));
        assert!(result.clean_text.contains("2. Solutions"));
        assert!(result.clean_text.contains("3. Pricing"));
        assert!(result.clean_text.starts_with("Make an outline with:"));
        assert!(result.clean_text.contains("Keep it under 100 words."));
    }

    #[test]
    fn cleanup_auto_mode_falls_back_to_sentence_when_no_list() {
        let result = cleanup_text(
            "send the update tomorrow",
            DictationMode::Auto,
            false,
            false,
            "",
            "formal",
        );
        assert_eq!(result.clean_text, "Send the update tomorrow.");
    }
}
