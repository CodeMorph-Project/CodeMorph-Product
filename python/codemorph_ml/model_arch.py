"""Sequence-to-sequence model families for CodeMorph.

Two architectures are kept deliberately:

* ``SmallSeq2Seq`` — the original GRU baseline, retained so older artifacts
  remain loadable.
* ``TransformerSeq2Seq`` — the v2 encoder-decoder Transformer used by the
  30m models. It has cross-attention and is the architecture used for
  C++ -> Rust migration.
* ``PointerTransformerSeq2Seq`` — a copy-augmented Transformer that can copy
  literals and identifiers directly from the source through attention.
"""

from __future__ import annotations

import math

import torch
import torch.nn as nn


class TransformerSeq2Seq(nn.Module):
    """A compact encoder-decoder Transformer for source-to-source translation."""

    def __init__(
        self,
        vocab_size: int,
        d_model: int = 512,
        nhead: int = 8,
        encoder_layers: int = 4,
        decoder_layers: int = 4,
        dim_feedforward: int = 2048,
        dropout: float = 0.1,
        max_length: int = 512,
        pad_id: int = 0,
        bos_id: int = 1,
    ) -> None:
        super().__init__()
        self.d_model = d_model
        self.pad_id = pad_id
        self.bos_id = bos_id
        self.max_length = max_length
        self.embedding = nn.Embedding(vocab_size, d_model, padding_idx=pad_id)
        self.position = nn.Embedding(max_length, d_model)
        encoder_layer = nn.TransformerEncoderLayer(
            d_model=d_model,
            nhead=nhead,
            dim_feedforward=dim_feedforward,
            dropout=dropout,
            batch_first=True,
            norm_first=True,
        )
        decoder_layer = nn.TransformerDecoderLayer(
            d_model=d_model,
            nhead=nhead,
            dim_feedforward=dim_feedforward,
            dropout=dropout,
            batch_first=True,
            norm_first=True,
        )
        self.encoder = nn.TransformerEncoder(encoder_layer, num_layers=encoder_layers,
                                             enable_nested_tensor=False)
        self.decoder = nn.TransformerDecoder(decoder_layer, num_layers=decoder_layers)
        self.output = nn.Linear(d_model, vocab_size)
        self._reset_parameters()

    def _reset_parameters(self) -> None:
        nn.init.normal_(self.embedding.weight, mean=0.0, std=0.02)
        with torch.no_grad():
            self.embedding.weight[self.pad_id].fill_(0)
        nn.init.normal_(self.position.weight, mean=0.0, std=0.02)
        nn.init.xavier_uniform_(self.output.weight)
        if self.output.bias is not None:
            nn.init.zeros_(self.output.bias)

    def _embed(self, ids: torch.Tensor) -> torch.Tensor:
        positions = torch.arange(ids.size(1), device=ids.device).clamp(max=self.max_length - 1).unsqueeze(0)
        return self.embedding(ids) * math.sqrt(self.d_model) + self.position(positions)

    @staticmethod
    def _causal_mask(length: int, device: torch.device) -> torch.Tensor:
        return torch.nn.Transformer.generate_square_subsequent_mask(length, device=device).bool()

    def forward(self, source_ids: torch.Tensor, target_ids: torch.Tensor) -> torch.Tensor:
        src_mask = source_ids.eq(self.pad_id)
        src = self._embed(source_ids)
        memory = self.encoder(src, src_key_padding_mask=src_mask)
        tgt_mask = self._causal_mask(target_ids.size(1), target_ids.device)
        tgt = self._embed(target_ids)
        decoded = self.decoder(
            tgt,
            memory,
            tgt_mask=tgt_mask,
            tgt_key_padding_mask=target_ids.eq(self.pad_id),
            memory_key_padding_mask=src_mask,
        )
        return self.output(decoded)

    @torch.no_grad()
    def generate(self, source_ids: torch.Tensor, max_length: int = 512, eos_id: int = 2) -> list[int]:
        src_mask = source_ids.eq(self.pad_id)
        memory = self.encoder(self._embed(source_ids), src_key_padding_mask=src_mask)
        generated = [self.bos_id]
        for _ in range(max_length):
            tgt = torch.tensor([generated], dtype=torch.long, device=source_ids.device)
            tgt_mask = self._causal_mask(tgt.size(1), tgt.device)
            decoded = self.decoder(
                self._embed(tgt),
                memory,
                tgt_mask=tgt_mask,
                memory_key_padding_mask=src_mask,
            )
            next_token = int(torch.argmax(self.output(decoded[:, -1, :]), dim=-1).item())
            generated.append(next_token)
            if next_token == eos_id:
                break
        return generated


