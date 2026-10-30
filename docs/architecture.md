# Architecture

## Layers

1. Generic core
2. Language adapters
3. Translation-direction plugins
4. Verification subsystems
5. Model backends and registry
6. Training subsystem (Python)
7. Migration engine and orchestrator
8. CLI

## Generic Core Abstractions

Generic code uses:

- `Language`
- `LanguageAdapter`
- `SourceAnalyzer`
- `Compiler`
- `Runtime`
- `Formatter`
- `Linter`
- `StaticAnalyzer`
- `TestHarness`
- `Fuzzer`
- `BenchmarkRunner`
- `TranslationDirection`
- `ModelRole`
- `ModelBackend`
- `VerificationStrategy`
- `PipelineStage`

No generic component names C++, Rust, or any language-specific concept.

## Language Adapters

Adapters implement capabilities discoverable at runtime:

- parse
- compile
- execute
- test
- format
- lint
- static analysis
- sanitize
- fuzz
- benchmark

Capabilities may be absent. Services query capability lists instead of assuming tools.

## Project Representation

Two levels:

- Generic Project IR: files, modules, symbols, dependencies, source locations, tests, build targets.
- Language-specific IR: extensions for templates, lifetimes, macros, annotations, JVM metadata, and similar.

Generic IR is stable and versioned. Language IR is attached as typed extensions.

## Translation Directions

A direction defines:

- source and target language
- model roles
- prompt/protocol versions
- semantic mappings
- feature taxonomy
- verification rules
- benchmarks
- migration strategy

C++ to Rust lives in `translations/cpp_to_rust/`.

## Verification

Verification is composable and separate from compilation. It combines deterministic tests, differential tests, property tests, fuzzing, static analysis, sanitizer runs, benchmarking, and quality assessment.

## Model Boundary

Core and orchestrators depend only on `ModelBackend`. Python training code communicates through stable artifacts and CLI/JSON formats.

## Security

Untrusted code execution always occurs through the sandbox abstraction. Process, CPU, memory, filesystem, network, and timeout limits apply. Containers/VMs may back Linux later.