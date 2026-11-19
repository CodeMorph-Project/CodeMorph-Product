use codemorph_core::sandbox::{ExecutionLimits, ExecutionRequest};
use codemorph_cpp::CppAdapter;
use codemorph_rustlang::RustAdapter;
use codemorph_sandbox::Sandbox;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone)]
enum FileStatus {
    Verified,
    Compiled,
    #[allow(dead_code)] // kept for report-schema compatibility; neural path never emits it
    Unsupported(String),
    Failed(String),
}

impl FileStatus {
    fn status_str(&self) -> &'static str {
        match self {
            FileStatus::Verified => "verified",
            FileStatus::Compiled => "compiled",
            FileStatus::Unsupported(_) => "unsupported",
            FileStatus::Failed(_) => "failed",
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            FileStatus::Unsupported(r) | FileStatus::Failed(r) => Some(r.as_str()),
            _ => None,
        }
    }
}

fn print_summary(
    verified: usize,
    compiled: usize,
    unsupported: usize,
    failed: usize,
    total: usize,
) {
    println!("verified: {}", verified);
    println!("compiled: {}", compiled);
    println!("unsupported: {}", unsupported);
    println!("failed: {}", failed);
    println!("total: {}", total);
}

fn collect_source_files(input: &Path) -> Vec<PathBuf> {
    if input.is_file() {
        vec![input.to_path_buf()]
    } else {
        collect_cpp_files(&input.to_path_buf())
    }
}

fn compile_rust_source(source: &str) -> Result<(), String> {
    let result = RustAdapter::new().compile_text(source, None);
    if result.success {
        Ok(())
    } else {
        Err(result.stderr)
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

/// Translate a source file using the selected backend and compile-check the candidate.
fn translate_one(source: &str, out_path: &Path, model_dir: Option<&Path>) -> FileStatus {
    let candidate = match neural_translate(source, model_dir) {
        Ok(text) => clean_text(&text),
        Err(reason) => {
            let _ = std::fs::remove_file(out_path);
            return FileStatus::Failed(reason);
        }
    };
    if candidate.is_empty() {
        let _ = std::fs::remove_file(out_path);
        return FileStatus::Failed("model returned empty output".to_string());
    }
    if let Err(e) = std::fs::write(out_path, &candidate) {
        return FileStatus::Failed(format!("cannot write output: {}", e));
    }
    match compile_rust_source(&candidate) {
        Ok(()) => FileStatus::Compiled,
        Err(err) => {
            let _ = std::fs::remove_file(out_path);
            FileStatus::Failed(format!("rust compilation failed: {}", first_line(&err)))
        }
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
        match codemorph_sandbox::container::execute_binary(&req) {
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

        let same = cpp_code.is_some()
            && !cpp_timeout
            && !rust_timeout
            && cpp_code == rust_code
            && cpp_outs == rust_outs;
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

fn resolve_python() -> String {
    if let Ok(value) = std::env::var("CODEMORPH_PYTHON") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    // Prefer a Python that has PyTorch first.
    for candidate in [
        "python3.12",
        "/opt/homebrew/bin/python3.12",
        "python3",
        "/usr/bin/python3",
    ] {
        if let Ok(output) = std::process::Command::new(candidate)
            .arg("-c")
            .arg("import torch; print('ok')")
            .output()
        {
            if output.status.success() {
                return candidate.to_string();
            }
        }
    }
    "python3".to_string()
}

/// Resolve an explicit translator profile or discover an installed translator.
fn resolve_translator_dir(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    let root = PathBuf::from(resolve_models_root());
    let preferred = root.join("translator");
    if preferred.join("manifest.json").exists() {
        return Ok(preferred);
    }
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&root)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("translator-"))
                            .unwrap_or(false)
                        && p.join("manifest.json").exists()
                })
                .collect()
        })
        .unwrap_or_default();
    candidates.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| "no local translator artifact found (use --model-dir)".to_string())
}

