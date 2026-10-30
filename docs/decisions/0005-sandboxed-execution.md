# ADR 0005: All Untrusted Execution Through Sandbox

Status: Accepted
Date: 2026-10-30

Context:
Acquired sources and generated code are untrusted.

Decision:
No downloaded or generated code may execute directly on the host outside the sandbox abstraction. Process, CPU, memory, filesystem, network, and timeout limits apply.

Consequences:
Untrusted fixture execution remains safe but requires platform-specific sandbox implementations. Linux backend can use containers/VMs later.