use codemorph_core::sandbox::{ExecutionLimits, ExecutionRequest};
use codemorph_cpp::CppAdapter;
use codemorph_rustlang::RustAdapter;
use codemorph_sandbox::Sandbox;
use std::path::PathBuf;

pub fn check() {
    println!("codemorph environment");
    println!("clang++: {}", find("clang++"));
    println!("g++: {}", find("g++"));
    println!("rustc: {}", find("rustc"));
    println!("cargo: {}", find("cargo"));
}

fn find(name: &str) -> &'static str {
    if which::which(name).is_ok() {
        "available"
    } else {
        "missing"
    }
}

pub fn compile(
    language: String,
    source: String,
    standard: Option<String>,
    sanitize: Option<String>,
) {
    match language.as_str() {
        "cpp" | "c++" => {
            let adapter = CppAdapter::new();
            let r = if let Some(opts) = sanitize {
                let sanitizers: Vec<&str> = opts
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                adapter.compile_text_sanitized(
                    &source,
                    standard.as_deref().unwrap_or("c++17"),
                    &sanitizers,
                    None,
                )
            } else {
                adapter.compile_text(&source, standard.as_deref().unwrap_or("c++17"), None)
            };
            print_compile("cpp", r);
        }
        "rust" | "rs" => {
            let adapter = RustAdapter::new();
            let r = adapter.compile_text(&source, None);
            print_compile("rust", r);
        }
        _ => eprintln!("unsupported language: {}", language),
    }
}

fn print_compile(lang: &str, r: codemorph_core::compiler::CompileResult) {
    println!("language: {}", lang);
    println!("success: {}", r.success);
    if let Some(p) = &r.output_path {
        println!("output: {}", p);
    }
    if !r.stdout.trim().is_empty() {
        println!("stdout:\n{}", r.stdout);
    }
    if !r.stderr.trim().is_empty() {
        println!("stderr:\n{}", r.stderr);
    }
}

pub fn diff_test(
    cpp_source: String,
    rust_source: String,
    input: Option<String>,
    cases: usize,
    seed: u64,
) {
    let cpp = CppAdapter::new();
    let rust = RustAdapter::new();
    let temp = match tempfile::TempDir::new() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("tempdir: {}", e);
            return;
        }
    };
    let cpp_out = temp.path().join("cpp.out");
    let rust_out = temp.path().join("rust.out");
    let cr = cpp.compile_text(&cpp_source, "c++17", Some(cpp_out.clone()));
    if !cr.success {
        eprintln!("C++ compile failed:\n{}", cr.stderr);
        return;
    }
    let rr = rust.compile_text(&rust_source, Some(rust_out.clone()));
    if !rr.success {
        eprintln!("Rust compile failed:\n{}", rr.stderr);
        return;
    }

    let run = |path: PathBuf, case_input: Option<String>| -> (Option<i32>, String, String, bool) {
        let req = ExecutionRequest {
            program: path.to_string_lossy().to_string(),
            args: vec![],
            cwd: None,
            env: vec![],
            stdin: case_input,
            limits: ExecutionLimits {
                timeout_ms: 10_000,
                ..Default::default()
            },
        };
        match Sandbox::execute(&req) {
            Ok(res) => (res.exit_code, res.stdout, res.stderr, res.timed_out),
            Err(e) => (None, String::new(), e.to_string(), false),
        }
    };

    let mut rng = codemorph_verify::SeededRng::new(seed);
    let mut mismatch = 0usize;
    let iterations = if input.is_some() { 1 } else { cases.max(1) };
    for i in 0..iterations {
        let case_input = if let Some(value) = input.clone() {
            Some(value)
        } else if iterations > 1 {
            Some(format!(
                "{} {}\n",
                rng.next_i32(-10, 10),
                rng.next_i32(-10, 10)
            ))
        } else {
            input.clone()
        };

        let (cpp_code, cpp_outs, cpp_err, cpp_timeout) = run(cpp_out.clone(), case_input.clone());
        let (rust_code, rust_outs, rust_err, rust_timeout) =
            run(rust_out.clone(), case_input.clone());

        let same = !cpp_timeout && !rust_timeout && cpp_code == rust_code && cpp_outs == rust_outs;
        if !same {
            mismatch += 1;
            if mismatch <= 3 {
                println!("case {}: MISMATCH", i);
                println!("  input: {:?}", case_input);
                println!("  C++ exit: {:?}, timed_out: {}", cpp_code, cpp_timeout);
                println!("  Rust exit: {:?}, timed_out: {}", rust_code, rust_timeout);
                println!("  C++ stdout: {:?}", cpp_outs);
                println!("  Rust stdout: {:?}", rust_outs);
                if !cpp_err.is_empty() {
                    println!("  C++ stderr: {}", cpp_err);
                }
                if !rust_err.is_empty() {
                    println!("  Rust stderr: {}", rust_err);
                }
            }
        }
    }
    println!("cases: {}, mismatches: {}", iterations, mismatch);
    if mismatch > 0 {
        std::process::exit(1);
    }
}


