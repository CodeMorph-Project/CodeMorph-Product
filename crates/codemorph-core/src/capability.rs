use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    Parse,
    Compile,
    Execute,
    Test,
    Format,
    Lint,
    StaticAnalysis,
    Sanitize,
    Fuzz,
    Benchmark,
}
