use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLimits {
    pub timeout_ms: u64,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub memory_bytes: Option<u64>,
    pub cpu_seconds: Option<u64>,
    pub network_enabled: bool,
    pub filesystem_root: Option<String>,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            timeout_ms: 10_000,
            max_stdout_bytes: 1_048_576,
            max_stderr_bytes: 1_048_576,
            memory_bytes: None,
            cpu_seconds: None,
            network_enabled: false,
            filesystem_root: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub stdin: Option<String>,
    pub limits: ExecutionLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}