fn generate_translation(prompt: &str, allow_local: bool, allow_ollama: bool) -> Result<String, String> {
    let models_root = resolve_models_root();
    let translator_dir = std::path::PathBuf::from(&models_root).join("translator");
    let manifest_path = translator_dir.join("manifest.json");
    if allow_local && manifest_path.exists() {
        let manifest = std::fs::read_to_string(&manifest_path).unwrap_or_default();
        if manifest.contains("python-torch") {
            let python = std::env::var("CODEMORPH_PYTHON").unwrap_or_else(|_| "python3.12".to_string());
            let mut pythonpath = resolve_pythonpath();
            if !std::path::Path::new(&pythonpath).exists() {
                pythonpath = ".".to_string();
            }
            let env = vec![("PYTHONPATH".to_string(), pythonpath)];
            use codemorph_core::sandbox::{ExecutionLimits, ExecutionRequest};
            use codemorph_sandbox::Sandbox;
            let req = ExecutionRequest {
                program: python.clone(),
                args: vec!["-m".to_string(), "codemorph_ml.inference".to_string(), "--model-dir".to_string(), translator_dir.display().to_string(), "--source-text".to_string(), prompt.to_string()],
                cwd: None,
                env: env.clone(),
                stdin: None,
                limits: ExecutionLimits { timeout_ms: 300_000, ..Default::default() },
            };
            let res = Sandbox::execute(&req).map_err(|e| e.to_string())?;
            if res.timed_out { return Err("python inference timeout".to_string()); }
            if res.exit_code != Some(0) { return Err(res.stderr); }
            return Ok(res.stdout);
        }
    }

    if !allow_ollama {
        return Err("Local model unavailable and Ollama assistance is disabled".to_string());
    }
    let model = std::env::var("CODEMORPH_OLLAMA_MODEL").unwrap_or_else(|_| "beam:cloud".to_string());
    python_ollama_generate(&model, prompt)
}


fn selected_translation_backend() -> String {
    let models_root = resolve_models_root();
    let manifest_path = PathBuf::from(&models_root).join("translator").join("manifest.json");
    let local_disabled = std::env::var("CODEMORPH_USE_LOCAL_MODEL").unwrap_or_default() == "0";
    if !local_disabled && manifest_path.exists() {
        let manifest = std::fs::read_to_string(&manifest_path).unwrap_or_default();
        if manifest.contains("python-torch") {
            let model_id = serde_json::from_str::<serde_json::Value>(&manifest)
                .ok()
                .and_then(|v| v.get("model_id").and_then(|x| x.as_str()).map(str::to_string))
                .unwrap_or_else(|| "local-torch".to_string());
            return format!("local trained model ({})", model_id);
        }
    }
    let model = std::env::var("CODEMORPH_OLLAMA_MODEL").unwrap_or_else(|_| "beam:cloud".to_string());
    format!("Ollama teacher ({})", model)
}


fn resolve_models_root() -> String {
    if let Ok(value) = std::env::var("CODEMORPH_MODELS_ROOT") {
        if !value.trim().is_empty() { return value; }
    }
    let cwd = std::path::PathBuf::from("models");
    if cwd.join("translator").join("manifest.json").exists() {
        return cwd.display().to_string();
    }
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            let candidate = ancestor.join("models");
            if candidate.join("translator").join("manifest.json").exists() {
                return candidate.display().to_string();
            }
        }
    }
    "models".to_string()
}


