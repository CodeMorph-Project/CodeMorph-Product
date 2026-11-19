# ADR-0007 — Neural-only migration

## Status

Accepted 2026-11-17. Supersedes the deterministic-first approach described in
ADR-0006.

## Context

The product shipped a deterministic C++ transpiler with a hard-coded supported
subset. It could not cover STL containers, classes, templates or library-scale
code, and its `unsupported construct` reports were not real translations.
A separate Ollama teacher existed for experiments, which meant migration quality
depended on an external model.

## Decision

Remove the deterministic translator and all external teacher calls from the
migration path. `codemorph migrate` translates only with a locally installed,
versioned model artifact selected by `--model-dir` or the model registry.

Compilers and seeded stdout/exit-code differentials remain the acceptance
authority. A candidate that fails these checks is reported as `failed` and is
not written as a partial Rust file.

## Consequences

- Every migration now requires a local model artifact; there is no offline
  algorithmic fallback.
- Model quality and dataset diversity directly determine migration coverage.
- Artifacts must declare `schema_version`, `model_version`,
  `training_config_version`, `dataset_version`, `language_pair` and
  `architecture` so a migration can be reproduced.
- The language-pair registry in `language_pairs/` is the extension point for
  future source/target pairs.
