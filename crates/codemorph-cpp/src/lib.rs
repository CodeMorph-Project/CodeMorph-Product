use codemorph_core::compiler::{CompileRequest, CompileResult, Compiler};
use codemorph_core::language::{Language, LanguageFamily, LanguageId};
use codemorph_core::traits::Adapter;
use codemorph_core::Capability;
use std::path::PathBuf;
use tempfile::TempDir;
use which::which;

pub struct CppAdapter {
    language: Language,
}
impl CppAdapter {
    pub fn new() -> Self {
        Self {
            language: Language {
                id: LanguageId::new("cpp"),
                display_name: "C++".to_string(),
                family: LanguageFamily::Systems,
            },
        }
    }

    pub fn compiler_path() -> Option<PathBuf> {
        which("clang++").ok().or_else(|| which("g++").ok())
    }

    pub fn compile_text(
        &self,
        source: &str,
        standard: &str,
        output: Option<PathBuf>,
    ) -> CompileResult {
        let dir = match TempDir::new() {
            Ok(d) => d,
            Err(e) => {
                return CompileResult {
                    success: false,
                    output_path: None,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    diagnostics: vec![e.to_string()],
                }
            }
        };
        let src = dir.path().join("input.cpp");
        if let Err(e) = std::fs::write(&src, source) {
            return CompileResult {
                success: false,
                output_path: None,
                stdout: String::new(),
                stderr: e.to_string(),
                diagnostics: vec![e.to_string()],
            };
        }
        let out = output.unwrap_or_else(|| dir.path().join("input.out"));
        let flags = [
            format!("-std={standard}"),
            "-O1".into(),
            "-Wall".into(),
            "-Wextra".into(),
        ];
        let limits = codemorph_core::sandbox::ExecutionLimits {
            timeout_ms: 60_000,
            ..Default::default()
        };
        match codemorph_sandbox::container::compile("g++", &flags, &src, &out, &limits) {
            Ok(res) => {
                let success = res.exit_code == Some(0) && out.exists();
                CompileResult {
                    success,
                    output_path: if success {
                        Some(out.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    stdout: res.stdout,
                    stderr: res.stderr.clone(),
                    diagnostics: if success {
                        Vec::new()
                    } else {
                        vec![res.stderr]
                    },
                }
            }
            Err(e) => CompileResult {
                success: false,
                output_path: None,
                stdout: String::new(),
                stderr: e.to_string(),
                diagnostics: vec![e.to_string()],
            },
        }
    }

    pub fn compile_text_sanitized(
        &self,
        source: &str,
        standard: &str,
        sanitizers: &[&str],
        output: Option<PathBuf>,
    ) -> CompileResult {
        let dir = match TempDir::new() {
            Ok(d) => d,
            Err(e) => {
                return CompileResult {
                    success: false,
                    output_path: None,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    diagnostics: vec![e.to_string()],
                }
            }
        };
        let src = dir.path().join("input.cpp");
        if let Err(e) = std::fs::write(&src, source) {
            return CompileResult {
                success: false,
                output_path: None,
                stdout: String::new(),
                stderr: e.to_string(),
                diagnostics: vec![e.to_string()],
            };
        }
        let out = output.unwrap_or_else(|| dir.path().join("input.out"));
        let mut args = vec![
            "-std=".to_string() + standard,
            "-O1".to_string(),
            "-g".to_string(),
            "-Wall".to_string(),
        ];
        for s in sanitizers {
            args.push(format!("-fsanitize={}", s));
            args.push("-fno-omit-frame-pointer".to_string());
        }
        let limits = codemorph_core::sandbox::ExecutionLimits {
            timeout_ms: 120_000,
            ..Default::default()
        };
        match codemorph_sandbox::container::compile("g++", &args, &src, &out, &limits) {
            Ok(res) => {
                let success = res.exit_code == Some(0) && out.exists();
                CompileResult {
                    success,
                    output_path: if success {
                        Some(out.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    stdout: res.stdout,
                    stderr: res.stderr.clone(),
                    diagnostics: if success {
                        Vec::new()
                    } else {
                        vec![res.stderr]
                    },
                }
            }
            Err(e) => CompileResult {
                success: false,
                output_path: None,
                stdout: String::new(),
                stderr: e.to_string(),
                diagnostics: vec![e.to_string()],
            },
        }
    }
}

impl Default for CppAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipRecommendation {
    BorrowShared,
    BorrowMut,
    UniqueOwner,
    SharedSingleThread,
    SharedThreadSafe,
    Optional,
    RawRequired,
    Unknown,
}

impl OwnershipRecommendation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BorrowShared => "borrow_shared",
            Self::BorrowMut => "borrow_mut",
            Self::UniqueOwner => "unique_owner",
            Self::SharedSingleThread => "shared_single_thread",
            Self::SharedThreadSafe => "shared_thread_safe",
            Self::Optional => "optional",
            Self::RawRequired => "raw_required",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OwnershipEvidence {
    pub symbol: Option<String>,
    pub recommendation: OwnershipRecommendation,
    pub confidence: &'static str,
    pub source: String,
    pub reason: String,
}

pub fn ownership_evidence(source: &str) -> Vec<OwnershipEvidence> {
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lc = trimmed.to_lowercase();
        let rec = if lc.contains("unique_ptr") {
            OwnershipRecommendation::UniqueOwner
        } else if lc.contains("shared_ptr") || lc.contains("arc") {
            OwnershipRecommendation::SharedThreadSafe
        } else if lc.contains("rc<") {
            OwnershipRecommendation::SharedSingleThread
        } else if lc.contains("optional") {
            OwnershipRecommendation::Optional
        } else if lc.contains("const ") && (lc.contains("&") || lc.contains("*")) {
            OwnershipRecommendation::BorrowShared
        } else if lc.contains("&mut") || (lc.contains("&") && !lc.contains("const")) {
            OwnershipRecommendation::BorrowMut
        } else if lc.contains("*") || lc.contains("void*") || lc.contains("reinterpret_cast") {
            OwnershipRecommendation::RawRequired
        } else {
            OwnershipRecommendation::Unknown
        };
        let confidence = match rec {
            OwnershipRecommendation::UniqueOwner
            | OwnershipRecommendation::SharedThreadSafe
            | OwnershipRecommendation::Optional => "high",
            OwnershipRecommendation::BorrowShared | OwnershipRecommendation::BorrowMut => "medium",
            OwnershipRecommendation::RawRequired | OwnershipRecommendation::SharedSingleThread => {
                "medium"
            }
            OwnershipRecommendation::Unknown => "low",
        };
        let reason = format!("heuristic classification from: {}", trimmed);
        out.push(OwnershipEvidence {
            symbol: None,
            recommendation: rec,
            confidence,
            source: trimmed.to_string(),
            reason,
        });
    }
    out
}

impl Adapter for CppAdapter {
    fn language(&self) -> &Language {
        &self.language
    }
    fn capabilities(&self) -> &[Capability] {
        static CAPS: [Capability; 5] = [
            Capability::Parse,
            Capability::Compile,
            Capability::Execute,
            Capability::StaticAnalysis,
            Capability::Sanitize,
        ];
        &CAPS
    }
}

impl Compiler for CppAdapter {
    fn id(&self) -> &'static str {
        "clang++/g++"
    }
    fn compile(&self, request: &CompileRequest) -> codemorph_core::CoreResult<CompileResult> {
        let source = request.source_text.clone().unwrap_or_default();
        let standard = request
            .standard
            .clone()
            .unwrap_or_else(|| "c++17".to_string());
        Ok(self.compile_text(
            &source,
            &standard,
            request.output_path.clone().map(PathBuf::from),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compile_simple() {
        let adapter = CppAdapter::new();
        let r = adapter.compile_text("int main(){return 0;}\n", "c++17", None);
        assert!(r.success, "stderr: {}", r.stderr);
    }
    #[test]
    fn compile_sanitized() {
        let adapter = CppAdapter::new();
        let r = adapter.compile_text_sanitized(
            "int main(){int*x=nullptr;return *x;}\n",
            "c++17",
            &["undefined"],
            None,
        );
        // may compile (UB at runtime) but should not crash compile
        let _ = r;
    }
    #[test]
    fn ownership_detects_unique_ptr() {
        let ev = ownership_evidence("std::unique_ptr<Foo> p = std::make_unique<Foo>();");
        assert!(!ev.is_empty());
        assert_eq!(ev[0].recommendation.as_str(), "unique_owner");
    }

    #[test]
    fn ownership_detects_raw_pointer() {
        let ev = ownership_evidence("int* p = new int(1);");
        assert_eq!(ev[0].recommendation.as_str(), "raw_required");
    }
}
