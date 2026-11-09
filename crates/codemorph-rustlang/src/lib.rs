use codemorph_core::compiler::{CompileRequest, CompileResult, Compiler};
use codemorph_core::language::{Language, LanguageFamily, LanguageId};
use codemorph_core::sandbox::{ExecutionLimits, ExecutionRequest};
use codemorph_core::traits::Adapter;
use codemorph_core::Capability;
use codemorph_sandbox::Sandbox;
use std::path::PathBuf;
use tempfile::TempDir;
use which::which;

pub struct RustAdapter {
    language: Language,
}
impl Default for RustAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RustAdapter {
    pub fn new() -> Self {
        Self {
            language: Language {
                id: LanguageId::new("rust"),
                display_name: "Rust".to_string(),
                family: LanguageFamily::Systems,
            },
        }
    }

    pub fn compiler_path() -> Option<PathBuf> {
        if let Ok(value) = std::env::var("CODEMORPH_RUSTC") {
            let path = PathBuf::from(value);
            if path.exists() {
                return Some(path);
            }
        }
        if let Ok(path) = which("rustc") {
            // Avoid the i386/old wrappers from /usr/local/bin on some machines.
            if path.exists() && path.to_string_lossy().contains("rustup") {
                return Some(path);
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let toolchains = PathBuf::from(home).join(".rustup").join("toolchains");
            if toolchains.exists() {
                let mut candidates = Vec::new();
                for entry in std::fs::read_dir(&toolchains)
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    let candidate = entry.path().join("bin").join("rustc");
                    if candidate.exists() {
                        candidates.push(candidate);
                    }
                }
                candidates.sort();
                if let Some(path) = candidates.into_iter().last() {
                    return Some(path);
                }
            }
        }
        for candidate in ["/opt/homebrew/bin/rustc", "/usr/local/bin/rustc", "rustc"] {
            let path = PathBuf::from(candidate);
            if path.exists() {
                return Some(path);
            }
        }
        // Last resort: any rustc on PATH (supports non-rustup installs).
        if let Ok(path) = which::which("rustc") {
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    pub fn compile_text(&self, source: &str, output: Option<PathBuf>) -> CompileResult {
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
        let src = dir.path().join("input.rs");
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
        let compiler = match Self::compiler_path() {
            Some(p) => p,
            None => {
                return CompileResult {
                    success: false,
                    output_path: None,
                    stdout: String::new(),
                    stderr: "no Rust compiler found".to_string(),
                    diagnostics: vec!["no Rust compiler found".to_string()],
                }
            }
        };
        let req = ExecutionRequest {
            program: compiler.to_string_lossy().to_string(),
            args: vec![
                src.to_string_lossy().to_string(),
                "-O".to_string(),
                "-o".to_string(),
                out.to_string_lossy().to_string(),
            ],
            cwd: None,
            env: vec![],
            stdin: None,
            limits: ExecutionLimits {
                timeout_ms: 120_000,
                ..Default::default()
            },
        };
        match Sandbox::execute(&req) {
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

impl Adapter for RustAdapter {
    fn language(&self) -> &Language {
        &self.language
    }
    fn capabilities(&self) -> &[Capability] {
        static CAPS: [Capability; 5] = [
            Capability::Parse,
            Capability::Compile,
            Capability::Execute,
            Capability::Lint,
            Capability::Fuzz,
        ];
        &CAPS
    }
}

impl Compiler for RustAdapter {
    fn id(&self) -> &'static str {
        "rustc"
    }
    fn compile(&self, request: &CompileRequest) -> codemorph_core::CoreResult<CompileResult> {
        let source = request.source_text.clone().unwrap_or_default();
        Ok(self.compile_text(&source, request.output_path.clone().map(PathBuf::from)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compile_simple() {
        let adapter = RustAdapter::new();
        let r = adapter.compile_text("fn main(){println!(\"hi\");}\n", None);
        assert!(r.success, "stderr: {}", r.stderr);
    }
}
