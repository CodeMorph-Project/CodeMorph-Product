"""Version-aware artifact loader shared by CLI inference and evaluation."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Tuple

import torch

from .bpe_tokenizer import BpeTokenizer
from .model_arch import SmallSeq2Seq, TransformerSeq2Seq, PointerTransformerSeq2Seq


def load_artifact(model_dir: Path, device: str = "cpu") -> Tuple[torch.nn.Module, BpeTokenizer]:
    """Load weights, tokenizer and architecture declared in the artifact."""
    model_dir = Path(model_dir).resolve()
    tokenizer = BpeTokenizer.load(model_dir / "tokenizer.json")
    config = json.loads((model_dir / "model_config.json").read_text(encoding="utf-8"))
    arch = str(config.get("architecture", "")).lower()
    params = {"vocab_size": len(tokenizer.token_to_id)}
    if "pointer" in arch:
        model_cls = PointerTransformerSeq2Seq
    elif "transformer" in arch:
        model_cls = TransformerSeq2Seq
    else:
        model_cls = SmallSeq2Seq
    if model_cls is not SmallSeq2Seq:
        params["pad_id"] = tokenizer.token_to_id["<pad>"]
        for key in ("d_model", "nhead", "encoder_layers", "decoder_layers",
                    "dim_feedforward", "dropout", "max_length", "copy_window"):
            if key in config:
                params[key] = config[key]
        model = model_cls(**params)
    else:
        for key in ("embedding_dim", "hidden_dim", "encoder_layers", "decoder_layers", "dropout"):
            if key in config:
                params[key] = config[key]
        model = SmallSeq2Seq(**params)
    state = torch.load(model_dir / "weights.pt", map_location=device, weights_only=True)
    model.load_state_dict(state)
    model.to(device)
    model.eval()
    return model, tokenizer
