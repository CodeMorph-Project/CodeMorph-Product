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

pub fn migrate(input: PathBuf, output: PathBuf) {
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
    for (lang, n) in by_lang {
        println!("  {}: {}", lang, n);
    }
    std::fs::create_dir_all(&output).ok();
    let mut job = codemorph_orchestrate::MigrationJob::new(
        "migrate",
        input.display().to_string(),
        output.display().to_string(),
    );
    job.state = codemorph_core::pipeline::PipelineState::AnalyzeProject;
    let _ = job.advance_deterministic();
    let _ = job.advance_deterministic();
    let state_path = codemorph_orchestrate::state_path(&output);
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let _ = job.save(&state_path);
    println!("state: {}", state_path.display());
    println!("status: project analysis complete; translation stage not yet automated");
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
