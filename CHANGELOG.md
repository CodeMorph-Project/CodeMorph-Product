# Changelog

## 0.2.0

- Migrate standalone C++ files to Rust using a local CodeMorph model or an
  explicitly enabled OpenAI-compatible endpoint.
- Install the CLI and Python runtime together. The migration CLI reports
  progress and saves per-stage JSON reports. Behavioral verification can
  be rerun from the translation report.
- Compile and execute verification workloads in separate Docker containers
  with no network, restricted mounts, and resource limits.
- Add English and Russian quick starts and a personal evaluation license.
  Organizations and commercial users require a separate written license.

### Verification

The local model smoke test used model
`translator-30m-v7-cppnew-final-20261117`, model schema `2.0`,
training config schema `2.0`, and dataset `cpp-new-fixed-v1`. One of
three standalone fixtures passed two seeded behavioral checks. The Rust
compiler rejected the other two candidates. Results reflect those fixtures
and that model. Model weights are distributed separately.

The compatibility tests use a local Chat Completions test server.
Behavioral checks compare observable outputs for the supplied inputs.
Projects with build graphs, linked libraries or multiple translation units
are outside the tested migration scope. Optional model-assisted stages
remain experimental.
