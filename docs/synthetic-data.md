# Synthetic Data

The primary training scenario is:

existing C++ source -> teacher translation -> Rust candidate -> verification

Synthetic examples are classified by curriculum features such as arithmetic, strings, structs, STL, RAII, templates, ownership, threading, and atomics. Teacher outputs become valid training samples only after verification.

Alternative C++, tests, and edge cases can be generated, but independently synthesized source and target programs must not be presented as verified translation pairs.