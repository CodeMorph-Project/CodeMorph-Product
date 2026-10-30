use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelRole {
    Translator,
    Ownership,
    Repair,
    Idiomatizer,
    Performance,
    Planner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub role: ModelRole,
    pub source_language: String,
    pub target_language: String,
    pub prompt_name: String,
    pub prompt_version: String,
    pub input_text: String,
    pub candidate_count: usize,
    pub generation: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationCandidate {
    pub text: String,
    pub score: Option<f64>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub model_id: String,
    pub model_version: String,
    pub candidates: Vec<GenerationCandidate>,
    pub latency_ms: u64,
    pub input_tokens: Option<usize>,
    pub output_tokens: Option<usize>,
    pub metadata: serde_json::Value,
}

pub trait ModelBackend {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn supports(&self, role: ModelRole) -> bool;
    fn generate(&self, request: &ModelRequest) -> crate::CoreResult<ModelResponse>;
}
