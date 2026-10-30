use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    None,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Accepted,
    Rejected,
    Escalated,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationStage {
    pub name: String,
    pub success: bool,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub cpp_compiles: bool,
    pub rust_compiles: bool,
    pub tests_passed: bool,
    pub differential_passed: bool,
    pub fuzz_iterations: u64,
    pub sanitizer_status: String,
    pub static_analysis_status: String,
    pub benchmark_ratio: Option<f64>,
    pub rust_quality: f64,
    pub confidence: Confidence,
    pub rejection_reasons: Vec<String>,
    pub stages: Vec<VerificationStage>,
}

impl VerificationResult {
    pub fn new() -> Self {
        Self {
            cpp_compiles: false,
            rust_compiles: false,
            tests_passed: false,
            differential_passed: false,
            fuzz_iterations: 0,
            sanitizer_status: "unknown".to_string(),
            static_analysis_status: "unknown".to_string(),
            benchmark_ratio: None,
            rust_quality: 0.0,
            confidence: Confidence::None,
            rejection_reasons: Vec::new(),
            stages: Vec::new(),
        }
    }

    pub fn add_rejection(&mut self, reason: impl Into<String>) {
        self.rejection_reasons.push(reason.into());
    }

    pub fn is_accepted(&self) -> bool {
        self.rust_compiles && self.tests_passed
    }
}

impl Default for VerificationResult {
    fn default() -> Self {
        Self::new()
    }
}
