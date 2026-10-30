# ADR 0004: Missing Models Are Explicit

Status: Accepted
Date: 2026-10-30

Context:
Development and tests must run before model weights exist.

Decision:
Deterministic test backends may support tests, but production CLI/reporting must explicitly state when a model is absent. Test backends are never presented as production implementations.

Consequences:
Model status command and manifest validation are required. CI can run without model artifacts. Integration tests remain meaningful.