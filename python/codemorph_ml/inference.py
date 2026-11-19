"""Run one trained CodeMorph model (CLI entry point used by the Rust CLI)."""

from __future__ import annotations

import argparse
from pathlib import Path

import torch

from .artifact import load_artifact
from .source_normalize import strip_includes


def main() -> int:
    parser = argparse.ArgumentParser(description="Run a trained CodeMorph model")
    parser.add_argument("--model-dir", required=True)
    parser.add_argument("--source-file", default=None)
    parser.add_argument("--source-text", default=None)
    parser.add_argument("--max-length", type=int, default=512)
    parser.add_argument("--device", default="auto")
    args = parser.parse_args()

    source = args.source_text
    if args.source_file:
        source = Path(args.source_file).read_text(encoding="utf-8")
    if source is None:
        raise ValueError("--source-file or --source-text is required")

    device = "mps" if args.device == "auto" and torch.backends.mps.is_available() else (
        "cpu" if args.device == "auto" else args.device)
    model_dir = Path(args.model_dir).resolve()
    model, tokenizer = load_artifact(model_dir, device=device)
    src_ids = torch.tensor([tokenizer.encode(strip_includes(source), add_bos=True, add_eos=True)],
                           dtype=torch.long, device=device)
    generated = model.generate(src_ids, max_length=args.max_length,
                               eos_id=tokenizer.token_to_id["<eos>"])
    print(tokenizer.decode([int(t) for t in generated]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
