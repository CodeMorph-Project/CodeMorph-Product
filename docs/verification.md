# Verification

Compilation is necessary but not sufficient. A candidate must pass the relevant configured pipeline:

- compilation
- static checks
- deterministic tests
- generated edge cases
- differential testing
- property testing when available
- fuzzing
- sanitizers
- performance benchmarking
- quality assessment

Each stage returns a structured result with rejection reasons. Behavioral disagreement rejects the candidate even if tests pass.

## Result Shape

`VerificationResult` records compilation status, test status, differential status, fuzz iterations, sanitizer status, static analysis status, benchmark result, rust quality, confidence, and rejection reasons.

## Differential Testing

C++ and Rust executions use the same inputs and comparable outputs. Random input generation is seeded and reproducible. Edge cases are explicitly tested. Differential results increase confidence but do not constitute proof.