# Security

Source and generated code are untrusted. Generic source acquisition must not blindly download entire repositories without license and provenance handling. Generated code can only run through the sandbox abstraction.

Sandbox requirements:

- process limits
- CPU limits
- memory limits
- filesystem isolation
- network disabled by default
- timeout enforcement
- cleanup of temporary artifacts

macOS uses the available safe backend. Linux can use containers or VMs later.