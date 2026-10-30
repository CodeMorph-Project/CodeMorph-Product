# Evaluation

Evaluation is based on behavioral and operational metrics, not training loss alone:

- compile@k
- test@k
- differential-pass@k
- fuzz-pass
- safe Rust rate
- performance ratio
- migration success rate

Metrics are sliced by feature category, including pointers, RAII, templates, inheritance, STL, and ownership. Permanent held-out benchmarks are immutable.