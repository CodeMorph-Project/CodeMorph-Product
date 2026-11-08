# CodeMorph

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

The runtime product ships a **deterministic C++→Rust transpiler** for a
conservative beginner subset (see below). It is the default translation path and
requires no model, no network, and no Ollama.

The neural translator in `models/translator/` is **experimental**: a small
sequence-to-sequence model trained in the Lab. It is disabled by default and can
only be used as an opt-in *proposer* (`CODEMORPH_USE_MODEL=1`); its output is
still validated by the compiler and behavior checks before anything is accepted.
It is not yet reliable enough to be a default, and this repository does not
pretend otherwise.

### Supported C++ subset (deterministic translator)

- `#include` / `using namespace std;` / comments
- `int main()` with `return 0;`
- scalar declarations (`int`, `long`, `short`, `unsigned`, `size_t`, `double`,
  `float`, `bool`, `char`) including comma declarations such as `int a, b;`
- `std::string` values
- `std::cin >> ...` chains (including `if (!(std::cin >> a >> b)) return 0;`)
- `std::cout << ...` chains and `std::endl`
- full expression precedence, ternary `?:`, prefix/postfix `++`/`--`
- `if` / `else if` / `else`, `while`, `do-while`, C-style `for`, range `for`
- simple `struct`s with methods and aggregate initialization
- top-level free functions

Anything outside this subset (templates, `class` with access specifiers,
`unordered_map`/`map`/`vector`, `string_view`, `istringstream`, `unique_ptr`,
`variant`, lambdas, `operator new`, `reinterpret_cast`, concepts, …) is reported
as `unsupported` with the first offending construct named. No partial or
garbage Rust is emitted for unsupported input.

## Quick start

Requirements: Rust toolchain (`cargo`, `rustc`), a C/C++ compiler (`g++` or
`clang++`).

```sh
cargo build --workspace
```

Migrate a directory of C++ files:

```sh
./target/debug/codemorph migrate ./cpp --output ./rust --full
```

`--full` enables behavior verification (compiles the original C++ and the
generated Rust and compares stdout over seeded inputs). Without `--full`, files
are only compile-checked and reported as `compiled`.

The report is written to:

```text
rust/.codemorph/migration-report.json
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
| `CODEMORPH_PYTHONPATH` | Python package path for the optional model backend |
| `CODEMORPH_PYTHON` | Python interpreter used for the optional model backend |
| `CODEMORPH_USE_MODEL=1` | Enable the experimental neural proposer for unsupported files |
| `CODEMORPH_USE_OLLAMA=1` | Enable the Ollama teacher (Lab/training only; never required for migration) |

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
python3 -m pytest python/tests
```

## Model artifacts

Models are not trained in the product runtime. They are installed from a Lab
release bundle:

```sh
python scripts/install_models.py --model-dir <artifact> --to models --role-name translator
```

See `docs/models.md` and `docs/inference.md` for the registry contract.

## License

Apache-2.0.
