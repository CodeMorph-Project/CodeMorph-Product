//! Generic CodeMorph core abstractions.

pub mod capability;
pub mod compiler;
pub mod diagnostics;
pub mod error;
pub mod ir;
pub mod language;
pub mod model;
pub mod pipeline;
pub mod sandbox;
pub mod traits;
pub mod verification;

pub use capability::Capability;
pub use compiler::{CompileRequest, CompileResult, Compiler};
pub use diagnostics::{Diagnostic, Severity};
pub use error::CoreError;
pub use error::CoreResult;
pub use language::{Language, LanguageId};
pub use model::{GenerationCandidate, ModelBackend, ModelRequest, ModelResponse, ModelRole};
pub use pipeline::{PipelineStage, StageResult};
pub use sandbox::{ExecutionLimits, ExecutionRequest, ExecutionResult};
pub use traits::{
    BenchmarkRunner, Formatter, Fuzzer, Linter, Runtime, SourceAnalyzer, StaticAnalyzer,
    TestHarness,
};
pub use verification::{Confidence, Verdict, VerificationResult};
