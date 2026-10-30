"""Synthetic C++ to Rust data factory using an Ollama teacher."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Dict, List

from .dataset import DATASET_SCHEMA_VERSION, DatasetExample, Provenance, VerificationRecord
from .ollama_client import OllamaTeacher

CURRICULUM = [
    "arithmetic",
    "loops",
    "conditionals",
    "arrays",
    "strings",
    "structs",
    "RAII",
    "references",
    "pointers",
    "STL vector",
]


@dataclass
class GenerationStats:
    total_cpp: int = 0
    teacher_calls: int = 0
    compiled_rust: int = 0
    accepted: int = 0
    rejected: int = 0


def prompt_for_translation(source_cpp: str, feature_tags: List[str]) -> str:
    return (
        "You are translating C++ to idiomatic but behavior-preserving Rust. "
        "Return only Rust code and no explanation. "
        "Preserve function signatures and observable behavior. "
        "Feature tags: "
        + ", ".join(feature_tags)
        + "\n\nC++ source:\n```cpp\n"
        + source_cpp
        + "\n```\n"
    )


def compile_rust_with_rustc(source: str) -> bool:
    with tempfile.TemporaryDirectory() as tmp:
        src = Path(tmp) / "candidate.rs"
        out = Path(tmp) / "candidate.out"
        src.write_text(source, encoding="utf-8")
        try:
            result = subprocess.run(
                ["rustc", "--crate-type", "lib", "-O", str(src), "-o", str(out)],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=120,
                check=False,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired):
            return False
        return result.returncode == 0 and out.exists()




def clean_rust_candidate(text: str) -> str:
    """Remove markdown fences and surrounding explanatory text from a candidate."""
    lines = text.strip().splitlines()
    if lines and lines[0].strip().startswith("```"):
        lines = lines[1:]
    while lines and lines[-1].strip().startswith("```"):
        lines = lines[:-1]
    cleaned = "\n".join(lines).strip()
    # Remove a common leading language identifier if it survived parsing.
    if cleaned.startswith("rust\n"):
        cleaned = cleaned[len("rust\n"):]
    return cleaned


def run_factory(cpp_samples: List[Dict[str, Any]], output: Path, candidates_per_sample: int = 1) -> GenerationStats:
    stats = GenerationStats(total_cpp=len(cpp_samples))
    output.mkdir(parents=True, exist_ok=True)
    generated_dir = output / "generated"
    rejected_dir = output / "rejected"
    generated_dir.mkdir(parents=True, exist_ok=True)
    rejected_dir.mkdir(parents=True, exist_ok=True)
    teacher = OllamaTeacher()

    for sample in cpp_samples:
        features = sample.get("feature_tags", [])
        prompt = prompt_for_translation(sample["source_cpp"], features)
        stats.teacher_calls += 1
        teacher_candidates = teacher.generate_candidates(prompt, candidate_count=candidates_per_sample)
        for cand in teacher_candidates:
            source_rust = clean_rust_candidate(cand.text)
            compiles = compile_rust_with_rustc(source_rust)
            if compiles:
                stats.compiled_rust += 1
            candidate_hash = hashlib.sha256(source_rust.encode("utf-8")).hexdigest()
            base = {
                "id": sample["id"] + "-" + candidate_hash[:12],
                "source_language": "cpp",
                "target_language": "rust",
                "source_cpp": sample["source_cpp"],
                "structured_context": sample.get("structured_context", {}),
                "rust_target": source_rust,
                "feature_tags": features,
                "difficulty": sample.get("difficulty", "unknown"),
                "teacher": {
                    "backend": "ollama",
                    "model": cand.model,
                    "latency_ms": cand.latency_ms,
                    "input_hash": cand.input_hash,
                },
                "provenance": {
                    "source": sample["provenance"]["source"],
                    "repository": sample["provenance"]["repository"],
                    "commit": sample["provenance"]["commit"],
                    "file_path": sample["provenance"]["file_path"],
                    "language": "cpp",
                    "license": sample["provenance"]["license"],
                    "retrieval_date": sample["provenance"]["retrieval_date"],
                    "content_hash": sample["provenance"]["content_hash"],
                },
            }
            if compiles:
                record = DatasetExample(
                    schema_version=DATASET_SCHEMA_VERSION,
                    id=base["id"],
                    source_language="cpp",
                    target_language="rust",
                    source_cpp=base["source_cpp"],
                    structured_context=base["structured_context"],
                    rust_target=base["rust_target"],
                    verification=VerificationRecord(
                        cpp_compiles=True,
                        rust_compiles=True,
                        tests_passed=False,
                        differential_passed=False,
                        fuzz_iterations=0,
                        sanitizer_status="unknown",
                        static_analysis_status="unknown",
                        benchmark_ratio=None,
                        rust_quality=0.0,
                        confidence="low",
                    ),
                    feature_tags=features,
                    difficulty=base["difficulty"],
                    provenance=sample_provenance(sample),
                    teacher=base["teacher"],
                )
                record.validate()
                (generated_dir / (record.id + ".json")).write_text(record.to_json(), encoding="utf-8")
                stats.accepted += 1
            else:
                (rejected_dir / (base["id"] + ".json")).write_text(
                    json.dumps(base, indent=2, ensure_ascii=False), encoding="utf-8"
                )
                stats.rejected += 1
    return stats


def sample_provenance(sample: Dict[str, Any]):
    from .dataset import Provenance

    return Provenance(**sample["provenance"])


def load_samples(path: Path) -> List[Dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, list):
        return payload
    if isinstance(payload, dict) and "samples" in payload:
        return payload["samples"]
    raise ValueError("samples JSON must be a list or object with samples")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate synthetic C++ to Rust data with Ollama")
    parser.add_argument("--samples", required=True, help="JSON file containing C++ samples")
    parser.add_argument("--output", required=True, help="Output directory")
    parser.add_argument("--candidates", type=int, default=1, help="Candidates per sample")
    args = parser.parse_args()

    samples = load_samples(Path(args.samples))
    stats = run_factory(samples, Path(args.output), candidates_per_sample=args.candidates)
    print(
        f"cpp_samples={stats.total_cpp} teacher_calls={stats.teacher_calls} "
        f"rust_compiled={stats.compiled_rust} accepted={stats.accepted} "
        f"rejected={stats.rejected}"
    )


if __name__ == "__main__":
    main()
