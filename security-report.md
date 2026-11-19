# Security Report — CodeMorph-Product (dev branch) — 2026-11-09

## Scope & Method
Audited repository root `/Users/osman-nyri/Проекты/CodeMorph/CodeMorph-Product`, branch `dev` (commit `30e2b0c`).
In scope: all Rust crates (`crates/*`), Python package `python/codemorph_ml`, CLI (`crates/codemorph-cli`), sandbox adapters, model manifests, `.env` handling, dependency manifests. Out of scope: large generated artifacts under `target/`, `models/` weights themselves, live network/Ollama services, and OS-level hardening outside this repo.

Staged recon performed:
- Terrain mapping: enumerated entry points in `crates/codemorph-cli/src/main.rs` and `crates/codemorph-cli/src/commands.rs`; listed dangerous sinks (`fs::write`, `Command`, `Sandbox::execute`, `serde_json::from_str`, `torch.load`).
- Prioritized deep dive: sandbox (`codemorph-sandbox/src/lib.rs`), CLI path handling (`commands.rs`), model/inference paths (`inference.py`, `commands.rs::role_model_generate` / `generate_translation`), deserialization sites.
- Cross-cutting sweeps: `grep` for `unsafe`, `Command::new`, shell strings, `eval/exec/yaml/pickle`, secrets patterns, `torch.load`.

Tools: `cargo clippy --workspace -- -D warnings` (passed, no warnings). `cargo test --workspace` could not run in this environment (`rustc -vV`: bad CPU type in executable). No `cargo-audit`, `gitleaks`, `semgrep`, `bandit`, `pip-audit` available; dependency review done by manifest inspection only. Secrets scan covered all tracked source/config (`git grep` for API-key/private-key patterns, extension filters) plus local `.env` inventory (not committed).

## Executive Summary

| Finding | Severity | Status | Location |
|---|---|---|---|
| Sandbox is a process spawner, does not enforce declared isolation | HIGH | Confirmed via code trace | `crates/codemorph-sandbox/src/lib.rs:10` |
| `torch.load` without `weights_only` (pickle RCE via model artifacts) | MEDIUM | Confirmed via code trace | `python/codemorph_ml/inference.py:34` |
| Model backend selected by substring check; no manifest-hash validation before inference | MEDIUM | Confirmed via code trace | `crates/codemorph-cli/src/commands.rs:324` (and `:385`, `:1374`) |
| `migration-report.json` paths used to read/compile/execute arbitrary files | MEDIUM | Confirmed via code trace | `crates/codemorph-cli/src/commands.rs:641` |
| Unbounded stdout/stderr capture despite declared caps | MEDIUM | Confirmed via code trace | `crates/codemorph-sandbox/src/lib.rs:57` |
| Env-controlled interpreter/host can redirect execution/network | LOW | Confirmed via code trace | `crates/codemorph-cli/src/commands.rs:289`, `python/codemorph_ml/ollama_client.py:32` |
| `analyze()` follows directory tree/symlinks with no containment | LOW | Potential — needs runtime verification | `crates/codemorph-project/src/lib.rs:16` |
| `ffi` module name/source not sanitized in generated scaffold | LOW | Potential — needs review | `crates/codemorph-cli/src/commands.rs:680` |

## Findings

### [severity: HIGH] crates/codemorph-sandbox/src/lib.rs:10 — Sandbox ignores every declared isolation field (network, filesystem_root, memory, CPU); it only spawns the process → Enforce or remove the limits before accepting generated code as trusted.

- **Vulnerable code:**
  ```rust
  pub fn execute(request: &ExecutionRequest) -> Result<ExecutionResult, std::io::Error> {
      let mut command = Command::new(&request.program);
      command.args(&request.args);
      ...
      for (k, v) in &request.env { command.env(k, v); }
  ```
  `ExecutionLimits { network_enabled, filesystem_root, memory_bytes, cpu_seconds }` are never read anywhere in `execute()`; only `timeout_ms`, `max_stdout_bytes`, `max_stderr_bytes` have any effect. `grep` for those field names only finds them in the struct definition.
