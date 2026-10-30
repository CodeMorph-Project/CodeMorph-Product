# Migration Engine

A C++ project is analyzed into Project IR, decomposed into migration units ordered by dependencies, translated incrementally, repaired against compiler diagnostics, tested, fuzzed, benchmarked, and assembled into a Rust crate or workspace.

The engine receives a minimal relevant context for each unit. It never passes an entire large project to a model as one prompt. Budgets limit candidates, repair iterations, tokens, time, and verification work.