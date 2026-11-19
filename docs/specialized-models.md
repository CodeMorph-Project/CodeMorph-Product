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
| `security`     | Security-relevant constructs and checks             | reserved (no automatic hook yet)          |

## Current status

As of the 2026-11-17 pass, models are versioned `TransformerSeq2Seq-v2`
artifacts. The translator is `30m-v2-20261117` (~30 M parameters) trained on
compiler-verified C++/Rust pairs. The deterministic transpiler and all external
teacher models have been removed: translation is performed by the local model
alone.

Compilation and behavior verification remain the authority. Specialized models
are *proposers* — output is never trusted until the verifier accepts it.

## Design targets (roadmap)

- **API/library mapping model**: map `std::vector`, `std::map`, `std::string_view`,
  plus enterprise frameworks (Boost, Qt, POCO) to idiomatic Rust crates
  (`Vec`, `HashMap`, `&str`, `tokio`, ...). This is now learned from verified
  parallel corpora rather than emitted by an algorithmic translator.
- **Unsafe detection model**: walk ownership/pointer graphs; mark safe regions
  with lifetimes and isolate genuinely unsafe regions behind `unsafe` with
  checked invariants.
- **FFI/bridge model**: generate `extern "C"` bridges so migration can be
  incremental. Reserved as `--stage ffi` for future work.

## Versioning

Training artifacts declare `schema_version`, `model_version`,
`training_config_version`, `dataset_version`, `language_pair`, `architecture`
and `parameter_count` in both `model_config.json` and `manifest.json`.
Directory names are `outputs/model_artifacts/<role>-<version>`. Configs live in
`language_pairs/<pair>/configs/` with the same version fields. Models are
swapped through the validated installer, never committed. The runtime prefers
an explicit `--model-dir`, then the newest `models/translator-*` artifact.
