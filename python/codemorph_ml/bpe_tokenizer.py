from __future__ import annotations

import json
import random
import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import List

LEGACY_PATTERN = r"\w+|[^\w\s]|\s+"
DIGIT_PATTERN = r"\d|[A-Za-z_]\w*|[^\w\s]|\s+"
CHAR_PATTERN = r"(?s)."
PRETOKENIZER_RE = re.compile(LEGACY_PATTERN)


@dataclass
class BpeTokenizer:
    merges: list[tuple[str, str]]
    token_to_id: dict[str, int]
    id_to_token: dict[int, str]
    pretokenizer_pattern: str = LEGACY_PATTERN

    SPECIAL = ("<pad>", "<bos>", "<eos>", "<unk>")

    @classmethod
    def character(cls, texts: List[str]) -> "BpeTokenizer":
        """Character-level tokenizer: every source character stays independently copyable.

        BPE merges such as "fu" in "0xdeadbeefu" prevent the pointer head from
        reproducing "f" and "u" separately in the Rust target. With no merges,
        arbitrary source spans can be copied token-for-token.
        """
        chars = sorted({ch for text in texts for ch in text})
        token_to_id = {token: i for i, token in enumerate(cls.SPECIAL)}
        for ch in chars:
            if ch not in token_to_id:
                token_to_id[ch] = len(token_to_id)
        id_to_token = {v: k for k, v in token_to_id.items()}
        return cls(merges=[], token_to_id=token_to_id, id_to_token=id_to_token,
                   pretokenizer_pattern=CHAR_PATTERN)

    @classmethod
    def train(cls, texts: List[str], vocab_size: int = 4096,
              num_merges: int | None = None, split_digits: bool = False):
        """Train a BPE tokenizer with stable special-token ids.

        `<pad>=0`, `<bos>=1`, `<eos>=2`, `<unk>=3` are guaranteed so the model
        can rely on them. A deterministic sample keeps training fast on large
        corpora while preserving coverage of the code alphabet.
        """
        sample = texts
        if len(sample) > 2000:
            sample = random.Random(20261117).sample(sample, 2000)
        pattern = DIGIT_PATTERN if split_digits else LEGACY_PATTERN
        pretokenizer = re.compile(pattern)
        word_split = []
        for text in sample:
            word_split.extend(pretokenizer.findall(text))
        tokenized = [list(word) for word in word_split]

        pair_counts = Counter()
        for word_tokens in tokenized:
            for i in range(len(word_tokens) - 1):
                pair_counts[(word_tokens[i], word_tokens[i + 1])] += 1

        if num_merges is None:
            num_merges = max(0, vocab_size - 256)
        merges: list[tuple[str, str]] = []
        for _ in range(num_merges):
            if not pair_counts:
                break
            best_pair, best_count = max(pair_counts.items(), key=lambda item: (item[1], item[0]))
            if best_count < 2:
                break
            merges.append(best_pair)
            new_token = best_pair[0] + best_pair[1]
            changed: list[tuple[list[str], list[str]]] = []
            for index, word_tokens in enumerate(tokenized):
                if not any(word_tokens[i] == best_pair[0] and word_tokens[i + 1] == best_pair[1]
                           for i in range(len(word_tokens) - 1)):
                    continue
                merged = []
                i = 0
                while i < len(word_tokens):
                    if i < len(word_tokens) - 1 and (word_tokens[i], word_tokens[i + 1]) == best_pair:
                        merged.append(new_token)
                        i += 2
                    else:
                        merged.append(word_tokens[i])
                        i += 1
                changed.append((index, merged))
            for index, merged in changed:
                old = tokenized[index]
                for i in range(len(old) - 1):
                    pair = (old[i], old[i + 1])
                    pair_counts[pair] -= 1
                    if pair_counts[pair] <= 0:
                        del pair_counts[pair]
                tokenized[index] = merged
                for i in range(len(merged) - 1):
                    pair_counts[(merged[i], merged[i + 1])] += 1

        base_tokens = sorted({ch for text in sample for ch in text} - set(cls.SPECIAL))
        tokens = list(cls.SPECIAL) + base_tokens
        for left, right in merges:
            token = left + right
            if token not in tokens:
                tokens.append(token)
        if len(tokens) > vocab_size:
            tokens = tokens[:vocab_size]
            token_set = set(tokens)
            merges = [(a, b) for a, b in merges if (a + b) in token_set]

        token_to_id = {token: idx for idx, token in enumerate(tokens)}
        for special in cls.SPECIAL:
            if special not in token_to_id:
                token_to_id[special] = len(token_to_id)
        id_to_token = {idx: token for token, idx in token_to_id.items()}
        return cls(merges=merges, token_to_id=token_to_id, id_to_token=id_to_token,
                   pretokenizer_pattern=pattern)

    def _encode_single(self, text: str) -> list[str]:
        ranks = {pair: index for index, pair in enumerate(self.merges)}
        tokens: list[str] = []
        pretokenizer = re.compile(self.pretokenizer_pattern)
        for word in pretokenizer.findall(text):
            chars = list(word)
            while len(chars) > 1:
                best_rank = len(ranks) + 1
                best_pair = None
                for i in range(len(chars) - 1):
                    rank = ranks.get((chars[i], chars[i + 1]), len(ranks) + 1)
                    if rank < best_rank:
                        best_rank, best_pair = rank, (chars[i], chars[i + 1])
                if best_pair is None:
                    break
                merged = []
                i = 0
                while i < len(chars):
                    if i < len(chars) - 1 and (chars[i], chars[i + 1]) == best_pair:
                        merged.append(chars[i] + chars[i + 1])
                        i += 2
                    else:
                        merged.append(chars[i])
                        i += 1
                if len(merged) == len(chars):
                    break
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
            "pretokenizer_pattern": self.pretokenizer_pattern,
        }
        path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")

    @classmethod
    def load(cls, path: Path) -> "BpeTokenizer":
        payload = json.loads(path.read_text(encoding="utf-8"))
        token_to_id = payload["token_to_id"]
        id_to_token = {int(v): k for k, v in token_to_id.items()}
        merges = [tuple(x) for x in payload["merges"]]
        pattern = payload.get("pretokenizer_pattern", LEGACY_PATTERN)
        return cls(merges=merges, token_to_id=token_to_id, id_to_token=id_to_token,
                   pretokenizer_pattern=pattern)
