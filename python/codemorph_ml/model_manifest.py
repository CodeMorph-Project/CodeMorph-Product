"""Python mirror of the Rust model manifest schema."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Dict, List


@dataclass
class ModelManifest:
    model_id: str
    version: str
    role: str
    source_language: str
    target_language: str
    supported_features: List[str]
    architecture: str
    parameter_count: int
    tokenizer: str
    context_length: int
    backend: str
    artifact_format: str
    protocol_version: str
    hashes: Dict[str, str] = field(default_factory=dict)

    @classmethod
    def from_json(cls, text: str) -> "ModelManifest":
        payload = json.loads(text)
        return cls(**payload)

    def to_json(self) -> str:
        import dataclasses

        return json.dumps(dataclasses.asdict(self), indent=2, sort_keys=True)