- **Vulnerability class:** insufficient execution isolation / ignored security control.
- **Attack scenario:** `migrate --stage all` compiles attacker-influenced or model-translated C++/Rust and runs the resulting binaries via `behavior_check()` (commands.rs:1199). A malicious translation containing `std::process::Command`, file reads, or network access (e.g. `std::net::TcpStream::connect`) executes on the host with the user's privileges, despite `network_enabled=false` being configured. The same applies to any user who runs `compile`/`diff_test` with hostile source.
- **Impact:** full host filesystem/network access and DoS by generated code; the "sandbox" name gives a false safety guarantee.
- **Evidence of confirmation:** static trace across `codemorph-sandbox/src/lib.rs` (lines 10–87): no branch references `limits.network_enabled`, `limits.filesystem_root`, `limits.memory_bytes`, or `limits.cpu_seconds`.
- **Fix:** either implement real containment (macOS `sandbox-exec` profile honoring `filesystem_root`/`network_enabled`, `ulimit` for memory/CPU, or a container/namespaced runner) and fail closed when unavailable, or remove these limit fields and document that execution is not isolated. At minimum, strip environment variables, set `cwd` to the requested root, and deny network via a seccomp/profile.
- **Fix status:** Routed to backend (core architecture change).

### [severity: MEDIUM] python/codemorph_ml/inference.py:34 — `torch.load` deserializes untrusted pickle artifacts without `weights_only` → RCE through a swapped/malicious `weights.pt`.

- **Vulnerable code:**
  ```python
  model.load_state_dict(torch.load(model_dir / "weights.pt", map_location="cpu"))
  ```
- **Vulnerability class:** insecure deserialization.
- **Attack scenario:** An attacker (or compromised mirror) supplies a crafted `weights.pt` whose pickle payload runs Python on load. `CODEMORPH_MODELS_ROOT` or a malicious installed model path causes `generate_translation`/`role_model_generate` to call inference on it and execute the payload with the user's privileges.
- **Impact:** arbitrary code execution in the local environment when a model artifact is loaded.
- **Evidence of confirmation:** Python 3.12 / torch 2.15.0 is installed in this environment; `weights_only=True` is supported by that torch version. No `weights_only` argument present.
- **Fix:** `torch.load(model_dir / "weights.pt", map_location="cpu", weights_only=True)`. Also validate the manifest hash **before** loading (see next finding) and reject artifacts whose hashes do not match.
- **Fix status:** Proposed narrow edit.

### [severity: MEDIUM] crates/codemorph-cli/src/commands.rs:324 — Model availability/backend is decided by substring match (`manifest.contains("python-torch")`) with no hash/manifest validation before spawning inference → attacker-controlled manifest can bypass backend selection and point inference at an arbitrary directory.

- **Vulnerable code:**
  ```rust
  let manifest = std::fs::read_to_string(&manifest_path).unwrap_or_default();
  if manifest.contains("python-torch") { ... }
  ```
  Same pattern at `commands.rs:385` (selected backend) and `commands.rs:1374` (`role_model_generate` gate: `if !manifest.contains("python-torch") { return Err(...) }`).
- **Vulnerability class:** input validation bypass / trust of untrusted local state.
- **Attack scenario:** A manifest containing the literal `"python-torch"` anywhere (even as a string value unrelated to `backend`) causes the CLI to run `python -m codemorph_ml.inference --model-dir <attacker dir>`, executing `torch.load` on the attacker's `weights.pt`. Combined with the missing manifest-hash check, replacing a single file is enough.
- **Impact:** RCE via crafted model directory; also breaks model-selection correctness.
- **Evidence of confirmation:** `ModelRegistry::validate_manifest()` (crates/codemorph-models/src/lib.rs:88) exists and performs hash checks, but the CLI inference paths never call it. Exact grep: `validate_manifest` is only invoked by `models_validate()` (`commands.rs:949`).
- **Fix:** Parse the manifest with `ModelManifest::load`, require `manifest.backend == "python-torch"`, and call `registry.validate_manifest(&manifest_path)`; abort on any error before spawning Python. Also resolve/validate model paths against `CODEMORPH_MODELS_ROOT`.
- **Fix status:** Routed to backend (touches model loading contract).

### [severity: MEDIUM] crates/codemorph-cli/src/commands.rs:641 — `verify` stage reads source/C++ and Rust paths taken from `migration-report.json` without containment, then compiles and executes them → arbitrary file read + execution of report-controlled code.

- **Vulnerable code:**
  ```rust
  let src = f.get("source").and_then(|v| v.as_str()).map(PathBuf::from);
  let out = f.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
  if let (Some(src), Some(outp)) = (src, out) {
      let source_cpp = std::fs::read_to_string(&src).unwrap_or_default();
      let generated = std::fs::read_to_string(&outp).unwrap_or_default();
      let (pass, _count) = behavior_check(&source_cpp, &generated, cases, seed);
  ```
