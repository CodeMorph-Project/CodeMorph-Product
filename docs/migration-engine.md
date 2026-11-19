# Migration Engine

The current CLI analyzes C++ inputs and translates supported files to Rust.
Translation uses a local CodeMorph artifact or an explicitly enabled
[OpenAI-compatible endpoint](model-backends.md). The compiler checks each
candidate. With `--full`, the CLI compares stdout and exit status on seeded
inputs. Run `--stage verify` to repeat the behavioral comparison from a
saved translation report.

## Model contract

Artifacts are versioned and declare:

- `schema_version`, `model_version`, `training_config_version`
- `language_pair` (currently `cpp-rust`)
- `architecture` (`TransformerSeq2Seq-v2`)
- `parameter_count`, tokenizer, context length and file hashes

The current translator is `translator-30m-v2-20261117`, a ~30m-parameter
encoder-decoder Transformer trained on compiler-verified C++/Rust pairs.

## File statuses

Each migrated file is `verified` (compiles and matches behavior), `compiled`
(compiles, behavior unchecked), `unsupported` (reserved for future language
pairs), or `failed` (model candidate rejected). See `docs/USAGE.md`.

## Implementation status

The runtime is a CLI monolith in `crates/codemorph-cli/src/commands.rs`.
`codemorph-orchestrate` remains an in-memory state machine for job tracking.
Multi-language preparation lives in `language_pairs/` at the workspace root;
only `cpp-rust` is active, other pairs are declared as planned.
