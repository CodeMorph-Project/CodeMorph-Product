use crate::capability::Capability;
use crate::language::Language;
use serde::{Deserialize, Serialize};

pub trait Adapter {
    fn language(&self) -> &Language;
    fn capabilities(&self) -> &[Capability];
    fn has_capability(&self, capability: Capability) -> bool {
        self.capabilities().contains(&capability)
    }
}

pub trait SourceAnalyzer: Adapter {
    fn analyze_project(&self, root: &str) -> crate::CoreResult<String>;
}

pub trait Runtime: Adapter {
    fn execute(&self, request: &str) -> crate::CoreResult<String>;
}

pub trait Formatter: Adapter {
    fn format(&self, source: &str) -> crate::CoreResult<String>;
}

pub trait Linter: Adapter {
    fn lint(&self, source: &str) -> crate::CoreResult<String>;
}

pub trait StaticAnalyzer: Adapter {
    fn analyze(&self, source: &str) -> crate::CoreResult<String>;
}

pub trait TestHarness: Adapter {
    fn run_tests(&self, root: &str) -> crate::CoreResult<String>;
}

pub trait Fuzzer: Adapter {
    fn fuzz(&self, target: &str) -> crate::CoreResult<String>;
}

pub trait BenchmarkRunner: Adapter {
    fn benchmark(&self, target: &str) -> crate::CoreResult<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub language: Language,
    pub capabilities: Vec<Capability>,
}
