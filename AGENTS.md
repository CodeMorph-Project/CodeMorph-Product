# AGENTS.md - Operational Manual for CodeMorph

## Project Goal

CodeMorph is an experimental platform for verified neural source-to-source program translation. The first supported direction is C++ to Rust. Generic infrastructure must remain language-agnostic and must not hardcode C++/Rust concepts into the core.

## Authoritative Sources

1. This file.
2. `docs/architecture.md`
3. `docs/roadmap.md`
4. ADRs in `docs/decisions/`

When there is a conflict, code should be changed to match documented architecture unless an ADR updates the decision.

## Architectural Principles

1. Neural networks propose. Compilers, tests, and verifiers check. Orchestration is deterministic.
2. Deterministic analysis is preferred over model generation. If a property can be computed reliably, do not ask a model.
3. C++ and Rust are the first adapters only. Generic core works with `Language`, `Compiler`, `Runtime`, `Analyzer`, `Formatter`, `Linter`, `StaticAnalyzer`, `TestHarness`, `Fuzzer`, and `BenchmarkRunner`.
4. Translation directions are plugins/configurations. C++ to Rust special concepts (ownership, RAII, lifetimes) belong under `translations/cpp_to_rust/`, not core.
5. Models are external, versioned artifacts loaded through a registry. Never hardcode a checkpoint path.
6. No fake verification. Rust compilation alone never proves correctness.
7. Every stage has a CLI/API surface that can be tested without a GUI.
8. Errors preserve diagnostic context and are never silently discarded.
9. Results are reproducible. Experiments record commit, dataset version, config, seed, and environment.
10. Produced models and data must be traceable.

## Repository Structure

```
Cargo.toml
crates/
  codemorph-core/
  codemorph-cli/
  codemorph-cpp/
  codemorph-rustlang/
  codemorph-ir/
  codemorph-sandbox/
python/
  codemorph_ml/
docs/
  decisions/
configs/
fixtures/
experiments/
```

Language-specific code lives in adapter crates. Generic concepts live in core/IR.

## Module Dependency Rules

- `codemorph-core` must not depend on language adapters.
- Adapter crates may depend on core and IR.
- CLI may depend on core and all adapters.
- Python ML code must not import Rust internals. Communication is through JSON, CLI, or documented APIs only.
- Models are accessed only through `ModelBackend` and registry abstractions.
- Verification code must not import neural backends.
- Sandbox is the only approved path for executing untrusted generated code.

## Coding Conventions

- Rust: `rustfmt`, `clippy`, no `unwrap` in production paths unless invariant is documented.
- Python: `ruff`, `mypy --strict` where practical.
- Prefer structured types over strings. Every meaningful failure returns a typed result or error.
- JSON output schemas are versioned.
- No secrets or large generated artifacts are committed.
- Every public function has a documented contract.
- Avoid TODOs for missing tests; add tests before marking a stage complete.

## Testing Requirements

- Unit tests for all deterministic logic.
- Integration tests for each adapter.
- End-to-end tests for each vertical slice.
- Fixture projects for success and failure cases.
- Differential tests use fixed seeds and reproducible generators.
- No stage is complete until its relevant tests pass.

## Build, Test, Lint

```sh
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

cd python
python -m pytest -q
ruff check .
mypy src tests 2>/dev/null || true
```

## Schema Change Rules

- Add schema version fields to serialized data.
- Never silently alter an existing published schema.
- Provide migration or bump version.
- Store examples in `fixtures/` and tests.
- Rejected artifacts retain rejection reasons.

## ML Reproducibility Rules

- Every training run references an immutable dataset version.
- Seeds, configs, checkpoints, metrics, environment, and commit hash are stored.
- Every model artifact includes manifest, hashes, tokenizer version, protocol version.
- Permanent held-out benchmarks never enter training data.
- Synthetic and real data provenance must be distinguishable.

## Definition of Done

A feature is done only when:

1. Code compiles.
2. Tests pass.
3. Documentation is updated if behavior or architecture changed.
4. `git diff` contains only the intended logical change.
5. Directory remains in a buildable state.
6. No fake success, stubbed production behavior, or hardcoded demo result is presented as complete.

## Prohibited

- No stubs in production code paths claiming functionality.
- No fake model results.
- No "compiled so it is correct" verification.
- No silent data contamination.
- No running untrusted generated or downloaded code outside sandbox.
- No ignoring failing tests to continue.
- No committing caches, datasets, model weights, secrets, or temporary build artifacts.
- No leaking AI tooling references into repository artifacts.

## Error Handling

- Preserve source locations, tool versions, diagnostics, seeds, and hashes.
- Every failed verification artifact remains on disk.
- Retry limits and budgets are enforced.
- Circuit breakers stop infinite loops.

## Translation Correctness

A translation is accepted only after at least compilation and deterministic tests pass, and preferentially differential/property/fuzz checks. Behavioral disagreement must reject the candidate.

## Branch And Release Policy

- `dev` is the linear development branch.
- `main` receives only release commits; no direct feature work.
- Release commits are created from a `dev` squash/merge or curated snapshot.
- Use tags for versions, for example `v0.1.0`.
- Feature commits stay on `dev` until release.

## Git Workflow

Release policy (mandatory):

- `dev`: linear development branch. All features/fixes/docs land there.
- `main`: release-only. Features are merged into main using a normal merge commit (`git merge --no-ff dev`), so the graph shows `dev` branching from a release and joining back into `main`.
- One release commit on `main` per release: `release vX.Y.Z: ...`.
- Every release gets tag `vX.Y.Z`.
- Do not fast-forward `dev` into `main`.
- Do not keep direct feature commits on `main`.
- After release, switch back to `dev` and continue from there.


- Work in small commits.
- One logical change per commit.
- Use Conventional Commits: `type(scope): description`.
- Format and tests must pass before each commit.
- Never commit generated data, caches, weights, secrets, or environment files.
- Update roadmap and docs when milestones change.

## Model Artifacts

Production code resolves models through the registry path:

```
models/<role>/manifest.json
```

Each manifest defines role, architecture, format, tokenizer, context size, version, hashes, backend, protocol, and supported languages/features. Local model directories may be absent; the CLI must report this explicitly without pretending a test backend is a model.