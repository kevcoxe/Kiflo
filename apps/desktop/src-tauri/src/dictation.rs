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
    models::{CleanupResponse, DictationMode, ModelStatus, SpeechModel, TranscriptionResponse},
    whisper_models::{all_models, bundled_model, find_model, is_known_model_file, WhisperModelSpec},
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

#[derive(Debug, Default)]
pub struct ModelRuntimeState {
    pub downloading: bool,
    pub download_target: Option<SpeechModel>,
    pub last_error: Option<String>,
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
    let clean_text = if matches!(mode, DictationMode::BulletList) {
        format_bullets(&text, style).unwrap_or_else(|| format_sentence(&text, mode, style))
    } else {
        format_sentence(&text, mode, style)
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
        bundled_model: bundled_model().id,
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
        .and_then(|model_path| load_whisper_context(&model_path).map(|_| ()));

    let mut state = runtime
        .lock()
        .map_err(|_| "Model status lock poisoned.".to_string())?;
    match result {
        Ok(()) => {
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
    let raw_text = transcribe_with_model(&model_path, &samples, MODEL_LANGUAGE)?;

    {
        let mut state = runtime
            .lock()
            .map_err(|_| "Model status lock poisoned.".to_string())?;
        state.last_error = None;
    }

    let raw_text = raw_text.trim().to_string();
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

fn transcribe_with_model(
    model_path: &Path,
    samples: &[f32],
    language: &str,
) -> Result<String, String> {
    let context = load_whisper_context(model_path)?;
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

    state
        .full(params, samples)
        .map_err(|error| format!("The selected whisper.cpp model failed to transcribe audio: {error}"))?;

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
    WhisperContext::new_with_params(&model_path, WhisperContextParameters::default()).map_err(
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

    if matches!(style, TypingStyle::VeryCasual) {
        return sentence;
    }

    let opener = sentence
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase();
    let is_question =
        !matches!(mode, DictationMode::Chat) && QUESTION_OPENERS.contains(&opener.as_str());

    if matches!(style, TypingStyle::Excited) {
        return excited_sentence(sentence, is_question);
    }

    if !sentence.ends_with(['.', '!', '?']) {
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
    let lowered = sentence.to_lowercase();
    let opener = lowered.split_whitespace().next().unwrap_or_default();

    EXCITED_OPENERS.contains(&opener)
        || EXCITED_PREFIXES
            .iter()
            .any(|prefix| lowered.starts_with(prefix))
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
    model_is_installed(app_data_dir, model) || bundled_model_source_path(resource_dir, model).is_some()
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
        .ok_or_else(missing_bundled_model_error)?;

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
    let mut failures = Vec::new();

    for url in model_download_urls(spec) {
        match download_model_from_url(&client, app_data_dir, spec, &url) {
            Ok(path) => return Ok(path),
            Err(error) => failures.push(format!("{url}: {error}")),
        }
    }

    Err(format!(
        "Unable to download the optional whisper.cpp model `{}` from any configured source. {}",
        spec.file_name,
        failures.join(" | ")
    ))
}

fn model_download_urls(spec: &WhisperModelSpec) -> Vec<String> {
    let mut urls = Vec::new();

    if let Ok(base_url) = env::var("TATOR_WHISPER_MODEL_MIRROR_BASE_URL") {
        let trimmed = base_url.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            urls.push(format!("{trimmed}/{}", spec.file_name));
        }
    }

    for url in &spec.urls {
        if !urls.iter().any(|candidate| candidate == url) {
            urls.push(url.clone());
        }
    }

    urls
}

fn download_model_from_url(
    client: &Client,
    app_data_dir: &Path,
    spec: &WhisperModelSpec,
    url: &str,
) -> Result<PathBuf, String> {
    let response = client
        .get(url)
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

    let entries =
        fs::read_dir(&model_dir).map_err(|error| format!("Unable to inspect the model directory: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Unable to inspect the model directory: {error}"))?;
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
        Some(missing_bundled_model_error())
    } else {
        Some(optional_model_install_error(model))
    }
}

fn missing_bundled_model_error() -> String {
    format!(
        "This build does not include the bundled whisper.cpp model `{}`. Run `npm run prepare:whisper-assets` before `tauri dev` or `tauri build` so the model is packaged into the desktop app.",
        bundled_model().file_name
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
    fn model_status_reports_bundled_base_by_default() {
        let runtime = Mutex::new(ModelRuntimeState::default());
        let root = temp_root("status-base");
        let app_data_dir = root.join("app-data");
        let resource_dir = root.join("resources");
        write_bundled_model_resource(&resource_dir, SpeechModel::BaseEn, b"base model");

        let status = get_model_status(
            &app_data_dir,
            Some(&resource_dir),
            &runtime,
            SpeechModel::BaseEn,
        )
        .unwrap();

        assert!(status.installed);
        assert_eq!(status.bundled_model, SpeechModel::BaseEn);
        assert_eq!(status.active_model, SpeechModel::BaseEn);
        assert_eq!(status.installed_models, vec![SpeechModel::BaseEn]);
        assert!(status.download_target.is_none());
        assert!(!status.downloading);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn model_status_lists_optional_small_when_present() {
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
    fn optional_small_model_reports_download_prompt_when_missing() {
        let message =
            model_availability_error(Path::new("/tmp/tator-missing-small-model"), None, SpeechModel::SmallEn)
                .unwrap();
        assert!(message.contains("Download the roughly 488 MB model from Settings"));
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
        assert!(installed_dir.join(&find_model(SpeechModel::SmallEn).file_name).exists());
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

        assert!(installed_dir.join(&find_model(SpeechModel::BaseEn).file_name).exists());
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
    fn model_download_urls_prefers_custom_mirror_then_catalog_sources() {
        env::set_var("TATOR_WHISPER_MODEL_MIRROR_BASE_URL", "https://mirror.example/models/");

        let urls = model_download_urls(find_model(SpeechModel::SmallEn));

        env::remove_var("TATOR_WHISPER_MODEL_MIRROR_BASE_URL");

        assert_eq!(
            urls,
            vec![
                "https://mirror.example/models/ggml-small.en.bin".to_string(),
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin?download=1"
                    .to_string(),
                "https://github.com/kevcoxe/tator/releases/latest/download/ggml-small.en.bin"
                    .to_string(),
            ]
        );
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
                return Err(io::Error::new(io::ErrorKind::Interrupted, "simulated failure"));
            }

            let chunk = b"partial";
            buf[..chunk.len()].copy_from_slice(chunk);
            self.emitted_chunk = true;
            Ok(chunk.len())
        }
    }
}
