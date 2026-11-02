# CodeMorph

CodeMorph is an experimental platform for verified neural source-to-source program translation.

C++ to Rust is the first supported translation direction. The architecture is intentionally generic so additional source/target languages can be added through adapters and translation-direction plugins without rewriting the core.

## Core principle

- Neural networks propose candidates.
- Compilers and verifiers decide correctness.
- The orchestrator decides what to do next.

CodeMorph separates deterministic analysis and verification from learned generation. Verifiers are mandatory, and translated Rust is never considered correct only because it compiles.

## Current status

This repository is at an early implementation stage. Core abstractions, documentation, and the first C++/Rust vertical slices are being built. See `docs/roadmap.md`.

## Quick start

Requirements for the current C++ to Rust slice:

- Rust toolchain (`cargo`, `rustc`)
- C/C++ toolchain (`clang`, `clang++`)
- Python 3.11 or newer

Build:

```sh
cargo build --workspace
```

Run tests:

```sh
cargo test --workspace
python3 -m pytest python/tests 2>/dev/null || true
```

CLI (planned):

```sh
migrate ./cpp-project --output ./rust-project
```

## Repository layout

- `crates/`: Rust core, adapters, CLI, verification, sandbox
- `python/`: machine learning, tokenizers, training
- `docs/`: architecture, roadmap, decisions
- `configs/`: datasets, models, training, evaluation
- `fixtures/`: small reproducible test projects
- `experiments/`: experiment definitions

## License

See `LICENSE` once published.>

## Model artifacts

CodeMorph resolves models through a local registry:

```text
models/<role>/manifest.json
```

Models are not trained in product runtime. They are installed from a
CodeMorph Lab release bundle with `tools/install_models.py`.

## Quick start

Install a translator model into the runtime registry:

```sh
export CODEMORPH_OLLAMA_MODEL=beam:cloud
```

Run migration:

```sh
codemorph migrate ./cpp-project --output ./rust-project
```

Each supported source file is translated with Ollama, checked with
`rustc`, and optionally repaired once using compiler diagnostics. The final
report is written to:

```text
rust-project/.codemorph/migration-report.json
```

## Install from source

```sh
cd CodeMorph-Product
cargo build --workspace --release
./install.sh
```