/// Run the selected translator backend on raw source code.
fn neural_translate(source: &str, model_dir: Option<&Path>) -> Result<String, String> {
    let translator_dir = resolve_translator_dir(model_dir)?;
    let manifest_path = translator_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(format!(
            "model artifact missing manifest: {}",
            manifest_path.display()
        ));
    }
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("cannot read {}: {e}", manifest_path.display()))?;
    let manifest_json: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|e| format!("invalid {}: {e}", manifest_path.display()))?;
    if !manifest_json.is_object() {
        return Err(format!("expected an object in {}", manifest_path.display()));
    }
    let backend = manifest_json
        .get("backend")
        .and_then(|v| v.as_str())
        .unwrap_or("python-torch");
    let mut pythonpath = resolve_pythonpath();
    if !std::path::Path::new(&pythonpath).exists() {
        pythonpath = ".".to_string();
    }
    if backend == "cloud-http" {
        if std::env::var("CODEMORPH_ALLOW_EXTERNAL").as_deref() != Ok("1") {
            return Err("external model backend requires CODEMORPH_ALLOW_EXTERNAL=1".to_string());
        }
        let req = ExecutionRequest {
            program: std::env::var("CODEMORPH_PYTHON").unwrap_or_else(|_| "python3".into()),
            args: vec![
                "-m".to_string(),
                "codemorph_ml.cloud_inference".to_string(),
                "--model-dir".to_string(),
                translator_dir.display().to_string(),
            ],
            cwd: None,
            env: vec![("PYTHONPATH".to_string(), pythonpath)],
            stdin: Some(source.to_owned()),
            limits: ExecutionLimits {
                timeout_ms: 600_000,
                ..Default::default()
            },
        };
        let res = Sandbox::execute(&req).map_err(|e| e.to_string())?;
        if res.timed_out {
            return Err("external model inference timeout".to_string());
        }
        if res.exit_code != Some(0) {
            return Err(format!(
                "external model inference failed: {}",
                first_line(&res.stderr)
            ));
        }
        return Ok(res.stdout);
    }
    if backend != "python-torch" {
        return Err(format!(
            "unsupported model backend in {}",
            manifest_path.display()
        ));
    }
    let req = ExecutionRequest {
        program: resolve_python(),
        args: vec![
            "-m".to_string(),
            "codemorph_ml.inference".to_string(),
            "--model-dir".to_string(),
            translator_dir.display().to_string(),
            "--source-text".to_string(),
            source.to_string(),
        ],
        cwd: None,
        env: vec![("PYTHONPATH".to_string(), pythonpath)],
        stdin: None,
        limits: ExecutionLimits {
            timeout_ms: 300_000,
            ..Default::default()
        },
    };
    let res = Sandbox::execute(&req).map_err(|e| e.to_string())?;
    if res.timed_out {
        return Err("local model inference timeout".to_string());
    }
    if res.exit_code != Some(0) {
        return Err(format!(
            "local model inference failed: {}",
            first_line(&res.stderr)
        ));
    }
    Ok(res.stdout)
}

