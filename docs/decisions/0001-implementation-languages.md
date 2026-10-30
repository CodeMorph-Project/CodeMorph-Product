# ADR 0001: Implementation Language Split

Status: Accepted
Date: 2026-10-30

Context:
CodeMorph needs reliable compilers, tests, sandbox execution, and orchestration
while also needing a mature machine learning ecosystem for tokenization and training.

Decision:
The core, adapters, verification, sandbox, and CLI are implemented in Rust.
Training and tokenizer research are implemented in Python.
Communication occurs through versioned files and JSON/CLI boundaries, not shared process memory.

Consequences:
Rust code gains type safety and predictable performance. Python code gains access to ML libraries. The boundary remains explicit and testable.