class PointerTransformerSeq2Seq(TransformerSeq2Seq):
    """Transformer seq2seq with an explicit source-copy distribution.

    The decoder still produces vocabulary logits, but those are mixed with a
    distribution over source token ids. This lets the model copy literals and
    identifiers exactly instead of reconstructing them from BPE fragments.
    """

    def __init__(self, *args, copy_window: int = 0, **kwargs) -> None:
        super().__init__(*args, **kwargs)
        self.copy_window = int(copy_window)
        d_model = self.d_model
        self.copy_query = nn.Linear(d_model, d_model, bias=False)
        self.copy_key = nn.Linear(d_model, d_model, bias=False)
        self.copy_gate = nn.Linear(d_model, 1)
        nn.init.xavier_uniform_(self.copy_query.weight)
        nn.init.xavier_uniform_(self.copy_key.weight)
        nn.init.zeros_(self.copy_gate.weight)
        nn.init.zeros_(self.copy_gate.bias)

    def _encode(self, source_ids: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        src_mask = source_ids.eq(self.pad_id)
        memory = self.encoder(self._embed(source_ids), src_key_padding_mask=src_mask)
        return memory, src_mask

    def _decode_step(self, target_ids: torch.Tensor, memory: torch.Tensor,
                     src_mask: torch.Tensor) -> torch.Tensor:
        tgt_mask = self._causal_mask(target_ids.size(1), target_ids.device)
        return self.decoder(
            self._embed(target_ids),
            memory,
            tgt_mask=tgt_mask,
            tgt_key_padding_mask=target_ids.eq(self.pad_id),
            memory_key_padding_mask=src_mask,
        )

    def _copy_distribution(self, decoded: torch.Tensor, memory: torch.Tensor,
                           source_ids: torch.Tensor, src_mask: torch.Tensor
                           ) -> tuple[torch.Tensor, torch.Tensor]:
        batch, target_len, _ = decoded.shape
        source_len = source_ids.size(1)
        query = self.copy_query(decoded)
        key = self.copy_key(memory)
        scores = torch.matmul(query, key.transpose(-1, -2)) / math.sqrt(self.d_model)
        invalid = src_mask[:, None, :].expand(batch, target_len, source_len)
        # Never copy padding or BOS/EOS/UNK control tokens.
        invalid = invalid | source_ids[:, None, :].lt(4).expand(batch, target_len, source_len)
        if self.copy_window > 0:
            positions = torch.arange(source_len, device=decoded.device)
            window_start = max(0, source_len - self.copy_window)
            invalid = invalid | positions[None, None, :].lt(window_start)
        scores = scores.masked_fill(invalid, float("-inf"))
        attention = torch.softmax(scores, dim=-1)
        index = source_ids[:, None, :].expand(batch, target_len, source_len)
        copy = torch.zeros(batch, target_len, self.output.out_features,
                           device=decoded.device, dtype=decoded.dtype)
        copy.scatter_add_(-1, index, attention)
        return copy, scores

    def forward(self, source_ids: torch.Tensor, target_ids: torch.Tensor,
                return_aux: bool = False):
        memory, src_mask = self._encode(source_ids)
        decoded = self._decode_step(target_ids, memory, src_mask)
        vocab_logits = self.output(decoded)
        vocab_prob = torch.softmax(vocab_logits, dim=-1)
        copy_prob, copy_scores = self._copy_distribution(decoded, memory, source_ids, src_mask)
        gen_prob = torch.sigmoid(self.copy_gate(decoded))
        combined = gen_prob * vocab_prob + (1.0 - gen_prob) * copy_prob
        logits = torch.log(combined.clamp_min(1e-8))
        if return_aux:
            return logits, copy_scores, gen_prob.squeeze(-1)
        return logits

    @torch.no_grad()
    def generate(self, source_ids: torch.Tensor, max_length: int = 512, eos_id: int = 2) -> list[int]:
        memory, src_mask = self._encode(source_ids)
        generated = [self.bos_id]
        for _ in range(max_length):
            target = torch.tensor([generated], dtype=torch.long, device=source_ids.device)
            logits = self.forward(source_ids, target)
            next_token = int(torch.argmax(logits[:, -1, :], dim=-1).item())
            generated.append(next_token)
            if next_token == eos_id:
                break
        return generated

    @torch.no_grad()
    def generate_sample(self, source_ids: torch.Tensor, max_length: int = 512,
                        eos_id: int = 2, temperature: float = 0.8) -> list[int]:
        """Sample one translation from the local model's output distribution."""
        generated = [self.bos_id]
        for _ in range(max_length):
            target = torch.tensor([generated], dtype=torch.long, device=source_ids.device)
            logits = self.forward(source_ids, target)
            probs = torch.softmax(logits[:, -1, :] / max(temperature, 1e-4), dim=-1)
            next_token = int(torch.multinomial(probs, 1).item())
            generated.append(next_token)
            if next_token == eos_id:
                break
        return generated


class SmallSeq2Seq(nn.Module):
    def __init__(self, vocab_size: int, embedding_dim: int, hidden_dim: int, encoder_layers: int, decoder_layers: int, dropout: float = 0.1):
        super().__init__()
        self.embedding = nn.Embedding(vocab_size, embedding_dim, padding_idx=0)
        self.encoder = nn.GRU(embedding_dim, hidden_dim, num_layers=encoder_layers, batch_first=True, dropout=dropout if encoder_layers > 1 else 0.0)
        self.decoder = nn.GRU(embedding_dim, hidden_dim, num_layers=decoder_layers, batch_first=True, dropout=dropout if decoder_layers > 1 else 0.0)
        self.output = nn.Linear(hidden_dim, vocab_size)

    def forward(self, source_ids: torch.Tensor, target_ids: torch.Tensor) -> torch.Tensor:
        source_emb = self.embedding(source_ids)
        _, hidden = self.encoder(source_emb)
        target_emb = self.embedding(target_ids)
        decoded, _ = self.decoder(target_emb, hidden)
        return self.output(decoded)

    @torch.no_grad()
    def generate(self, source_ids: torch.Tensor, max_length: int = 512, eos_id: int = 2) -> list[int]:
        source_emb = self.embedding(source_ids)
        _, hidden = self.encoder(source_emb)
        generated = [1]  # <bos>
        for _ in range(max_length):
            last = torch.tensor([[generated[-1]]], device=source_ids.device)
            emb = self.embedding(last)
            decoded, hidden = self.decoder(emb, hidden)
            logits = self.output(decoded[:, -1, :])
            next_token = int(torch.argmax(logits, dim=-1).item())
            generated.append(next_token)
            if next_token == eos_id:
                break
        return generated