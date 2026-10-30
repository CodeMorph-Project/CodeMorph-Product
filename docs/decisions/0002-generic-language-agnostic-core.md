# ADR 0002: Generic Language-Agnostic Core

Status: Accepted
Date: 2026-10-30

Context:
C++ to Rust is only the first direction. Future directions may include C to Rust, Java to Kotlin, and Python to Rust.

Decision:
Generic core operates only on language-agnostic abstractions. C++ and Rust implementations live in adapters and translation-direction plugins. Generic code must not reference C++, Rust, RAII, ownership, or lifetimes.

Consequences:
Adding a language requires an adapter plus a direction plugin. Generic refactors are harder now, but later additions avoid core rewrites.