//! Contract tests for `codemorph migrate`.
//!
//! Every test here spawns the real `codemorph` binary
//! (`env!("CARGO_BIN_EXE_codemorph")`) against a throwaway fixture and asserts
//! on the artifacts it actually writes. Nothing is mocked and no model backend
//! is involved: the child process gets `CODEMORPH_USE_OLLAMA=0` and has
//! `CODEMORPH_USE_MODEL` removed from its environment.
//!
//! Artifact contract (see `docs/USAGE.md`):
//!
//! * `--stage all` (the default) writes
//!   `<out>/.codemorph/migration-report.json`, an object with the top-level
//!   keys `verified`, `compiled`, `unsupported`, `failed`, `total`, `files`.
//!   Each `files[]` entry has `source`, `output` (null when unsupported),
//!   `status` and `reason`.
//! * `--stage translate` stops after compile checking and writes
//!   `<out>/.codemorph/stages/translate.json`, an array whose entries use the
//!   same per-file fields (`source`, `output`, `status`, `reason`).
//!
//! Because `migration-report.json` is an `all`-stage artifact, the tests that
//! assert on `migration-report.json` (`unsupported_leaves_no_rust_file`,
//! `report_shape_is_stable`) run `--stage all`; the `translate` stage itself is
//! covered by `translate_stage_reports_compiled_not_verified` and by the first
//! half of `unsupported_leaves_no_rust_file`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

const HELLO_CPP: &str = "#include <iostream>\nint main() { std::cout << \"hi\\n\"; return 0; }\n";
const BAD_CPP: &str =
    "#include <unordered_map>\nint main() { std::unordered_map<int, int> m; return 0; }\n";

/// Path to the binary under test, provided by Cargo for integration tests.
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_codemorph")
}

/// Return `program` if it can be executed (`--version` exits successfully).
fn probe(program: &str) -> Option<String> {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|status| status.success())
        .map(|_| program.to_string())
}

/// Locate a usable `rustc`, honouring the environment used by the test runner.
fn rustc_path() -> Option<String> {
    let mut candidates = Vec::new();
    for var in ["CODEMORPH_RUSTC", "RUSTC"] {
        if let Ok(value) = std::env::var(var) {
            if !value.trim().is_empty() {
                candidates.push(value);
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(format!("{}/.cargo/bin/rustc", home));
    }
    candidates.push("rustc".to_string());
    candidates.iter().find_map(|c| probe(c))
}

/// Locate a usable `g++` (the behavior checker invokes it by name).
fn gpp_path() -> Option<String> {
    ["/usr/bin/g++", "g++"]
        .iter()
        .find_map(|candidate| probe(candidate))
}

/// Build a `PATH` for the child that contains the toolchain we detected plus
/// the usual system locations, then the current `PATH`.
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

/// Run `codemorph migrate <input> --output <output> --stage <stage> [--full]`
/// with a hermetic environment (no Ollama, no neural proposer).
fn run_migrate(input: &Path, output: &Path, stage: &str, full: bool) -> Output {
    let rustc = rustc_path();
    let gpp = gpp_path();

    let mut command = Command::new(binary());
    command
        .arg("migrate")
        .arg(input)
        .arg("--output")
        .arg(output)
        .arg("--stage")
        .arg(stage);
    if full {
        command.arg("--full");
    }
    command
        .env("CODEMORPH_USE_OLLAMA", "0")
        .env_remove("CODEMORPH_USE_MODEL")
        .env("PATH", child_path(rustc.as_deref(), gpp.as_deref()));
    if let Some(rustc) = &rustc {
        command.env("CODEMORPH_RUSTC", rustc);
    }
    command
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {}", binary(), e))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: codemorph exited with {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
    serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "invalid JSON in {}: {}\n--- file ---\n{}",
            path.display(),
            e,
            text
        )
    })
}

/// Create `<tmp>/src` with a single fixture file and return the input dir.
fn fixture_dir(tmp: &TempDir, name: &str, source: &str) -> PathBuf {
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).expect("create fixture dir");
    fs::write(src.join(name), source).expect("write fixture file");
    src
}

fn report_path(output: &Path) -> PathBuf {
    output.join(".codemorph").join("migration-report.json")
}

fn translate_stage_path(output: &Path) -> PathBuf {
    output
        .join(".codemorph")
        .join("stages")
        .join("translate.json")
}

// ---------------------------------------------------------------------------
// 1. hello world end-to-end: compiles, behavior-verifies, reports "verified".
// ---------------------------------------------------------------------------

#[test]
fn hello_is_verified() {
    let (rustc, gpp) = (rustc_path(), gpp_path());
    if rustc.is_none() || gpp.is_none() {
        eprintln!(
            "skipping hello_is_verified: g++ and/or rustc not available \
             (rustc={rustc:?}, g++={gpp:?})"
        );
        return;
    }

    let tmp = TempDir::new().expect("tempdir");
    let input = fixture_dir(&tmp, "hello.cpp", HELLO_CPP);
    let output = tmp.path().join("out");

    let result = run_migrate(&input, &output, "all", true);
    assert_success(&result, "migrate --full on hello.cpp");

    let out = stdout(&result);
    assert!(
        out.contains("verified: 1"),
        "expected `verified: 1` in stdout\n--- stdout ---\n{out}"
    );
    assert!(
        out.contains("failed: 0"),
        "expected `failed: 0` in stdout\n--- stdout ---\n{out}"
    );

    let rust_file = output.join("hello.rs");
    assert!(
        rust_file.is_file(),
        "expected translated file at {}",
        rust_file.display()
    );

    let report = read_json(&report_path(&output));
    assert_eq!(
        report["files"][0]["status"], "verified",
        "files[0].status should be verified, report:\n{report:#}"
    );
    assert_eq!(
        report["verified"], 1,
        "top-level verified should be 1, report:\n{report:#}"
    );
    assert_eq!(
        report["total"], 1,
        "top-level total should be 1, report:\n{report:#}"
    );
}

