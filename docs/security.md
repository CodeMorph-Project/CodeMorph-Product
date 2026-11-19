# Security

CodeMorph analyzes and compiles standalone C++/Rust files. Product compilers
and generated test programs run in separate Docker containers. The runner
uses a read-only root filesystem, disables network access, drops capabilities,
and limits memory, CPU, PIDs, temporary files, output and wall time. Compilation
mounts a single source file and a disposable output directory. Test programs
mount only their own executable, read-only. Neither receives model API keys.

Run a trusted Docker daemon on the same machine as the CLI and pull the
default image once with `docker pull rust:1-bookworm`. Set
`CODEMORPH_SANDBOX_IMAGE` to select another trusted local image with
`g++` and `rustc`. Use a digest to fix the toolchain for reproducible
verification. Docker startup time is included in the per-case time budget.

Inference runs in a local Python process, or calls an explicitly enabled
remote endpoint. Install only trusted model artifacts and configurations.
Use an independent disposable environment for workloads requiring a stronger
isolation boundary than the local container engine.