fn resolve_pythonpath() -> String {
    if let Ok(value) = std::env::var("CODEMORPH_PYTHONPATH") {
        if !value.trim().is_empty() { return value; }
    }
    let cwd = std::path::PathBuf::from("python");
    if cwd.join("codemorph_ml").join("inference.py").exists() {
        return cwd.display().to_string();
    }
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            let candidate = ancestor.join("python");
            if candidate.join("codemorph_ml").join("inference.py").exists() {
                return candidate.display().to_string();
            }
        }
    }
    ".".to_string()
}


fn deterministic_translation(source: &str) -> Option<String> {
    if !source.contains("int main") {
        return None;
    }
    let mut out = String::new();
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with("#include") { continue; }
        if line == "using namespace std;" { continue; }
        out.push_str(line);
        out.push('\n');
    }
    while let Some(start) = out.find("std::cout << ") {
        let prefix = out[..start].to_string();
        let rest = &out[start..];
        if let Some(end) = rest.find(';') {
            let mut expr = rest["std::cout << ".len()..end].trim().to_string();
            let mut newline = false;
            if let Some(pos) = expr.rfind(r#" << "\n""#) {
                newline = true;
                expr = expr[..pos].trim().to_string();
            }
            if let Some(pos) = expr.rfind(r#" << "\n""#) {
                newline = true;
                expr = expr[..pos].trim().to_string();
            }
            if expr.starts_with('"') && expr.ends_with('"') && (expr.contains("\\n") || expr.contains("\\t")) {
            // trailing newline already handled above if marker present
            }
            let remain = &rest[end + 1..];
            let stmt = if newline {
                format!("println!(\"{{}}\", {});", expr)
            } else {
                format!("print!(\"{{}}\", {});", expr)
            };
            out = format!("{}{}{}", prefix, stmt, remain);
        } else {
            break;
        }
    }
    out = out.replace("int main(){", "fn main() {");
    out = out.replace("int main() {", "fn main() {");
    out = out.replace("return 0;", "");
    if out.contains("fn main") {
        if !out.trim_end().ends_with('}') {
            out.push('}');
        }
        Some(out.trim().to_string())
    } else {
        None
    }
}

