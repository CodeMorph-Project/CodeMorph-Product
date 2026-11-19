//! Contract tests for the neural-only `codemorph migrate` path.
//!
//! The deterministic translator has been removed. These tests assert that:
//! * a missing local model is reported as `failed`, never silently replaced
//!   by an algorithmic translation;
//! * the migration report keeps its documented shape;
//! * unsupported language pairs are rejected;
//! * the compiler/stdout differential still detects behavior mismatches.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

const HELLO_CPP: &str = "#include <iostream>\nint main() { std::cout << \"hi\\n\"; return 0; }\n";

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_codemorph")
}

fn probe(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn rustc_path() -> Option<String> {
    if let Ok(home) = std::env::var("HOME") {
        let candidate = format!("{}/.cargo/bin/rustc", home);
        if probe(&candidate) {
            return Some(candidate);
        }
    }
    probe("rustc").then(|| "rustc".to_string())
}

fn gpp_path() -> Option<String> {
    ["/usr/bin/g++", "g++"]
        .iter()
        .find(|p| probe(p))
        .map(|s| s.to_string())
}

fn child_path(rustc: Option<&str>, gpp: Option<&str>) -> String {
    let mut dirs: Vec<String> = Vec::new();
    for tool in [rustc, gpp].into_iter().flatten() {
        if let Some(parent) = Path::new(tool).parent() {
            if !parent.as_os_str().is_empty() {
                dirs.push(parent.display().to_string());
            }
        }
    }
    for extra in ["/usr/bin", "/usr/local/bin", "/opt/homebrew/bin", "/bin"] {
        dirs.push(extra.to_string());
    }
    if let Ok(existing) = std::env::var("PATH") {
        dirs.push(existing);
    }
    dirs.join(":")
}

fn run_migrate(input: &Path, output: &Path, model_dir: Option<&Path>, extra: &[&str]) -> Output {
    let rustc = rustc_path();
    let gpp = gpp_path();
    let mut command = Command::new(binary());
    command
        .arg("migrate")
        .arg(input)
        .arg("--output")
        .arg(output)
        .env("PATH", child_path(rustc.as_deref(), gpp.as_deref()))
        .env_remove("CODEMORPH_USE_OLLAMA")
        .env_remove("CODEMORPH_USE_MODEL");
    if let Some(dir) = model_dir {
        command.arg("--model-dir").arg(dir);
    }
    command.args(extra);
    command.output().expect("spawn codemorph")
}

fn report_path(output: &Path) -> PathBuf {
    output.join(".codemorph").join("migration-report.json")
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("invalid JSON in {}: {}", path.display(), e))
}

fn fixture(tmp: &TempDir) -> PathBuf {
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("hello.cpp"), HELLO_CPP).unwrap();
    src
}

#[test]
fn missing_local_model_is_failed_not_replaced_by_algorithm() {
    let tmp = TempDir::new().unwrap();
    let input = fixture(&tmp);
    let empty_model = tmp.path().join("no-model");
    fs::create_dir_all(&empty_model).unwrap();
    let output = tmp.path().join("out");

    let result = run_migrate(&input, &output, Some(&empty_model), &["--full"]);
    assert!(
        result.status.success(),
        "migrate should still produce a report"
    );

    let report = read_json(&report_path(&output));
    assert_eq!(report["failed"], 1, "report:\n{report:#}");
    assert_eq!(report["verified"], 0, "report:\n{report:#}");
    assert_eq!(
        report["files"][0]["status"], "failed",
        "report:\n{report:#}"
    );
    let reason = report["files"][0]["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("manifest") || reason.contains("model"),
        "reason should explain the missing model, was: {reason:?}"
    );
    assert!(
        !output.join("hello.rs").exists(),
        "no garbage .rs may be left behind"
    );
}

#[test]
fn report_shape_is_stable() {
    let tmp = TempDir::new().unwrap();
    let input = fixture(&tmp);
    let empty_model = tmp.path().join("no-model");
    fs::create_dir_all(&empty_model).unwrap();
    let output = tmp.path().join("out");

    let result = run_migrate(&input, &output, Some(&empty_model), &[]);
    assert!(result.status.success());
    let report = read_json(&report_path(&output));
    for key in [
        "verified",
        "compiled",
        "unsupported",
        "failed",
        "total",
        "files",
    ] {
        assert!(
            report.get(key).is_some(),
            "missing key `{key}` in report:\n{report:#}"
        );
    }
    for field in ["source", "output", "status", "reason"] {
        assert!(
            report["files"][0].get(field).is_some(),
            "missing field `{field}`"
        );
    }
}

#[test]
fn unsupported_language_pair_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let input = fixture(&tmp);
    let output = tmp.path().join("out");
    let result = run_migrate(
        &input,
        &output,
        None,
        &["--source-lang", "python", "--target-lang", "rust"],
    );
    assert_eq!(result.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("not active yet"), "stderr: {stderr}");
}

#[test]
fn behavior_diff_detects_mismatch() {
    let (Some(rustc), Some(gpp)) = (rustc_path(), gpp_path()) else {
        eprintln!("skipping: compilers unavailable");
        return;
    };
    let cpp = "#include <iostream>\nint main(){ std::cout << 1 << \"\\n\"; }\n";
    let rs = "fn main(){ println!(\"2\"); }\n";
    let result = Command::new(binary())
        .arg("diff-test")
        .arg("--cpp-source")
        .arg(cpp)
        .arg("--rust-source")
        .arg(rs)
        .arg("--input")
        .arg("")
        .arg("--cases")
        .arg("1")
        .env("PATH", child_path(Some(&rustc), Some(&gpp)))
        .output()
        .expect("spawn diff-test");
    assert_eq!(result.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("MISMATCH"), "stdout: {stdout}");
}

#[test]
fn verify_reads_translate_artifact_and_persists_result() {
    let (Some(_), Some(_)) = (rustc_path(), gpp_path()) else {
        eprintln!("skipping: compilers unavailable");
        return;
    };
    let tmp = TempDir::new().unwrap();
    let input = fixture(&tmp);
    let output = tmp.path().join("out");
    let candidate = output.join("hello.rs");
    let stages = output.join(".codemorph/stages");
    fs::create_dir_all(&stages).unwrap();
    fs::write(&candidate, r#"fn main() { println!("hi"); }"#).unwrap();
    fs::write(
        stages.join("translate.json"),
        serde_json::to_vec(&serde_json::json!([{
            "source": input.join("hello.cpp"),
            "output": candidate,
            "status": "compiled",
        }]))
        .unwrap(),
    )
    .unwrap();
    let result = run_migrate(
        &input,
        &output,
        None,
        &["--stage", "verify", "--cases", "1"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report = read_json(&stages.join("verify.json"));
    assert_eq!(report[0]["behavior_verified"], true);
    assert_eq!(report[0]["behavior_passed_inputs"], 1);
}