fn selected_translation_backend(model_dir: Option<&Path>) -> String {
    match resolve_translator_dir(model_dir).ok().and_then(|dir| {
        let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let id = json
            .get("model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("model")
            .to_string();
        let backend = json
            .get("backend")
            .and_then(|v| v.as_str())
            .unwrap_or("python-torch");
        Some((id, backend.to_string()))
    }) {
        Some((id, backend)) if backend == "cloud-http" => {
            format!("EXTERNAL neural model ({id}) [opt-in required]")
        }
        Some((id, _)) => format!("local neural model ({id})"),
        None => "local neural model (not installed)".to_string(),
    }
}

fn resolve_models_root() -> String {
    if let Ok(value) = std::env::var("CODEMORPH_MODELS_ROOT") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    // Prefer the per-user global registry for installed binaries.
    let home_candidate = format!(
        "{}/.codemorph/models",
        std::env::var("HOME").unwrap_or_default()
    );
    if std::path::PathBuf::from(&home_candidate)
        .join("translator")
        .join("manifest.json")
        .exists()
    {
        return home_candidate;
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
    // Installed/global single-user model location.
    let home_candidate = format!(
        "{}/.codemorph/models",
        std::env::var("HOME").unwrap_or_default()
    );
    if std::path::PathBuf::from(&home_candidate)
        .join("translator")
        .join("manifest.json")
        .exists()
    {
        return home_candidate;
    }
    "models".to_string()
}

fn resolve_pythonpath() -> String {
    if let Ok(value) = std::env::var("CODEMORPH_PYTHONPATH") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            let installed = bin_dir.join("../share/codemorph/python");
            if installed.join("codemorph_ml/inference.py").is_file() {
                return installed.display().to_string();
            }
        }
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

fn create_migration_dir(path: &Path) {
    if let Err(error) = std::fs::create_dir_all(path) {
        eprintln!("cannot create {}: {error}", path.display());
        std::process::exit(1);
    }
}

fn write_migration_artifact(path: impl AsRef<Path>, value: &impl serde::Serialize) {
    use std::io::Write;

    let path = path.as_ref();
    let result = (|| -> anyhow::Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("artifact path has no parent"))?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, value)?;
        writeln!(file)?;
        file.persist(path)?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("cannot save {}: {error}", path.display());
        std::process::exit(1);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn migrate(
    input: PathBuf,
    output: PathBuf,
    stage: String,
    full: bool,
    source_lang: String,
    target_lang: String,
    model_dir: Option<PathBuf>,
    cases: usize,
    seed: u64,
    include_stages: Option<String>,
    skip_stages: Option<String>,
) {
    let _ = include_stages;
    let _ = skip_stages;

    if source_lang != "cpp" || target_lang != "rust" {
        eprintln!(
            "language pair {source_lang}->{target_lang} is not active yet; only cpp->rust is available"
        );
        std::process::exit(2);
    }
    if !input.exists() {
        eprintln!("input does not exist: {}", input.display());
        std::process::exit(2);
    }
    if cases == 0 {
        eprintln!("--cases must be greater than zero");
        std::process::exit(2);
    }
    let model_ref = model_dir.as_deref();
    let codemorph_dir = output.join(".codemorph");
    let stage_dir = codemorph_dir.join("stages");
    create_migration_dir(&stage_dir);

    match stage.as_str() {
        "analyze" => {
            let ir = codemorph_project::analyze(&input);
            let path = codemorph_dir.join("project_ir.json");
            write_migration_artifact(&path, &ir);
            println!("analyzed files: {}", ir.files.len());
            println!("artifact: {}", path.display());
            println!("status: analyze complete");
            return;
        }
        "translate" => {
            let source_files = collect_source_files(&input);
            let mut summary = Vec::new();
            let mut verified = 0usize;
            let mut compiled = 0usize;
            let mut unsupported = 0usize;
            let mut failed = 0usize;
            for source_path in &source_files {
                let rel = if input.is_dir() {
                    source_path
                        .strip_prefix(&input)
                        .unwrap_or(source_path)
                        .to_path_buf()
                } else {
                    source_path
                        .file_name()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("input.rs"))
                };
                let out_path = output.join(rel.with_extension("rs"));
                if let Some(p) = out_path.parent() {
                    create_migration_dir(p);
                }
                let source_cpp = match std::fs::read_to_string(source_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("cannot read {}: {}", source_path.display(), e);
                        failed += 1;
                        continue;
                    }
                };
                let status = translate_one(&source_cpp, &out_path, model_ref);
                match &status {
                    FileStatus::Verified => verified += 1,
                    FileStatus::Compiled => compiled += 1,
                    FileStatus::Unsupported(_) => unsupported += 1,
                    FileStatus::Failed(_) => failed += 1,
                }
                let output_value = match &status {
                    FileStatus::Unsupported(_) => serde_json::Value::Null,
                    _ => serde_json::json!(out_path.display().to_string()),
                };
                summary.push(serde_json::json!({
                    "source": source_path.display().to_string(),
                    "output": output_value,
                    "status": status.status_str(),
                    "reason": status.reason(),
                }));
            }
            let path = stage_dir.join("translate.json");
            write_migration_artifact(&path, &summary);
            println!("translated artifacts: {}", summary.len());
            println!("artifact: {}", path.display());
            print_summary(verified, compiled, unsupported, failed, source_files.len());
            println!("status: translate complete");
            return;
        }
        "repair" => {
            let mut results = Vec::new();
            for rust_path in &collect_rust_files(&output) {
                let source = std::fs::read_to_string(rust_path).unwrap_or_default();
                if source.trim().is_empty() {
                    continue;
                }
                let original_compiles = write_and_check_rust(rust_path, &source);
                match role_model_generate(
                    "repair",
                    &format!(
                        "Repair this Rust source and return complete valid Rust code only.\n{}",
                        source
                    ),
                ) {
                    Ok(candidate) => {
                        let candidate = clean_text(&candidate);
                        let candidate_path = rust_path.with_extension("repair.rs");
                        let candidate_compiles = write_and_check_rust(&candidate_path, &candidate);
                        let behavior_ok = matching_cpp_for_rust(&input, rust_path.as_path())
                            .and_then(|cpp| std::fs::read_to_string(cpp).ok())
                            .map(|cpp| behavior_check(&cpp, &candidate, cases, seed).0)
                            .unwrap_or(false);
                        let accepted = candidate_compiles && behavior_ok;
                        if accepted {
                            let backup = rust_path.with_extension("rs.orig");
                            let _ = std::fs::copy(rust_path, &backup);
                            let _ = std::fs::write(rust_path, &candidate);
                        }
                        results.push(serde_json::json!({
                            "file": rust_path.display().to_string(),
                            "original_compiles": original_compiles,
                            "candidate": candidate_path.display().to_string(),
                            "candidate_compiles": candidate_compiles,
                            "behavior_verified": behavior_ok,
                            "accepted": accepted,
                            "status": if accepted { "candidate accepted" } else { "candidate rejected" },
                        }));
                    }
                    Err(e) => results.push(serde_json::json!({
                        "file": rust_path.display().to_string(),
                        "original_compiles": original_compiles,
                        "status": e,
                    })),
                }
            }
            let path = stage_dir.join("repair.json");
            write_migration_artifact(&path, &results);
            println!("repair stage complete: {} files scanned", results.len());
            println!("artifact: {}", path.display());
            return;
        }
        "ownership" => {
            let mut reports = Vec::new();
            for source_path in collect_cpp_files(&input) {
                let source = std::fs::read_to_string(&source_path).unwrap_or_default();
                if source.trim().is_empty() {
                    continue;
                }
                match role_model_generate("ownership", &format!("Classify ownership recommendation.\n{}", source)) {
                    Ok(resp) => reports.push(serde_json::json!({ "file": source_path.display().to_string(), "status": "model-checked", "note": clean_text(&resp) })),
                    Err(e) => reports.push(serde_json::json!({ "file": source_path.display().to_string(), "status": e })),
                }
            }
            let path = stage_dir.join("ownership.json");
            write_migration_artifact(&path, &reports);
            println!(
                "ownership stage: {} files, artifact {}",
                reports.len(),
                path.display()
            );
            return;
        }
        "idiomatize" => {
            let mut reports = Vec::new();
            for rust_path in collect_rust_files(&output) {
                let source = std::fs::read_to_string(&rust_path).unwrap_or_default();
                if source.trim().is_empty() {
                    continue;
                }
                match role_model_generate("idiomatizer", &format!("Rewrite Rust more idiomatically while preserving behavior. Return code only.\n{}", source)) {
                Ok(candidate) => {
                    let candidate = clean_text(&candidate);
                    let ok = write_and_check_rust(&rust_path.with_extension("idiom.rs"), &candidate);
                    let behavior_ok = matching_cpp_for_rust(&input, &rust_path)
                        .and_then(|cpp| std::fs::read_to_string(cpp).ok())
                        .map(|cpp| behavior_check(&cpp, &candidate, cases, seed).0)
                        .unwrap_or(false);
                    let accepted = ok && behavior_ok;
                    if accepted {
                        let backup = rust_path.with_extension("rs.orig");
                        if !backup.exists() {
                            let _ = std::fs::copy(&rust_path, &backup);
                        }
                        let _ = std::fs::write(&rust_path, &candidate);
                    }
                    reports.push(serde_json::json!({
                        "file": rust_path.display().to_string(),
                        "candidate_compiles": ok,
                        "behavior_verified": behavior_ok,
                        "accepted": accepted,
                        "status": if accepted { "candidate accepted" } else { "candidate rejected" },
                    }));
                }
                    Err(e) => reports.push(serde_json::json!({ "file": rust_path.display().to_string(), "status": e })),
                }
            }
            let path = stage_dir.join("idiomatize.json");
            write_migration_artifact(&path, &reports);
            println!(
                "idiomatize stage: {} files, artifact {}",
                reports.len(),
                path.display()
            );
            return;
        }
        "planner" | "performance" | "security" => {
            let role = stage.as_str();
            let prompt_prefix = match role {
                "planner" => "Produce a concise migration plan for this C++ source. Return plain text only.",
                "performance" => "Suggest performance risks and budget notes for this C++ source. Return plain text only.",
                _ => "List security-relevant constructs and risks in this C++ source. Return plain text only.",
            };
            let mut reports = Vec::new();
            for source_path in collect_cpp_files(&input) {
                let source = std::fs::read_to_string(&source_path).unwrap_or_default();
                if source.trim().is_empty() {
                    continue;
                }
                let prompt = format!("{}\n{}", prompt_prefix, source);
                match role_model_generate(role, &prompt) {
                    Ok(resp) => reports.push(serde_json::json!({
                        "file": source_path.display().to_string(),
                        "status": "model-checked",
                        "note": clean_text(&resp),
                    })),
                    Err(e) => reports.push(serde_json::json!({
                        "file": source_path.display().to_string(),
                        "status": e,
                    })),
                }
            }
            let path = stage_dir.join(format!("{}.json", role));
            write_migration_artifact(&path, &reports);
            println!(
                "{} stage: {} files, artifact {}",
                role,
                reports.len(),
                path.display()
            );
            return;
        }
        "verify" => {
            let manifest_path = codemorph_dir.join("migration-report.json");
            let translate_path = stage_dir.join("translate.json");
            let report_path = if translate_path.exists() {
                &translate_path
            } else {
                &manifest_path
            };
            let files = std::fs::read_to_string(report_path)
                .map_err(anyhow::Error::from)
                .and_then(|text| Ok(serde_json::from_str::<serde_json::Value>(&text)?))
                .and_then(|report| {
                    report
                        .as_array()
                        .or_else(|| report.get("files").and_then(|files| files.as_array()))
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("expected an array of files"))
                })
                .unwrap_or_else(|error| {
                    eprintln!("cannot read {}: {error}", report_path.display());
                    std::process::exit(1);
                });
            let mut results = Vec::new();
            for file in &files {
                let (Some(source), Some(candidate)) = (
                    file.get("source").and_then(|v| v.as_str()),
                    file.get("output")
                        .or_else(|| file.get("translation"))
                        .and_then(|v| v.as_str()),
                ) else {
                    continue;
                };
                if file.get("status").and_then(|v| v.as_str()) == Some("failed") {
                    continue;
                }
                let (pass, count, reason) = match (
                    std::fs::read_to_string(source),
                    std::fs::read_to_string(candidate),
                ) {
                    (Ok(cpp), Ok(rust)) => {
                        let (pass, count) = behavior_check(&cpp, &rust, cases, seed);
                        (pass, count, None)
                    }
                    (Err(error), _) => (false, 0, Some(format!("cannot read {source}: {error}"))),
                    (_, Err(error)) => {
                        (false, 0, Some(format!("cannot read {candidate}: {error}")))
                    }
                };
                println!("  {source} -> {}", if pass { "OK" } else { "FAILED" });
                results.push(serde_json::json!({
                    "source": source,
                    "output": candidate,
                    "behavior_verified": pass,
                    "behavior_passed_inputs": count,
                    "behavior_total_inputs": cases,
                    "reason": reason,
                }));
            }
            let passed = results
                .iter()
                .filter(|entry| entry["behavior_verified"] == true)
                .count();
            let artifact = stage_dir.join("verify.json");
            write_migration_artifact(&artifact, &results);
            println!("verified: {passed}/{}", results.len());
            println!("artifact: {}", artifact.display());
            if results.is_empty() || passed != results.len() {
                std::process::exit(1);
            }
            return;
        }
        "ffi" => {
            let mut sources = Vec::new();
            if input.is_file() {
                sources.push(input.clone());
            } else {
                collect_cpp_files(&input)
                    .into_iter()
                    .for_each(|p| sources.push(p));
            }
            let ffi_dir = output.join("ffi");
            create_migration_dir(&ffi_dir);
            let mut count = 0usize;
            let mut manifest = Vec::new();
            for source_path in &sources {
                let source_cpp = std::fs::read_to_string(source_path).unwrap_or_default();
                let module_name = source_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "ffi".to_string());
                let (module_name, bridge) = (module_name, ffi_bridge_for(source_cpp.as_str()).1);
                let out_path = ffi_dir.join(format!("{}.rs", module_name));
                let _ = std::fs::write(&out_path, &bridge);
                count += 1;
                manifest.push(serde_json::json!({
                    "source": source_path.display().to_string(),
                    "bridge": out_path.display().to_string(),
                    "status": "scaffold"
                }));
            }
            let path = output.join(".codemorph").join("stages").join("ffi.json");
            write_migration_artifact(&path, &manifest);
            println!("ffi stage: {} modules scaffolded", count);
            println!("artifact: {}", path.display());
            println!("status: ffi scaffold generated (manual review required)");
            return;
        }
        _ => {}
    }

    eprintln!("[analyze] {}", input.display());

    let ir = codemorph_project::analyze(&input);
    println!("analyzed files: {}", ir.files.len());
    let mut by_lang = std::collections::BTreeMap::new();
    for f in &ir.files {
        *by_lang.entry(f.language.clone()).or_insert(0usize) += 1;
    }
    for (lang, n) in &by_lang {
        println!("  {}: {}", lang, n);
    }

    let source_files = collect_source_files(&input);
    let source_files_is_empty = source_files.is_empty();

    println!(
        "translator backend selected: {}",
        selected_translation_backend(model_ref)
    );

    struct FileResult {
        source: PathBuf,
        output: Option<PathBuf>,
        status: FileStatus,
        behavior_verified: bool,
        behavior_passed: u64,
        behavior_total: u64,
    }

    let mut results: Vec<FileResult> = Vec::new();
    for (index, source_path) in source_files.iter().enumerate() {
        let rel = if input.is_dir() {
            source_path
                .strip_prefix(&input)
                .unwrap_or(source_path)
                .to_path_buf()
        } else {
            source_path
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("input.rs"))
        };
        let out_path = output.join(rel.with_extension("rs"));
        if let Some(parent) = out_path.parent() {
            create_migration_dir(parent);
        }

        let source_cpp = match std::fs::read_to_string(source_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cannot read {}: {}", source_path.display(), e);
                results.push(FileResult {
                    source: source_path.clone(),
                    output: None,
                    status: FileStatus::Failed(format!("cannot read source: {}", e)),
                    behavior_verified: false,
                    behavior_passed: 0,
                    behavior_total: 0,
                });
                continue;
            }
        };

        eprintln!(
            "[translate {}/{}] {}",
            index + 1,
            source_files.len(),
            source_path.display()
        );
        let status = translate_one(&source_cpp, &out_path, model_ref);
        match &status {
            FileStatus::Verified => println!("verified"),
            FileStatus::Compiled => println!("compiled"),
            FileStatus::Unsupported(r) => println!("unsupported ({})", r),
            FileStatus::Failed(r) => println!("failed ({})", r),
        }
        let output_value = match &status {
            FileStatus::Unsupported(_) => None,
            _ => Some(out_path.clone()),
        };
        results.push(FileResult {
            source: source_path.clone(),
            output: output_value,
            status,
            behavior_verified: false,
            behavior_passed: 0,
            behavior_total: 0,
        });
    }

    if full {
        eprintln!("[verify] {cases} cases per file, seed {seed}");
        for r in results.iter_mut() {
            if !matches!(r.status, FileStatus::Compiled) {
                continue;
            }
            let out_path = match &r.output {
                Some(p) => p.clone(),
                None => continue,
            };
            if !r.source.exists() || !out_path.exists() {
                continue;
            }
            let source_cpp = std::fs::read_to_string(&r.source).unwrap_or_default();
            let generated_rust = std::fs::read_to_string(&out_path).unwrap_or_default();
            if source_cpp.is_empty() || generated_rust.is_empty() {
                continue;
            }
            let (ok, passed) = behavior_check(&source_cpp, &generated_rust, cases, seed);
            r.behavior_total = cases as u64;
            r.behavior_passed = passed;
            if ok {
                r.behavior_verified = true;
                r.status = FileStatus::Verified;
            } else {
                r.status =
                    FileStatus::Failed(format!("behavior mismatch ({}/{})", passed, cases.max(1)));
            }
            println!(
                "  {} -> behavior {}",
                r.source.display(),
                if ok { "OK" } else { "MISMATCH" }
            );
        }
    }

    let verified = results
        .iter()
        .filter(|r| matches!(r.status, FileStatus::Verified))
        .count();
    let compiled = results
        .iter()
        .filter(|r| matches!(r.status, FileStatus::Compiled))
        .count();
    let unsupported = results
        .iter()
        .filter(|r| matches!(r.status, FileStatus::Unsupported(_)))
        .count();
    let failed = results
        .iter()
        .filter(|r| matches!(r.status, FileStatus::Failed(_)))
        .count();

    let manifest: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "source": r.source.display().to_string(),
                "output": r.output.as_ref().map(|p| p.display().to_string()),
                "status": r.status.status_str(),
                "reason": r.status.reason(),
                "behavior_verified": r.behavior_verified,
                "behavior_passed_inputs": r.behavior_passed,
                "behavior_total_inputs": r.behavior_total,
            })
        })
        .collect();

    let stage_dir = output.join(".codemorph").join("stages");
    let translate_summary: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "source": r.source.display().to_string(),
                "translation": r.output.as_ref().map(|p| p.display().to_string()),
                "status": r.status.status_str(),
                "reason": r.status.reason(),
            })
        })
        .collect();
    let verify_summary: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "source": r.source.display().to_string(),
                "output": r.output.as_ref().map(|p| p.display().to_string()),
                "behavior_passed_inputs": r.behavior_passed,
                "behavior_total_inputs": r.behavior_total,
                "behavior_verified": r.behavior_verified,
            })
        })
        .collect();
    write_migration_artifact(stage_dir.join("translate.json"), &translate_summary);
    write_migration_artifact(stage_dir.join("verify.json"), &verify_summary);

    let report = serde_json::json!({
        "schema_version": 1,
        "backend": selected_translation_backend(model_ref),
        "behavior_check": full.then_some(serde_json::json!({"cases": cases, "seed": seed})),
        "input": input.display().to_string(),
        "output": output.display().to_string(),
        "verified": verified,
        "compiled": compiled,
        "unsupported": unsupported,
        "failed": failed,
        "total": results.len(),
        "files": manifest,
    });
    let report_path = output.join(".codemorph").join("migration-report.json");
    write_migration_artifact(&report_path, &report);

    print_summary(verified, compiled, unsupported, failed, results.len());
    println!("report: {}", report_path.display());
    if source_files_is_empty {
        println!("status: no source files found");
    } else {
        println!("status: migration attempt completed");
    }
}