pub fn migrate(input: PathBuf, output: PathBuf, full: bool, use_ollama: bool, cases: usize, seed: u64) {
    let allow_ollama = use_ollama || std::env::var("CODEMORPH_USE_OLLAMA").map(|v| v=="1").unwrap_or(false);

    println!("Analyzing {}", input.display());
    if !input.exists() {
        eprintln!("input does not exist");
        return;
    }

    let ir = codemorph_project::analyze(&input);
    println!("analyzed files: {}", ir.files.len());
    let mut by_lang = std::collections::BTreeMap::new();
    for f in &ir.files {
        *by_lang.entry(f.language.clone()).or_insert(0usize) += 1;
    }
    for (lang, n) in &by_lang {
        println!("  {}: {}", lang, n);
    }

    std::fs::create_dir_all(&output).ok();
    let mut source_files = Vec::new();
    if input.is_file() {
        source_files.push(input.clone());
    } else {
        fn walk(dir: &PathBuf, out: &mut Vec<PathBuf>) {
            if let Ok(read) = std::fs::read_dir(dir) {
                for entry in read.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
                        if matches!(ext, "cpp" | "cc" | "cxx" | "c" | "h" | "hpp") {
                            out.push(p);
                        }
                    }
                }
            }
        }
        walk(&input, &mut source_files);
    }

    println!("translator backend selected: {}", selected_translation_backend());

    let mut manifest: Vec<serde_json::Value> = Vec::new();
    let mut translated = 0usize;
    let mut failed = 0usize;
    let source_files_is_empty = source_files.is_empty();
    for source_path in &source_files {
        let rel = if input.is_dir() {
            source_path.strip_prefix(&input).unwrap_or(source_path).to_path_buf()
        } else {
            source_path.file_name().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("input.rs"))
        };
        let out_path = output.join(rel.with_extension("rs"));
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let source_cpp = match std::fs::read_to_string(source_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cannot read {}: {}", source_path.display(), e);
                failed += 1;
                continue;
            }
        };

        print!("translating {} via {} ... ", source_path.display(), selected_translation_backend());
        let candidate = match (allow_ollama, deterministic_translation(&source_cpp)) {
            (_, Some(x)) => clean_text(&x),
            _ => match generate_translation(&build_translation_prompt(&source_cpp), true, allow_ollama) {
                Ok(x) => clean_text(&x),
                Err(e) => {
                    println!("FAILED ({e})");
                    failed += 1;
                    continue;
                }
            }
        };

        let compile_ok = write_and_check_rust(&out_path, &candidate);
        if !compile_ok {
            print!("repairing {} ... ", out_path.display());
            let repair_prompt = format!(
                "You are repairing Rust code. Return ONLY complete valid Rust source, no markdown and no explanation. Original C++:\n```cpp\n{}\n```\nBroken Rust:\n```rust\n{}\n```",
                source_cpp, candidate
            );
            if let Ok(repaired) = generate_translation(&repair_prompt, false, allow_ollama) {
                let repaired = clean_text(&repaired);
                if write_and_check_rust(&out_path, &repaired) {
                    println!("compiled after repair");
                    translated += 1;
                } else {
                    println!("still failed");
                    failed += 1;
                }
            } else {
                println!("repair generation failed");
                failed += 1;
            }
        } else {
            println!("compiled");
            translated += 1;
        }

        manifest.push(serde_json::json!({
            "source": source_path.display().to_string(),
            "output": out_path.display().to_string(),
            "model": "auto",
            "behavior_verified": false,
            "behavior_passed_inputs": 0,
            "behavior_total_inputs": 0,
        }));
    }

    let mut behavior_verified_files = 0usize;
    if full {
        println!("Behavior verification stage: {} cases per file", cases);
        for entry in manifest.iter_mut() {
            let source_path = entry.get("source").and_then(|v| v.as_str()).map(PathBuf::from);
            let out_path = entry.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            if let (Some(source_path), Some(out_path)) = (source_path, out_path) {
                if !source_path.exists() || !out_path.exists() { continue; }
                let source_cpp = std::fs::read_to_string(&source_path).unwrap_or_default();
                let generated_rust = std::fs::read_to_string(&out_path).unwrap_or_default();
                if source_cpp.is_empty() || generated_rust.is_empty() { continue; }
                let (ok, passed) = behavior_check(&source_cpp, &generated_rust, cases, seed);
                entry["behavior_total_inputs"] = serde_json::json!(cases);
                entry["behavior_passed_inputs"] = serde_json::json!(passed);
                entry["behavior_verified"] = serde_json::json!(ok);
                if ok { behavior_verified_files += 1; }
                println!("  {} -> behavior {}", source_path.display(), if ok { "OK" } else { "MISMATCH" });
            }
        }
        println!("behavior verified files: {}/{}", behavior_verified_files, manifest.len());
    }

    let report = serde_json::json!({
        "input": input.display().to_string(),
        "output": output.display().to_string(),
        "translated": translated,
        "failed": failed,
        "behavior_verified_files": behavior_verified_files,
        "files": manifest,
    });
    std::fs::create_dir_all(output.join(".codemorph")).ok();
    let report_path = output.join(".codemorph").join("migration-report.json");
    let _ = std::fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap_or_default());

    println!("translated: {}", translated);
    println!("failed: {}", failed);
    println!("report: {}", report_path.display());
    if source_files_is_empty {
        println!("status: no source files found");
    } else {
        println!("status: migration attempt completed");
    }
}

pub fn models_list() {
    let root = std::env::var("CODEMORPH_MODELS").unwrap_or_else(|_| "models".to_string());
    let reg = codemorph_models::ModelRegistry::new(PathBuf::from(&root));
    let manifests = reg.discover();
    if manifests.is_empty() {
        println!("No model manifests found under {}", root);
        return;
    }
    for m in manifests {
        println!("{}", m.display());
    }
}

