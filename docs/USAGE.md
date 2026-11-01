# CodeMorph Usage

## migrate

```sh
codemorph migrate <INPUT> --output <OUTPUT> [OPTIONS]
```

Stages:
- `--stage all` (default): analyze + translate + repair-attempts + report
- `--stage analyze`: writes `.codemorph/project_ir.json` and stops
- `--stage translate`: writes `.codemorph/stages/translate.json`
- `--stage repair`: repairs existing Rust outputs
- `--stage verify`: compares outputs behaviorally

Options:
- `--full`
- `--use-ollama`
- `--cases <N>`
- `--seed <N>`

## diff-test

```sh
codemorph diff-test --cpp-source '...' --rust-source '...' --cases 20 --seed 7
```

## benchmark

```sh
codemorph benchmark --cpp-source '...' --rust-source '...' --iterations 20
```

## models

```sh
codemorph models validate
```
