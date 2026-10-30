# Orchestration

The first orchestrator is a deterministic state machine. LLMs are not required for orchestration. States include project analysis, graph construction, unit selection, ownership analysis, translation, compilation, repair, testing, fuzzing, benchmarking, acceptance, retry, escalation, and completion.

Every transition is logged. State is persisted so the pipeline can restart. The user-facing process appears as one command with progress reporting.