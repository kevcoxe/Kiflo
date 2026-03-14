use std::sync::OnceLock;

use serde::Deserialize;

use crate::models::SpeechModel;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelSpec {
    pub id: SpeechModel,
    pub file_name: String,
    pub url: String,
    pub sha1: String,
    pub size_bytes: u64,
    pub bundled: bool,
}

const MODEL_CATALOG_JSON: &str = include_str!("../resources/whisper-models.json");

static MODEL_CATALOG: OnceLock<Vec<WhisperModelSpec>> = OnceLock::new();

fn load_catalog() -> Vec<WhisperModelSpec> {
    serde_json::from_str(MODEL_CATALOG_JSON).expect("whisper model catalog should be valid JSON")
}

pub fn all_models() -> &'static [WhisperModelSpec] {
    MODEL_CATALOG.get_or_init(load_catalog).as_slice()
}

pub fn find_model(model: SpeechModel) -> &'static WhisperModelSpec {
    all_models()
        .iter()
        .find(|spec| spec.id == model)
        .unwrap_or_else(|| panic!("missing whisper.cpp model metadata for {:?}", model))
}

pub fn bundled_model() -> &'static WhisperModelSpec {
    all_models()
        .iter()
        .find(|spec| spec.bundled)
        .expect("whisper model catalog should define one bundled model")
}

pub fn is_known_model_file(file_name: &str) -> bool {
    all_models().iter().any(|spec| spec.file_name == file_name)
}