pub fn models_list() {
    let root = resolve_models_root();
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
    let root = resolve_models_root();
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
        if let Ok(r) = codemorph_sandbox::container::execute_binary(&req) {
            cpp_times.push(r.duration_ms as f64);
        }
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
        if let Ok(r) = codemorph_sandbox::container::execute_binary(&req) {
            rust_times.push(r.duration_ms as f64);
        }
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

fn clean_text(text: &str) -> String {
    let mut lines = text.trim().lines().collect::<Vec<_>>();
    if lines
        .first()
        .map(|x| x.trim().starts_with("```"))
        .unwrap_or(false)
    {
        lines.remove(0);
    }
    while lines
        .last()
        .map(|x| x.trim().starts_with("```"))
        .unwrap_or(false)
    {
        lines.pop();
    }
    lines.join("\n").trim().to_string()
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
    use codemorph_sandbox::container::execute_binary;
    use codemorph_verify::SeededRng;
    use tempfile::TempDir;

    let dir = match TempDir::new() {
        Ok(d) => d,
        Err(_) => return (false, 0),
    };
    let cpp_path = dir.path().join("cpp.out");
    let rust_path = dir.path().join("rust.out");
    let cpp_src = dir.path().join("main.cpp");
    let rust_src = dir.path().join("main.rs");
    if std::fs::write(&cpp_src, cpp_source).is_err() {
        return (false, 0);
    }
    if std::fs::write(&rust_src, rust_source).is_err() {
        return (false, 0);
    }

    let cr = CppAdapter::new().compile_text(cpp_source, "c++17", Some(cpp_path.clone()));
    if !cr.success {
        return (false, 0);
    }
    let rr = RustAdapter::new().compile_text(rust_source, Some(rust_path.clone()));
    if !rr.success {
        return (false, 0);
    }

    let mut rng = SeededRng::new(seed);
    let mut passed = 0u64;
    const INPUT_TOKENS: usize = 8;
    for _ in 0..cases.max(1) {
        let mut input = String::new();
        for i in 0..INPUT_TOKENS {
            if i > 0 {
                input.push(' ');
            }
            input.push_str(&rng.next_i32(-10, 10).to_string());
        }
        input.push('\n');
        let run_cpp = ExecutionRequest {
            program: cpp_path.to_string_lossy().into(),
            args: vec![],
            cwd: None,
            env: vec![],
            stdin: Some(input.clone()),
            limits: ExecutionLimits {
                timeout_ms: 5_000,
                ..Default::default()
            },
        };
        let run_rust = ExecutionRequest {
            program: rust_path.to_string_lossy().into(),
            args: vec![],
            cwd: None,
            env: vec![],
            stdin: Some(input.clone()),
            limits: ExecutionLimits {
                timeout_ms: 5_000,
                ..Default::default()
            },
        };
        match (execute_binary(&run_cpp), execute_binary(&run_rust)) {
            (Ok(c), Ok(r))
                if c.exit_code.is_some()
                    && c.exit_code == r.exit_code
                    && c.stdout == r.stdout
                    && !c.timed_out
                    && !r.timed_out =>
            {
                passed += 1;
            }
            _ => return (false, passed),
        }
    }
    (passed == cases.max(1) as u64 && cases > 0, passed)
}

fn collect_rust_files(root: &std::path::PathBuf) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &std::path::PathBuf, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(read) = std::fs::read_dir(dir) {
            for entry in read.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                    let name = p.file_name().and_then(|x| x.to_str()).unwrap_or_default();
                    if name.ends_with(".repair.rs") || name.ends_with(".idiom.rs") {
                        continue;
                    }
                    out.push(p);
                }
            }
        }
    }
    walk(root, &mut out);
    out.sort();
    out
}

