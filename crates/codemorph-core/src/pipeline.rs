use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineState {
    AnalyzeProject,
    BuildGraph,
    SelectUnit,
    Translate,
    Compile,
    Repair,
    Test,
    Fuzz,
    Benchmark,
    Accept,
    Retry,
    Escalate,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    pub name: String,
    pub state: PipelineState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage: String,
    pub success: bool,
    pub message: String,
    pub artifacts: Vec<String>,
}