pub fn models_validate() {
    let root = std::env::var("CODEMORPH_MODELS").unwrap_or_else(|_| "models".to_string());
    let reg = codemorph_models::ModelRegistry::new(PathBuf::from(&root));
    let results = reg.validate_all();
    if results.is_empty() {
        println!("No model manifests found under {}", root);
        return;
    }
    for r in results {
        println!(
            "{}: {}",
            r.manifest_path,
            if r.valid { "valid" } else { "invalid" }
        );
        for e in r.errors {
            println!("  error: {}", e);
        }
        for w in r.warnings {
            println!("  warning: {}", w);
        }
    }
}

pub fn benchmark(
    cpp_source: String,
    rust_source: String,
    input: Option<String>,
    iterations: usize,
) {
    let cpp = CppAdapter::new();
    let rust = RustAdapter::new();
    let temp = match tempfile::TempDir::new() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("tempdir: {}", e);
            return;
        }
    };
    let cpp_out = temp.path().join("cpp.out");
    let rust_out = temp.path().join("rust.out");
    let cr = cpp.compile_text(&cpp_source, "c++17", Some(cpp_out.clone()));
    if !cr.success {
        eprintln!("C++ compile failed:\n{}", cr.stderr);
        return;
    }
    let rr = rust.compile_text(&rust_source, Some(rust_out.clone()));
    if !rr.success {
        eprintln!("Rust compile failed:\n{}", rr.stderr);
        return;
    }

    let mut cpp_times = Vec::new();
    let mut rust_times = Vec::new();
    for _ in 0..iterations.max(1) {
        let req = ExecutionRequest {
            program: cpp_out.to_string_lossy().to_string(),
            args: vec![],
            cwd: None,
            env: vec![],
            stdin: input.clone(),
            limits: ExecutionLimits {
                timeout_ms: 10_000,
                ..Default::default()
            },
        };
        let r = Sandbox::execute(&req).unwrap();
        cpp_times.push(r.duration_ms as f64);
    }
    for _ in 0..iterations.max(1) {
        let req = ExecutionRequest {
            program: rust_out.to_string_lossy().to_string(),
            args: vec![],
            cwd: None,
            env: vec![],
            stdin: input.clone(),
            limits: ExecutionLimits {
                timeout_ms: 10_000,
                ..Default::default()
            },
        };
        let r = Sandbox::execute(&req).unwrap();
        rust_times.push(r.duration_ms as f64);
    }

    let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len().max(1) as f64;
    let cpp_mean = mean(&cpp_times);
    let rust_mean = mean(&rust_times);
    let ratio = if cpp_mean > 0.0 {
        rust_mean / cpp_mean
    } else {
        f64::NAN
    };
    println!("iterations: {}", iterations);
    println!("cpp_mean_ms: {:.3}", cpp_mean);
    println!("rust_mean_ms: {:.3}", rust_mean);
    println!("rust_over_cpp_ratio: {:.3}", ratio);
}


fn build_translation_prompt(source_cpp: &str) -> String {
    format!(
        "Translate the following C/C++ code into a complete Rust program. Preserve behavior, signatures, and main when present. Return only Rust code, no markdown, no explanation.\n\n{}",
        source_cpp
    )
}

fn clean_text(text: &str) -> String {
    let mut lines = text.trim().lines().collect::<Vec<_>>();
    if lines.first().map(|x| x.trim().starts_with("```")).unwrap_or(false) {
        lines.remove(0);
    }
    while lines.last().map(|x| x.trim().starts_with("```")).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n").trim().to_string()
}


