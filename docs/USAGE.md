# CodeMorph Usage

## migrate

```sh
codemorph migrate <INPUT> --output <OUTPUT> [OPTIONS]
```

`INPUT` may be a single C/C++ file or a directory (walked recursively for
`.cpp`, `.cc`, `.cxx`, `.c`, `.h`, `.hpp`).

Stages:
- `--stage all` (default): analyze + translate + optional behavior verification + report
- `--stage analyze`: writes `.codemorph/project_ir.json` and stops
- `--stage translate`: translates and compile-checks only; writes `.codemorph/stages/translate.json`
- `--stage repair`: re-checks existing Rust outputs
- `--stage verify`: compares behavior using `migration-report.json` or `stages/translate.json` and writes `stages/verify.json`
- `--stage ownership`, `--stage idiomatize`: optional model-assisted reports

Options:
- `--full`: run behavior verification (compiles C++ and Rust, compares stdout
  over seeded inputs). Without it, results are reported as `compiled`.
- `--model-dir <path>`: translator profile to use. See [model setup](model-backends.md).
- `--source-lang` / `--target-lang`: language pair. Only `cpp`→`rust` is active.
- Stage selection: `--stage <name>` only. The `--include-stages` / `--skip-stages`
  options are accepted for compatibility but are not yet honored (reserved).
- `--cases <N>`: behavior cases per file (default 20)
- `--seed <N>`: RNG seed for behavior cases (default 7)

### Per-file statuses

Every file in `migration-report.json` has one of:

| Status | Meaning |
|---|---|
| `verified` | Generated Rust compiles **and** matches the original behavior |
| `compiled` | Generated Rust compiles; behavior was not checked |
| `unsupported` | Reserved for language pairs without a verifier; no `.rs` is written |
| `failed` | Translation was attempted but rejected (compile or behavior mismatch) |

Top-level report keys: `verified`, `compiled`, `unsupported`, `failed`, `total`,
`files`.

### Examples

```sh
codemorph migrate ./cpp --output ./rust --full
codemorph migrate ./cpp --output ./rust --stage translate
codemorph migrate ./cpp --output ./rust --stage verify
```

## diff-test

```sh
codemorph diff-test --cpp-source '...' --rust-source '...' --cases 20 --seed 7
```

Compiles both sources and compares behavior over generated inputs.

## benchmark

```sh
codemorph benchmark --cpp-source '...' --rust-source '...' --iterations 20
```

## models

```sh
codemorph models list
codemorph models validate
```

## Environment

| Variable | Meaning |
|---|---|
| `CODEMORPH_MODELS_ROOT` | Model registry root (`models/<role>/manifest.json`) |
| `CODEMORPH_PYTHONPATH` | Python path for the local model runtime |
| `CODEMORPH_PYTHON` | Python interpreter for the local model runtime |
