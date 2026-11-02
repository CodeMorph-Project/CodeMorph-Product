"""Tokenizer evaluation helpers.

This module does not implement a production tokenizer. It measures simple
fragmentation metrics required by experiments.
"""

from __future__ import annotations

import re
from typing import List

_OPERATOR_PATTERN = re.compile(r"[{}();<>[\],:+\-*/&|^!~?=]|//|/\*|\*/")


def tokenize_simple(text: str) -> List[str]:
    return text.split()


def count_operator_fragments(text: str) -> int:
    return len(_OPERATOR_PATTERN.findall(text))


def metrics(text: str) -> dict:
    lines = [line for line in text.splitlines() if line.strip()]
    tokens = tokenize_simple(text)
    loc = len(lines) or 1
    return {
        "loc": loc,
        "tokens": len(tokens),
        "tokens_per_loc": len(tokens) / loc,
        "operators": count_operator_fragments(text),
        "avg_line_length": len(text) / loc,
    }