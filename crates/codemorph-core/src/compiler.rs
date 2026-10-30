use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileRequest {
    pub language: String,
    pub source_path: Option<String>,
    pub source_text: Option<String>,
    pub standard: Option<String>,
    pub flags: Vec<String>,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub success: bool,
    pub output_path: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub diagnostics: Vec<String>,
}

pub trait Compiler {
    fn id(&self) -> &'static str;
    fn compile(&self, request: &CompileRequest) -> crate::CoreResult<CompileResult>;
}
