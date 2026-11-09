# Migration Engine

A C++ project is analyzed into Project IR, decomposed into migration units ordered by dependencies, translated incrementally, repaired against compiler diagnostics, tested, fuzzed, benchmarked, and assembled into a Rust crate or workspace.

The engine receives a minimal relevant context for each unit. It never passes an entire large project to a model as one prompt. Budgets limit candidates, repair iterations, tokens, time, and verification work.

## Deterministic-first translation

The runtime translation path is deterministic and model-free. `translate_cpp_to_rust`
tokenizes the input, detects unsupported constructs, parses a conservative
subset, and emits Rust. Anything outside the subset returns
`Unsupported(reason)` — the engine never emits partial or fabricated Rust.

The experimental neural translator is an opt-in *proposer* only
(`CODEMORPH_USE_MODEL=1`). Its output must still pass the same compiler and
behavior checks. It is not the default and is not trusted on its own.

## Supported subset

- `#include` / `using namespace std;` / comments
- `int main()` and free functions
- scalar and `std::string` declarations (including `int a, b;`)
- `std::cin >> ...` and `std::cout << ...` chains
- full expression precedence, ternary, `++`/`--`
- `if`/`else`, `while`, `do-while`, C-style and range `for`
- simple `struct`s with methods and aggregate initialization

Unsupported constructs (templates, `class`, STL containers, `string_view`,
`istringstream`, `unique_ptr`, `variant`, lambdas, `operator new`,
`reinterpret_cast`, concepts, …) are reported by name.

## File statuses

Each migrated file is `verified` (compiles and matches behavior), `compiled`
(compiles, behavior unchecked), `unsupported` (outside the subset, no output
written), or `failed` (attempted and rejected). See `docs/USAGE.md`.

## Implementation status (current)

The current implementation is a **CLI monolith**: streaming `migrate` in `crates/codemorph-cli/src/commands.rs`, backed by the deterministic translator in `crates/codemorph-cli/src/translate.rs`. `codemorph-orchestrate` is an in-memory state machine for job tracking only — it does *not* yet orchestrate units/budgets. Architecture split into per-language crates is tracked under `docs/decisions/` as future work (specialized-models doc).
