from __future__ import annotations

import json
import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import List

PRETOKENIZER_RE = re.compile(r"\w+|[^\w\s]|\s+")


@dataclass
class BpeTokenizer:
    merges: list[tuple[str, str]]
    token_to_id: dict[str, int]
    id_to_token: dict[int, str]

    @classmethod
    def train(cls, texts: List[str], vocab_size: int = 4096, num_merges: int = 120):
        word_split = []
        for text in texts:
            word_split.extend(PRETOKENIZER_RE.findall(text))

        tokenized = []
        for word in word_split:
            tokenized.append(list(word))

        pair_counts = Counter()
        for word_tokens in tokenized:
            for i in range(len(word_tokens) - 1):
                pair_counts[(word_tokens[i], word_tokens[i + 1])] += 1

        merges = []
        for _ in range(num_merges):
            if not pair_counts:
                break
            best_pair, best_count = max(pair_counts.items(), key=lambda item: item[1])
            if best_count < 2:
                break
            merges.append(best_pair)
            new_token = best_pair[0] + best_pair[1]

            new_tokenized = []
            for word_tokens in tokenized:
                merged = []
                i = 0
                while i < len(word_tokens):
                    if i < len(word_tokens) - 1 and (word_tokens[i], word_tokens[i + 1]) == best_pair:
                        merged.append(new_token)
                        i += 2
                    else:
                        merged.append(word_tokens[i])
                        i += 1
                new_tokenized.append(merged)
            tokenized = new_tokenized

            pair_counts = Counter()
            for word_tokens in tokenized:
                for i in range(len(word_tokens) - 1):
                    pair_counts[(word_tokens[i], word_tokens[i + 1])] += 1

        base_tokens = sorted({ch for text in texts for ch in text} | {"<pad>", "<bos>", "<eos>", "<unk>"})
        tokens = list(base_tokens)
        for left, right in merges:
            tokens.append(left + right)
        tokens = list(dict.fromkeys(tokens))
        if len(tokens) > vocab_size:
            tokens = tokens[:vocab_size]
            token_set = set(tokens)
            merges = [(a, b) for a, b in merges if (a + b) in token_set]

        token_to_id = {token: idx for idx, token in enumerate(tokens)}
        token_to_id.setdefault("<pad>", len(token_to_id))
        token_to_id.setdefault("<bos>", len(token_to_id))
        token_to_id.setdefault("<eos>", len(token_to_id))
        token_to_id.setdefault("<unk>", len(token_to_id))
        id_to_token = {idx: token for token, idx in token_to_id.items()}
        return cls(merges=merges, token_to_id=token_to_id, id_to_token=id_to_token)

    def _encode_single(self, text: str) -> list[str]:
        tokens = []
        for word in PRETOKENIZER_RE.findall(text):
            chars = list(word)
            for left, right in self.merges:
                merged = []
                i = 0
                while i < len(chars):
                    if i < len(chars) - 1 and (chars[i], chars[i + 1]) == (left, right):
                        merged.append(left + right)
                        i += 2
                    else:
                        merged.append(chars[i])
                        i += 1
                chars = merged
            tokens.extend(chars)
        return tokens

    def encode(self, text: str, add_bos: bool = True, add_eos: bool = True) -> list[int]:
        pieces = self._encode_single(text)
        ids = [self.token_to_id.get(piece, self.token_to_id["<unk>"]) for piece in pieces]
        if add_bos:
            ids = [self.token_to_id["<bos>"]] + ids
        if add_eos:
            ids = ids + [self.token_to_id["<eos>"]]
        return ids

    def decode(self, ids: list[int]) -> str:
        pieces = []
        for token_id in ids:
            token = self.id_to_token.get(token_id, "")
            if token in {"<pad>", "<bos>", "<eos>", "<unk>"}:
                continue
            pieces.append(token)
        return "".join(pieces)

    def save(self, path: Path) -> None:
        payload = {
            "merges": self.merges,
            "token_to_id": self.token_to_id,
        }
        path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")

    @classmethod
    def load(cls, path: Path) -> "BpeTokenizer":
        payload = json.loads(path.read_text(encoding="utf-8"))
        token_to_id = payload["token_to_id"]
        id_to_token = {int(v): k for k, v in token_to_id.items()}
        merges = [tuple(x) for x in payload["merges"]]
        return cls(merges=merges, token_to_id=token_to_id, id_to_token=id_to_token)
