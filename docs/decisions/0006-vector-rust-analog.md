# ADR-0006 — `std::vector` to Rust `Vec<T>` mapping

## Status

Superseded on 2026-11-17: the deterministic translator was removed and
translation is now neural-only. Retained as the semantic rule for building
verified C++/Rust training pairs and for the verifier contract.

## Context

The deterministic translator reported every `std::vector` as unsupported,
while the specialized-model docs proposed mapping it to `Vec<T>`. The concern
was whether using a Rust analog changes program behavior.

## Decision

Map only the operations whose Rust analog is provably equivalent for the
supported scalar element types, and let the compiler + seeded stdout
differential decide acceptance. Unsupported operations remain rejected with
`unsupported construct: … (suggest: Vec<T>)`; no partial Rust is emitted.

| C++ | Rust | Behavior note |
| --- | --- | --- |
| `std::vector<T> v;` | `let mut v: Vec<T> = Vec::new();` | `T` limited to `int/long/double/bool/char` → `i32/i64/f64/bool/char` |
| `v = {a, b, c};` | `vec![a, b, c]` | same element order |
| `v.push_back(x)` | `v.push(x)` | amortized append; same observable order |
| `v.size()` | `v.len()` | `usize` vs `size_t`, same integer output |
| `v[i]` read/write | `v[i]` | same indexing; out-of-range is UB in C++ and panics in Rust (both are rejected by the differential on seeded inputs) |
| `v.clear()` | `v.clear()` | same effect |
| `v.pop_back()` | `v.pop()` | same effect on non-empty vectors |
| `for (T x : v)` | `for x in &v` | element-by-value copy; pointer deref in body emitted as `*x` |
| `for (T &x : v)` | `for x in &mut v` | mutation visible after the loop |

Nested `std::vector<std::vector<T>>`, non-scalar element types, iterators,
`insert`/`erase`, and allocator/performance semantics are explicitly out of
scope. The Rust `Vec<T>` has different growth strategy and memory layout from
`std::vector<T>`, but those are not observable in stdout for the supported
subset. Where a difference could be observed (e.g. overflow on `size()`),
the behavior verifier rejects the candidate.

`using namespace std;` now also makes bare `cout`, `cin` and `endl` resolve to
their `std::` forms, matching the dominant style in the example corpus.

## Consequences

- `std::vector` programs in the beginner subset now migrate and verify
  end to end (`verified`, not merely `compiled`).
- Other STL containers (`unordered_map`, `map`, `set`) stay unsupported and are
  reported with a crate suggestion.
- Every mapping is covered by unit tests and at least one `--full` migration
  integration test that compares behavior.