fn matching_cpp_for_rust(input: &PathBuf, rust_path: &Path) -> Option<PathBuf> {
    let stem = rust_path.file_stem()?.to_str()?;
    collect_cpp_files(input)
        .into_iter()
        .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(stem))
}

fn ffi_rust_type(cpp: &str) -> String {
    let t = cpp.trim();
    if t.is_empty() || t == "void" {
        String::new()
    } else if t.starts_with("int") || t == "size_t" {
        "i32".to_string()
    } else if t.starts_with("long") {
        "i64".to_string()
    } else if t.starts_with("double") || t.starts_with("float") {
        "f64".to_string()
    } else if t.starts_with("bool") {
        "u8".to_string()
    } else if t.starts_with("char*") || t.contains("*") {
        "*mut std::ffi::c_char".to_string()
    } else if t.starts_with("std::string") {
        "*const std::ffi::c_char".to_string()
    } else {
        "i32".to_string()
    }
}

fn extract_top_level_functions(
    source: &str,
) -> Vec<(
    String,      /*ret*/
    String,      /*name*/
    Vec<String>, /*param types*/
)> {
    let mut out = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        if !t.ends_with('{') {
            continue;
        }
        let sig = t.trim_end_matches('{').trim();
        let open = match sig.find('(') {
            Some(i) => i,
            None => continue,
        };
        let name_part = sig[..open].trim();
        let mut words: Vec<&str> = name_part.split_whitespace().collect();
        if words.len() < 2 {
            continue;
        }
        let name = words.pop().unwrap().to_string();
        let ret_type = words.join(" ");
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.is_empty() {
            continue;
        }
        let params_text = sig[open + 1..].trim().trim_end_matches(')').trim();
        let mut param_types = Vec::new();
        if !params_text.is_empty() {
            for p in params_text.split(',') {
                let p = p.trim();
                if p.is_empty() {
                    continue;
                }
                let words: Vec<&str> = p.split_whitespace().collect();
                param_types.push(words[..words.len().saturating_sub(1)].join(" "));
            }
        }
        out.push((ret_type, name, param_types));
    }
    out
}