- **Vulnerability class:** path traversal / untrusted deserialization-driven code execution.
- **Attack scenario:** Anyone who can edit `output/.codemorph/migration-report.json` (shared output dir, or a crafted project drop) sets `source` to a sensitive file path and `output` to a Rust source they control; running `migrate --stage verify` reads that file and compiles+executes the supplied Rust. Even without an obvious leak, this bypasses the intended input/output binding.
- **Impact:** arbitrary file read (best-effort; source is fed to the C++ compiler and only errors leak) plus execution of report-controlled Rust binaries.
- **Evidence of confirmation:** `src`/`outp` are converted with `PathBuf::from` and used directly; no canonicalization or prefix check against `input`/`output`.
- **Fix:** canonicalize both paths and require them to be descendants of the configured input/output roots; reject entries containing `..` components or absolute paths; treat report as untrusted.
- **Fix status:** Routed to backend.

### [severity: MEDIUM] crates/codemorph-sandbox/src/lib.rs:57 — stdout/stderr are read with `read_to_end` before caps are applied, so `max_stdout_bytes` only truncates after unbounded allocation → memory-exhaustion DoS within the timeout window.

- **Vulnerable code:**
  ```rust
  let mut out_buf = Vec::new();
  ...
  if let Some(mut s) = stdout {
      let _ = s.read_to_end(&mut out_buf); // unbounded
  }
  let trim = |buf: Vec<u8>, limit: usize| -> String {
      let slice = if buf.len() > limit { &buf[..limit] } else { &buf };
      ...
  };
  ```
- **Vulnerability class:** unbounded resource consumption / DoS.
- **Attack scenario:** A generated binary runs `yes` or otherwise streams gigabytes before the timeout; the runner allocates all of it. With 10s default timeout and 1MB caps reported to the user, actual memory use is uncontrolled.
- **Impact:** OOM/DoS of the host process; also the timeout kill path reads pipes after kill, risking hangs.
- **Evidence of confirmation:** limits are applied in `trim` after `read_to_end`, so the whole stream is buffered first.
- **Fix:** read in bounded chunks and stop at the limit (or use `take(limit+1)`), and close pipes before/after kill. Add a test for large output staying within memory.
- **Fix status:** Proposed narrow edit.

### [severity: LOW] crates/codemorph-cli/src/commands.rs:289 (resolve_python) and python/codemorph_ml/ollama_client.py:32 (from_env) — Interpreter path and teacher host are taken from environment variables without validation or allowlisting; with env control, an attacker can replace the interpreter or exfiltrate prompts to an arbitrary endpoint.

- **Vulnerable code:**
  ```rust
  if let Ok(value) = std::env::var("CODEMORPH_PYTHON") {
      if !value.trim().is_empty() { return value; }
  }
  ```
  ```python
  host=os.getenv("CODEMORPH_OLLAMA_HOST", "http://localhost:11434").rstrip("/"),
  ```
- **Vulnerability class:** unsafe configuration trust / potential SSRF.
- **Attack scenario:** Requires the attacker to already set environment variables (not an unauthenticated network adversary). If achieved, `CODEMORPH_PYTHON` makes the CLI exec an arbitrary binary, and `CODEMORPH_OLLAMA_HOST=http://internal-service/...` causes prompts/source code to be POSTed to an attacker host. Both are plausible in CI or shared shell environments.
- **Impact:** arbitrary program execution (local env) or data exfiltration on the operator's machine.
- **Evidence of confirmation:** resolved interpreter is passed as `ExecutionRequest.program` directly; Ollama host is concatenated into a request URL with no allowlist.
- **Fix:** validate interpreter path against a known allowlist or PATH resolution; restrict `CODEMORPH_OLLAMA_HOST` to localhost/private-allowed hosts by default; document trust boundary.
- **Fix status:** Routed to backend/frontend (config policy).

### [severity: LOW] crates/codemorph-project/src/lib.rs:16 — `analyze` recursively walks the tree, following `read_dir` entries without symlink checks or traversal bounds; a crafted input tree can escape the input root or explode via deep recursion.

- **Vulnerable code:**
  ```rust
  fn walk(root: &Path, dir: &Path, ir: &mut ProjectIr) {
      let Ok(read) = fs::read_dir(dir) else { return; };
      for entry in read.flatten() {
          let path = entry.path();
          if path.is_dir() { walk(root, &path, ir); }
  ```
