# Roadmap

## M0 - Architecture and repository foundation

Status: completed initial foundation

- repository structure
- AGENTS.md
- architecture docs
- ADRs
- CI/local test workflow

## M1 - Compilation and sandbox

Status: basic adapters and CLI implemented

- compiler abstractions for C++ and Rust
- fixture projects
- sandbox execution
- basic CLI

## M2 - Differential testing

Status: initial infrastructure implemented

- common input encoding
- C++/Rust harness protocol
- deterministic generators
- mismatch reporting

## M3 - Verification pipeline

Status: initial infrastructure implemented

- structured verification results
- pipeline orchestration at function level
- regression artifacts

## M4 - Dataset schema and provenance

Status: initial infrastructure implemented

- versioned schema
- content-addressable storage
- license policy enforcement
- manifests

## M5 - Teacher backend and synthetic data

- teacher backend interface
- candidate generation
- feature tagging
- rejected candidate tracking

## M6 - Dataset factory end to end

- raw to verified pipeline
- deduplication
- split generation

## M7 - Tokenizer and training baseline

- tokenizer artifacts
- training runner
- MLX/PyTorch backend

## M8 - Tiny baseline model

- train first small model
- evaluation

## M9 - Inference backend

- local inference
- candidate counts

## M10 - Translator evaluation

- compile/test/differential metrics
- benchmark suite

## M11 - Project analyzer and Project IR

Status: initial infrastructure implemented

- multi-file analysis
- dependencies and graph

## M12 - Multi-file migration engine

- decomposition
- ordering
- assembly

## M13 - Repair loop

- rustc diagnostics to patch
- repeated repair attempts
- acceptance budgets

## M14 - Ownership subsystem

Status: initial infrastructure implemented

- deterministic evidence
- ownership expert interface

## M15 - Fuzzing and performance verification

Status: initial infrastructure implemented

- input shrinking
- performance reporting

## M16 - Multiple specialized model registry

Status: initial infrastructure implemented

- translator, repair, ownership, idiomatizer, performance, planner roles

## M17 - Unified orchestrator

Status: initial infrastructure implemented

- state persistence
- restartability
- deterministic state machine

## M18 - Full migration CLI

- migrate ./project
- reports and artifacts

## M19 - Experiment automation

- model comparison
- ablations

## M20 - Hardening

- security review
- reproducibility validation
- documentation polish