// ---------------------------------------------------------------------------
// 2. Unsupported input: reported with a reason, no `.rs` left behind.
// ---------------------------------------------------------------------------

#[test]
fn unsupported_leaves_no_rust_file() {
    let tmp = TempDir::new().expect("tempdir");
    let input = fixture_dir(&tmp, "bad.cpp", BAD_CPP);

    // `--stage translate`: documented artifact is stages/translate.json.
    let translate_out = tmp.path().join("out_translate");
    let result = run_migrate(&input, &translate_out, "translate", false);
    assert_success(&result, "migrate --stage translate on bad.cpp");

    let stage = read_json(&translate_stage_path(&translate_out));
    let entry = &stage[0];
    assert_eq!(
        entry["status"], "unsupported",
        "translate stage status, artifact:\n{stage:#}"
    );
    let reason = entry["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("unordered_map"),
        "reason should mention unordered_map, was: {reason:?}"
    );
    assert!(
        entry["output"].is_null(),
        "unsupported entry must have null output, artifact:\n{stage:#}"
    );
    assert!(
        !translate_out.join("bad.rs").exists(),
        "unsupported input must not leave {}",
        translate_out.join("bad.rs").display()
    );

    // `--stage all`: same per-file fields under `files[]` of the report.
    let all_out = tmp.path().join("out_all");
    let result = run_migrate(&input, &all_out, "all", false);
    assert_success(&result, "migrate --stage all on bad.cpp");

    let report = read_json(&report_path(&all_out));
    let entry = &report["files"][0];
    assert_eq!(
        entry["status"], "unsupported",
        "report files[0].status, report:\n{report:#}"
    );
    let reason = entry["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("unordered_map"),
        "files[0].reason should mention unordered_map, was: {reason:?}"
    );
    assert!(
        entry["output"].is_null(),
        "files[0].output must be null for unsupported input, report:\n{report:#}"
    );
    assert!(
        !all_out.join("bad.rs").exists(),
        "unsupported input must not leave {}",
        all_out.join("bad.rs").display()
    );
    assert_eq!(
        report["unsupported"], 1,
        "top-level unsupported should be 1, report:\n{report:#}"
    );
}

// ---------------------------------------------------------------------------
// 3. `--stage translate` compile-checks only: status is `compiled`, not
//    `verified`, because behavior verification is skipped.
// ---------------------------------------------------------------------------

#[test]
fn translate_stage_reports_compiled_not_verified() {
    let Some(_rustc) = rustc_path() else {
        eprintln!("skipping translate_stage_reports_compiled_not_verified: rustc not available");
        return;
    };

    let tmp = TempDir::new().expect("tempdir");
    let input = fixture_dir(&tmp, "hello.cpp", HELLO_CPP);
    let output = tmp.path().join("out");

    let result = run_migrate(&input, &output, "translate", false);
    assert_success(&result, "migrate --stage translate on hello.cpp");

    let out = stdout(&result);
    assert!(
        out.contains("compiled: 1"),
        "expected `compiled: 1` in stdout\n--- stdout ---\n{out}"
    );
    assert!(
        !out.contains("verified: 1"),
        "translate stage must not report `verified: 1`\n--- stdout ---\n{out}"
    );

    let stage = read_json(&translate_stage_path(&output));
    assert_eq!(
        stage[0]["status"], "compiled",
        "translate stage status, artifact:\n{stage:#}"
    );
}

// ---------------------------------------------------------------------------
// 4. Report shape: every documented top-level key is present.
// ---------------------------------------------------------------------------

#[test]
fn report_shape_is_stable() {
    let tmp = TempDir::new().expect("tempdir");
    let input = fixture_dir(&tmp, "hello.cpp", HELLO_CPP);
    let output = tmp.path().join("out");

    // `migration-report.json` is produced by the `all` stage; `--stage
    // translate` intentionally writes only stages/translate.json.
    let result = run_migrate(&input, &output, "all", false);
    assert_success(&result, "migrate --stage all on hello.cpp");

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
            "migration-report.json is missing top-level key `{key}`, report:\n{report:#}"
        );
    }

    let files = report["files"]
        .as_array()
        .unwrap_or_else(|| panic!("`files` must be an array, report:\n{report:#}"));
    assert_eq!(
        files.len(),
        1,
        "expected one file entry, report:\n{report:#}"
    );
    for field in ["source", "output", "status", "reason"] {
        assert!(
            files[0].get(field).is_some(),
            "files[0] is missing `{field}`, report:\n{report:#}"
        );
    }
}