- **Vulnerability class:** filesystem traversal / symlink following.
- **Attack scenario:** Input directory contains a symlink to `/` or a deep nesting; `analyze` may hash/read files outside the intended input, increasing exposure and possibly panicking on huge trees. Requires the operator to choose a malicious input tree.
- **Impact:** unintended file reads and potential DoS; not a remote vector by itself.
- **Evidence of confirmation:** `if path.is_dir()` follows symlinks (Rust `Path::is_dir` follows), and there is no `symlink_metadata`/depth check.
- **Fix:** use `entry.file_type()` and skip symlinked directories unless explicitly allowed; add a depth limit and an entry cap.
- **Fix status:** Routed to backend.

### [severity: LOW] crates/codemorph-cli/src/commands.rs:680 — FFI scaffold derives a bridge filename from uncontrolled source file stem without sanitization; a pathological filename can escape the output `ffi/` directory.

- **Vulnerable code:**
  ```rust
  let module_name = source_path.file_stem()...;
  ...
  let out_path = ffi_dir.join(format!("{}.rs", module_name));
  let _ = std::fs::write(&out_path, &bridge);
  ```
- **Vulnerability class:** path traversal via filename.
- **Attack scenario:** Only relevant if an operator runs `migrate --stage ffi` over a tree containing a file whose stem includes path separators or relative components. On most filesystems file stems cannot contain `/`, so exploitability is limited; module identifiers are also emitted into generated source and should be sanitized.
- **Impact:** possible write outside the FFI directory on platforms that permit traversal-like names; generated code quality issue.
- **Evidence of confirmation:** `module_name` is used in `ffi_dir.join(format!("{}.rs", module_name))` without canonicalization or allowlisting.
- **Fix:** sanitize `module_name` to ASCII identifiers (`[A-Za-z0-9_]`), reject empty/`.`/`..`, and canonicalize `out_path` under `ffi_dir`.
- **Fix status:** Proposed narrow edit candidate; routed if broader.

## Dependency Audit
No `cargo audit`/`pip-audit` available in this environment. Manifest review:
- Rust workspace pins `serde 1.0`, `serde_json 1.0`, `clap 4.5`, `tempfile 3.13`, `wait-timeout 0.2`, `which 6.0`, `sha2 0.10`, `nix 0.29` (unused), `chrono 0.4`. No known vulnerable versions identified manually; recommend installing `cargo-audit` and running `cargo audit` against `Cargo.lock` before release.
- Python (`python/pyproject.toml`) declares only `requires-python>=3.11` and no pinned dependencies; `torch` is imported by inference but not declared — reproducibility gap, not a CVE finding. `torch.load(pickle)` is the main Python risk (above).

## Secrets Scan
Scanned tracked files (`*.rs`, `*.py`, manifests, shell/config) and `git grep` patterns for API keys, private keys, bearer tokens, and high-entropy secrets. `.env` exists locally but is gitignored (not tracked); `.env.example` contains only non-secret localhost/development placeholders. No secrets found in committed source. Recommendation: keep `.env` ignored, rotate if it ever contained a real value, and add pre-commit secret scanning.

## Areas Reviewed With No Confirmed Issues
- Shell injection: `Sandbox::execute()` uses `Command::new(program).args(&args)` (no shell) for all execution, including `python_ollama_generate` (model passed as `sys.argv[1]`, prompt via `stdin`) and `role_model_generate` (`--source-text` as one argv element). Static trace confirms no `sh -c`/`cmd /c`. Ollama host is config, not shell input.
- SQL/NoSQL/command injection sinks absent (no SQL, no `eval`/`exec` of user input).
- JWT/session/authN code absent (CLI is a local tool; no auth endpoints).
- Unsafe Rust absent (`grep unsafe` across `crates/` returned no matches).
- Manifest hash algorithm is SHA-256 and validation logic correctly compares computed vs expected (`crates/codemorph-models/src/lib.rs`); only the CLI fails to invoke it before inference.
- `models/` and `datasets/` are gitignored; model weights and artifacts were not found in `git ls-files`.
- `install_models.py` not present in the product repository; model loading goes through `ModelRegistry` (manifest-based).

## Recommended Priority Order
1. Implement or remove the "sandbox" containment guarantee (HIGH).
2. Add `weights_only=True` to `torch.load` and validate manifest hashes before any model load (MEDIUM).
3. Sanitize/authorize paths read in `migrate --stage verify` (MEDIUM).
4. Replace manifest `contains("python-torch")` checks with typed `backend` checks + `validate_manifest` (MEDIUM).
5. Bound sandbox output capture (MEDIUM).
6. Harden environment-controlled interpreter/host settings and filesystem traversal (LOW).

Summary count: 1 HIGH, 4 MEDIUM, 3 LOW.
No high-severity issues can be accepted for release until the sandbox isolation finding (HIGH) is remediated; three medium findings should be closed in the same release.
