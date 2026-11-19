"""Shared source normalization for local neural inference.

Include directives are environment plumbing, not translation content. Removing
them lets the translator focus on the program body and makes it robust to
absolute include paths. The Rust compiler still verifies the final output.
"""

from __future__ import annotations


def strip_includes(source: str) -> str:
    lines = [line for line in source.splitlines() if not line.strip().startswith("#include")]
    return "\n".join(lines).strip("\n") + "\n"