fn ffi_bridge_for(source: &str) -> (String, String) {
    let funcs = extract_top_level_functions(source);
    // Derive a module-ish name from the basename; caller overrides path.
    let mut out = String::new();
    out.push_str("// Auto-generated FFI bridge scaffold. MANUAL REVIEW REQUIRED.\n");
    out.push_str("// All bodies initially `unimplemented!()`; lift safe logic into Rust here.\n");
    out.push_str("#![allow(unused)]\n\n");
    out.push_str("use std::ffi::c_char;\n\n");
    for (ret, name, params) in &funcs {
        let arity = params.len();
        let ffi_name = format!("codemorph_{}", name);
        let mut sig = Vec::new();
        for (i, t) in params.iter().enumerate() {
            sig.push(format!("arg{}: {}", i + 1, ffi_rust_type(t)));
        }
        let param_sig = if sig.is_empty() {
            String::new()
        } else {
            sig.join(", ")
        };
        let ret_type = ffi_rust_type(ret);
        let ret_part = if ret_trimmed_is_void(ret) {
            String::new()
        } else {
            format!(" -> {}", ret_type)
        };
        out.push_str(&format!(
            "/// Bridge for C++ function `{}` ({} args).\n#[no_mangle]\npub extern \"C\" fn {}({}){} {{\n    // TODO: forward to a safe Rust implementation.\n    unimplemented!(\"{} not yet implemented\")\n}}\n\n",
            name,
            arity,
            ffi_name,
            param_sig,
            ret_part,
            name,
        ));
    }
    if funcs.is_empty() {
        out.push_str("// No top-level functions detected.\n");
    }
    ("ffi".to_string(), out)
}

