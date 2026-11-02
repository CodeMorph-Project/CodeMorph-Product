"""Versioned dataset records and validation."""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass, field
from typing import Any, Dict, List, Optional

DATASET_SCHEMA_VERSION = "0.1"


@dataclass
class Provenance:
    source: str
    repository: str
    commit: str
    file_path: str
    language: str
    license: str
    retrieval_date: str
    content_hash: str

    def validate(self) -> None:
        required = ["source", "repository", "commit", "file_path", "language", "license", "retrieval_date", "content_hash"]
        for name in required:
            value = getattr(self, name)
            if not isinstance(value, str) or not value:
                raise ValueError(f"provenance field {name} must be non-empty string")


@dataclass
class VerificationRecord:
    cpp_compiles: bool
    rust_compiles: bool
    tests_passed: bool
    differential_passed: bool
    fuzz_iterations: int
    sanitizer_status: str
    static_analysis_status: str
    benchmark_ratio: Optional[float]
    rust_quality: float
    confidence: str
    rejection_reasons: List[str] = field(default_factory=list)


@dataclass
class DatasetExample:
    schema_version: str
    id: str
    source_language: str
    target_language: str
    source_cpp: str
    structured_context: Dict[str, Any]
    rust_target: str
    verification: VerificationRecord
    feature_tags: List[str]
    difficulty: str
    provenance: Provenance
    teacher: Dict[str, Any]
    metadata: Dict[str, Any] = field(default_factory=dict)
    negative_candidates: List[str] = field(default_factory=list)

    def content_hash(self) -> str:
        payload = self.source_cpp + "\n" + self.rust_target
        return hashlib.sha256(payload.encode("utf-8")).hexdigest()

    def validate(self) -> None:
        if self.schema_version != DATASET_SCHEMA_VERSION:
            raise ValueError("unsupported schema version")
        if not self.id:
            raise ValueError("id is required")
        if self.source_language == self.target_language:
            raise ValueError("source and target language must differ")
        self.provenance.validate()
        if self.verification.rust_quality < 0.0 or self.verification.rust_quality > 1.0:
            raise ValueError("rust_quality must be in [0,1]")

    def to_json(self) -> str:
        return json.dumps(asdict(self), indent=2, ensure_ascii=True)


def load_example(payload: Dict[str, Any]) -> DatasetExample:
    return DatasetExample(
        schema_version=payload["schema_version"],
        id=payload["id"],
        source_language=payload["source_language"],
        target_language=payload["target_language"],
        source_cpp=payload["source_cpp"],
        structured_context=payload.get("structured_context", {}),
        rust_target=payload["rust_target"],
        verification=VerificationRecord(**payload["verification"]),
        feature_tags=payload.get("feature_tags", []),
        difficulty=payload.get("difficulty", "unknown"),
        provenance=Provenance(**payload["provenance"]),
        teacher=payload.get("teacher", {}),
        metadata=payload.get("metadata", {}),
        negative_candidates=payload.get("negative_candidates", []),
    )