fn python_ollama_generate(model: &str, prompt: &str) -> Result<String, String> {
    use codemorph_core::sandbox::{ExecutionLimits, ExecutionRequest};
    use codemorph_sandbox::Sandbox;

    let python = std::env::var("CODEMORPH_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let mut pythonpath = resolve_pythonpath();
    if !std::path::Path::new(&pythonpath).exists() {
        pythonpath = ".".to_string();
    }

    let script = r#"
import os, sys
os.environ.setdefault("CODEMORPH_OLLAMA_HOST", "http://localhost:11434")
os.environ.setdefault("CODEMORPH_TEACHER_MODEL", sys.argv[1])
sys.path.insert(0, os.environ.get("PYTHONPATH", "."))
from codemorph_ml.ollama_client import OllamaTeacher
teacher = OllamaTeacher(OllamaTeacher.from_env())
prompt = sys.stdin.read()
candidates = teacher.generate_candidates(prompt, candidate_count=1)
print(candidates[0].text, end="")
"#
    .to_string();

    let env = vec![("PYTHONPATH".to_string(), pythonpath)];
    let req = ExecutionRequest {
        program: python.clone(),
        args: vec!["-c".to_string(), script, model.to_string()],
        cwd: None,
        env,
        stdin: Some(prompt.to_string()),
        limits: ExecutionLimits { timeout_ms: 300_000, ..Default::default() },
    };
    let res = Sandbox::execute(&req).map_err(|e| e.to_string())?;
    if res.timed_out { return Err("python ollama timeout".to_string()); }
    if res.exit_code != Some(0) { return Err(res.stderr); }
    if res.stdout.trim().is_empty() { return Err(format!("empty result; stderr={}", res.stderr)); }
    Ok(res.stdout)
}

fn write_and_check_rust(path: &std::path::Path, source: &str) -> bool {
    if std::fs::write(path, source).is_err() {
        return false;
    }
    let tmp_out = path.with_extension("out");
    let result = codemorph_rustlang::RustAdapter::new().compile_text(source, Some(tmp_out));
    result.success
}



fn behavior_check(cpp_source: &str, rust_source: &str, cases: usize, seed: u64) -> (bool, u64) {
    use codemorph_core::sandbox::{ExecutionLimits, ExecutionRequest};
    use codemorph_sandbox::Sandbox;
    use codemorph_verify::SeededRng;
    use tempfile::TempDir;

    let dir = match TempDir::new() { Ok(d) => d, Err(_) => return (false, 0) };
    let cpp_path = dir.path().join("cpp.out");
    let rust_path = dir.path().join("rust.out");
    let cpp_src = dir.path().join("main.cpp");
    let rust_src = dir.path().join("main.rs");
    if std::fs::write(&cpp_src,cpp_source).is_err() { return (false,0); }
    if std::fs::write(&rust_src,rust_source).is_err() { return (false,0); }

    let compile_cpp = ExecutionRequest { program: "g++".to_string(), args: vec!["-std=c++17".into(),"-O2".into(),"-o".into(),cpp_path.display().to_string(),cpp_src.display().to_string()], cwd: None, env: vec![], stdin: None, limits: ExecutionLimits { timeout_ms: 30_000, ..Default::default() } };
    let compile_rust = ExecutionRequest { program: "rustc".to_string(), args: vec!["-O".into(),"-o".into(),rust_path.display().to_string(),rust_src.display().to_string()], cwd: None, env: vec![], stdin: None, limits: ExecutionLimits { timeout_ms: 30_000, ..Default::default() } };

    if Sandbox::execute(&compile_cpp).map(|r|r.exit_code).unwrap_or(None) != Some(0) { return (false,0); }
    if Sandbox::execute(&compile_rust).map(|r|r.exit_code).unwrap_or(None) != Some(0) { return (false,0); }

    let mut rng = SeededRng::new(seed);
    let mut passed = 0u64;
    for _ in 0..cases.max(1) {
        let input = format!("{} {}\n", rng.next_i32(-10,10), rng.next_i32(-10,10));
        let run_cpp = ExecutionRequest { program: cpp_path.to_string_lossy().into(), args: vec![], cwd: None, env: vec![], stdin: Some(input.clone()), limits: ExecutionLimits { timeout_ms: 5_000, ..Default::default() } };
        let run_rust = ExecutionRequest { program: rust_path.to_string_lossy().into(), args: vec![], cwd: None, env: vec![], stdin: Some(input.clone()), limits: ExecutionLimits { timeout_ms: 5_000, ..Default::default() } };
        match (Sandbox::execute(&run_cpp), Sandbox::execute(&run_rust)) {
            (Ok(c), Ok(r)) if c.exit_code == r.exit_code && c.stdout == r.stdout && !c.timed_out && !r.timed_out => passed += 1,
            _ => return (false, passed),
        }
    }
    (passed == cases.max(1) as u64 && cases > 0, passed)
}



