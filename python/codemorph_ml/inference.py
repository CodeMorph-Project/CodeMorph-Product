from __future__ import annotations

import argparse
import json
from pathlib import Path

import torch

from codemorph_ml.model_arch import SmallSeq2Seq
from codemorph_ml.tokenizer_simple import SimpleCharTokenizer


SIZE_PARAMS = {
    "tiny": {"embedding_dim": 64, "hidden_dim": 128, "encoder_layers": 1, "decoder_layers": 1, "dropout": 0.0},
    "10m": {"embedding_dim": 128, "hidden_dim": 384, "encoder_layers": 2, "decoder_layers": 2, "dropout": 0.1},
    "30m": {"embedding_dim": 192, "hidden_dim": 768, "encoder_layers": 4, "decoder_layers": 4, "dropout": 0.1},
    "60m": {"embedding_dim": 256, "hidden_dim": 1024, "encoder_layers": 6, "decoder_layers": 6, "dropout": 0.1},
    "100m": {"embedding_dim": 384, "hidden_dim": 1536, "encoder_layers": 8, "decoder_layers": 8, "dropout": 0.1},
}


def load_artifact(model_dir: Path):
    tokenizer = SimpleCharTokenizer.load(model_dir / "tokenizer.json")
    config = json.loads((model_dir / "model_config.json").read_text(encoding="utf-8"))
    params = {
        "vocab_size": len(tokenizer.token_to_id),
        "embedding_dim": config["embedding_dim"],
        "hidden_dim": config["hidden_dim"],
        "encoder_layers": config["encoder_layers"],
        "decoder_layers": config["decoder_layers"],
        "dropout": config.get("dropout", 0.0),
    }
    model = SmallSeq2Seq(**params)
    model.load_state_dict(torch.load(model_dir / "weights.pt", map_location="cpu"))
    model.eval()
    return model, tokenizer


def main() -> int:
    parser = argparse.ArgumentParser(description="Run a trained CodeMorph model")
    parser.add_argument("--model-dir", required=True)
    parser.add_argument("--source-file", default=None)
    parser.add_argument("--source-text", default=None)
    parser.add_argument("--max-length", type=int, default=512)
    args = parser.parse_args()

    source = args.source_text
    if args.source_file:
        source = Path(args.source_file).read_text(encoding="utf-8")
    if source is None:
        raise ValueError("--source-file or --source-text is required")

    model_dir = Path(args.model_dir).resolve()
    model, tokenizer = load_artifact(model_dir)
    src_ids = torch.tensor([tokenizer.encode(source)], dtype=torch.long)
    generated = model.generate(src_ids, max_length=args.max_length, eos_id=tokenizer.token_to_id["<eos>"])
    print(tokenizer.decode(generated))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
