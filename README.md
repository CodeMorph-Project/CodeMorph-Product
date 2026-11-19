# CodeMorph

English · [Русский](README.ru.md)

CodeMorph is an experimental platform for **verified** source-to-source program
translation. C++ to Rust is the first supported direction; the architecture is
intentionally generic so further language pairs can be added through adapters
and translation-direction plugins.

## Core principle

- Deterministic analysis and translation are preferred over model generation.
- Neural models only ever *propose* candidates.
- Compilers and behavior tests decide correctness.
- Nothing is reported as translated unless it compiles, and nothing is reported
  as verified unless it also behaves like the original.

A translation is never accepted merely because it compiles. The migration
report distinguishes `verified` (compiles **and** matches behavior) from
`compiled` (compiles, behavior not checked) from `unsupported` (outside the
supported subset) from `failed` (attempted but rejected).

## Current status

Migration produces Rust candidates with either an installed CodeMorph artifact
or an explicitly enabled OpenAI-compatible endpoint. The first supported
direction is C++ to Rust. See the [model setup guide](docs/model-backends.md).

### Verification contract

A file is `verified` only when the generated Rust compiles and matches the
C++ program on seeded inputs. It is `compiled` when only compilation was
checked, and `failed` when the model candidate was rejected. Inspect the report before using output. A candidate can compile and still fail behavioral verification.

## Quick start

Requirements: Rust, Python 3.11+, and a running local Docker engine.

```sh
docker pull rust:1-bookworm
cargo build --locked --workspace
```

Run `./bootstrap.sh` to build and install the CLI and Python runtime.
The default executable directory is `~/.local/bin`. Override it with
`CODEMORPH_INSTALL_DIR`.

Migrate a directory of C++ files:

```sh
./target/debug/codemorph migrate ./cpp --output ./migration-output --full
```

`--full` enables behavior verification (compiles the original C++ and the
generated Rust and compares stdout over seeded inputs). Without `--full`, files
are only compile-checked and reported as `compiled`.

The report is written to:

```text
migration-output/.codemorph/migration-report.json
```

Example summary:

```text
verified: 8
compiled: 0
unsupported: 4
failed: 0
total: 12
```

### Environment variables

| Variable | Meaning |
|---|---|
| `CODEMORPH_MODELS_ROOT` | Directory containing `models/<role>/manifest.json` |
| `CODEMORPH_PYTHONPATH` | Python package path for the local model runtime |
| `CODEMORPH_PYTHON` | Python interpreter used for the local model runtime |

## Repository layout

- `crates/`: Rust core, adapters, CLI, verification, sandbox
- `python/`: model runtime, tokenizers, inference
- `docs/`: architecture, usage, roadmap, decisions
- `fixtures/`: small reproducible test projects
- `models/`: installed model registry (weights are never committed)

## Tests

```sh
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
PYTHONPATH=python python3 -m unittest discover -s python/tests
```

## Model artifacts

Models are not trained in the product runtime. They are installed from a Lab
release bundle:

```sh
python ../CodeMorph-Lab/scripts/install_models.py --model-dir ARTIFACT --to models --role-name translator
```

See `docs/models.md` and `docs/inference.md` for the registry contract.

## License

Source is available for private, noncommercial evaluation by individuals.
Organizational and commercial use requires a separate written license.
See [LICENSE](LICENSE).

## Public release status

See the [publication checks](docs/public-release.md) and the
[container execution boundary](docs/security.md).
