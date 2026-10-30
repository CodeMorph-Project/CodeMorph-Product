# ADR 0003: Compilation Is Necessary but Not Sufficient

Status: Accepted
Date: 2026-10-30

Context:
Translated Rust can compile while diverging in behavior from C++.

Decision:
No candidate is accepted based on compilation alone. At minimum deterministic tests and differential testing are required where applicable. Fuzzing, sanitizers, and benchmarks provide additional confidence but do not prove equivalence.

Consequences:
Evaluation and acceptance logic must store detailed stage results and rejection reasons. False positives decrease at the cost of longer runtime.