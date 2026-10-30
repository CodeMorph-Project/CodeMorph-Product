# Vision

CodeMorph is a research and engineering platform that turns source code into
semantically checked target-language projects using small, specialized models and
deterministic software.

The first use case is C++ to Rust. The long-term goal is an arbitrary source
project input and a target-language project output, given the relevant adapters,
models, and verification strategy.

## Non-goals for early versions

- Full C++ standard support
- Unreal Engine migration
- Formal semantic equivalence proofs
- Elimination of all `unsafe` Rust
- Complex template metaprogramming
- GUI

## Success is measured

- Compile rates by candidate position
- Behavioral pass rates
- Fuzz pass rates
- Rust quality
- Performance ratios
- Automated migration coverage
- Human interventions required
- Unsafe lines of code
- Behavioral regressions