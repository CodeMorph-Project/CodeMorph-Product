"""A deterministic character tokenizer for small experimental models."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import List

SPECIAL_TOKENS = ["<pad>", "<bos>", "<eos>", "<unk>"]


@dataclass
class SimpleCharTokenizer:
    token_to_id: dict
    id_to_token: dict

    @classmethod
    def from_texts(cls, texts: List[str]) -> "SimpleCharTokenizer":
        vocab = set(SPECIAL_TOKENS)
        for text in texts:
            vocab.update(text)
        token_to_id = {token: idx for idx, token in enumerate(sorted(vocab))}
        id_to_token = {idx: token for token, idx in token_to_id.items()}
        return cls(token_to_id, id_to_token)

    def encode(self, text: str, add_bos: bool = True, add_eos: bool = True) -> List[int]:
        ids = [self.token_to_id.get(ch, self.token_to_id["<unk>"]) for ch in text]
        if add_bos:
            ids = [self.token_to_id["<bos>"]] + ids
        if add_eos:
            ids = ids + [self.token_to_id["<eos>"]]
        return ids

    def decode(self, ids: List[int]) -> str:
        pieces = []
        for token_id in ids:
            token = self.id_to_token.get(token_id, "")
            if token in SPECIAL_TOKENS:
                continue
            pieces.append(token)
        return "".join(pieces)

    def save(self, path: Path) -> None:
        path.write_text(json.dumps({"token_to_id": self.token_to_id}, ensure_ascii=False, indent=2), encoding="utf-8")

    @classmethod
    def load(cls, path: Path) -> "SimpleCharTokenizer":
        payload = json.loads(path.read_text(encoding="utf-8"))
        token_to_id = payload["token_to_id"]
        id_to_token = {int(v): k for k, v in token_to_id.items()}
        return cls(token_to_id, id_to_token)