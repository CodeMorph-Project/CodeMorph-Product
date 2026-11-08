# Specialized Models (Neural Proposers)

CodeMorph trains one specialized transformer per *role*, not only the translator.
Roles and the pipeline hook that consumes (or will consume) them:

| Role           | Purpose in migration                                | Hook                                      |
| -------------- | --------------------------------------------------- | ----------------------------------------- |
| `planner`      | Decompose source into migration units + scope       | reserved (no automatic hook yet)          |
| `translator`   | Primary C++->Rust translation (default)             | `migrate` translation stage (default)     |
| `repair`       | Propose Rust code from compiler diagnostics         | invoked on compile failure via repair hook |
| `ownership`    | Classify safe/unsafe, lifetime/invariant policy      | `--stage ownership`                       |
| `idiomatizer`  | API/standard-library mapping + idiomatic Rust       | `--stage idiomatize`                      |
| `performance`  | Performance/bundle targets                          | reserved (no automatic hook yet)          |

## Current status

As of the 2026-11-08 pass, the trained role checkpoints are small Seq2Seq GRU
models (`60m`, 58 M parameters) trained on the verified corpus. Their inference
is **experimental**: in a smoke test the output is non-idiomatic and often does
not compile. They are therefore **opt-in only** (`CODEMORPH_USE_MODEL=1` for
translator proposals/repair; stages `ownership`/`idiomatize` run them directly).

Deterministic translation and behavior verification remain the authority. The
specialized models are *proposers* — their output is never trusted until the
compiler and behavior checks accept it. This aligns with AGENTS.md:
neural networks propose, compilers/tests/verifiers decide.

## Design targets (roadmap)

- **API/library mapping model**: map `std::vector`, `std::map`, `std::string_view`,
  plus enterprise frameworks (Boost, Qt, POCO) to idiomatic Rust crates
  (`Vec`, `HashMap`, `&str`, `tokio`, ...). Today the deterministic translator
  provides crate/type *hints* via `unsupported construct: X (suggest: Y)`.
- **Unsafe detection model**: walk ownership/pointer graphs; mark safe regions
  with lifetimes and isolate genuinely unsafe regions behind `unsafe` with
  checked invariants.
- **FFI/bridge model**: generate `extern "C"` bridges so migration can be
  incremental. Reserved as `--stage ffi` for future work.

## Versioning

Training artifacts carry a `version` field in `manifest.json` and a `-tag`
suffix when retraining:
`outputs/model_artifacts/<role>-<size>[-v2]` (`--run-tag v2` in `train_role.py`).
Models are swapped through the validated installer (`scripts/install_models.py`),
never committed. The runtime prefers `models/<role>/manifest.json`.
