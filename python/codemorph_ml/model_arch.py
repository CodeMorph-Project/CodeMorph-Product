"""Small sequence-to-sequence model for CodeMorph experiments."""

from __future__ import annotations

import torch
import torch.nn as nn


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