fn ret_trimmed_is_void(ret: &str) -> bool {
    ret.trim() == "void"
}

fn collect_cpp_files(root: &PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &PathBuf, out: &mut Vec<PathBuf>) {
        if let Ok(read) = std::fs::read_dir(dir) {
            for entry in read.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let p = entry.path();
                if kind.is_dir() {
                    walk(&p, out);
                } else if kind.is_file()
                    && matches!(codemorph_project::detect_language(&p), Some("c" | "cpp"))
                {
                    out.push(p);
                }
            }
        }
    }
    walk(root, &mut out);
    out.sort();
    out
}

fn role_model_generate(role: &str, prompt: &str) -> Result<String, String> {
    let model_dir = PathBuf::from(resolve_models_root()).join(role);
    let manifest_path = model_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(format!("model missing for {}", role));
    }
    let manifest = std::fs::read_to_string(&manifest_path).unwrap_or_default();
    if !manifest.contains("python-torch") {
        return Err(format!("unsupported backend for {}", role));
    }
    let python = resolve_python();
    let pythonpath = resolve_pythonpath();
    use codemorph_core::sandbox::{ExecutionLimits, ExecutionRequest};
    use codemorph_sandbox::Sandbox;
    let req = ExecutionRequest {
        program: python.clone(),
        args: vec![
            "-m".to_string(),
            "codemorph_ml.inference".to_string(),
            "--model-dir".to_string(),
            model_dir.display().to_string(),
            "--source-text".to_string(),
            prompt.to_string(),
        ],
        cwd: None,
        env: vec![("PYTHONPATH".to_string(), pythonpath)],
        stdin: None,
        limits: ExecutionLimits {
            timeout_ms: 120_000,
            ..Default::default()
        },
    };
    let res = Sandbox::execute(&req).map_err(|e| e.to_string())?;
    if res.timed_out {
        return Err(format!("{} timeout", role));
    }
    if res.exit_code != Some(0) {
        return Err(res.stderr);
    }
    if res.stdout.trim().is_empty() {
        return Err(format!("{} empty", role));
    }
    Ok(res.stdout)
}
