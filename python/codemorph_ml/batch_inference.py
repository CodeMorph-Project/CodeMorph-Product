"""Batch inference for CodeMorph seq2seq artifacts.

Loads an artifact exactly once and translates many sources, which is what the
evaluation / self-improvement loop needs.
"""

from __future__ import annotations

from pathlib import Path
from typing import Iterable, List, Tuple

import torch

from .artifact import load_artifact
from .bpe_tokenizer import BpeTokenizer
from .source_normalize import strip_includes


def load_model(model_dir: Path, device: str = "cpu") -> Tuple[torch.nn.Module, BpeTokenizer]:
    return load_artifact(model_dir, device=device)


def translate_many(
    model_dir: Path,
    sources: Iterable[str],
    max_source_length: int = 512,
    max_target_length: int = 512,
    device: str = "cpu",
    batch_size: int = 1,
) -> List[str]:
    """Translate sources with a single model load (greedy decoding)."""
    sources = list(sources)
    if not sources:
        return []
    model, tokenizer = load_model(model_dir, device=device)
    eos_id = tokenizer.token_to_id["<eos>"]
    results: List[str] = []
    with torch.no_grad():
        for start in range(0, len(sources), batch_size):
            chunk = sources[start:start + batch_size]
            for source in chunk:
                ids = tokenizer.encode(strip_includes(source), add_bos=True, add_eos=True)[:max_source_length]
                tensor = torch.tensor([ids], dtype=torch.long, device=device)
                generated = model.generate(tensor, max_length=max_target_length, eos_id=eos_id)
                results.append(tokenizer.decode([int(t) for t in generated]))
    return results
