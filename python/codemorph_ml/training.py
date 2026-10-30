"""Training configuration and backend selection.

Real neural-network training is intentionally not implemented in this generic
package. Backends report availability so callers cannot accidentally treat a
missing implementation as a trained model.
"""

from __future__ import annotations

import dataclasses
import json
from dataclasses import dataclass, field
from typing import Any, Dict, List


@dataclass
class TrainingConfig:
    architecture: str
    parameter_count: int
    tokenizer_version: str
    dataset_version: str
    context_length: int
    batch_size: int
    gradient_accumulation: int
    optimizer: str
    learning_rate_schedule: str
    precision: str
    seed: int
    checkpoint_interval: int
    hardware: str = "unknown"

    def to_json(self) -> str:
        return json.dumps(dataclasses.asdict(self), indent=2, sort_keys=True)


def _module_available(name: str) -> bool:
    import shutil
    import subprocess

    candidates = [os.getenv("CODEMORPH_PYTHON") if "os" in globals() else None, "python3.12", "python3"]
    for candidate in candidates:
        if not candidate:
            continue
        resolved = shutil.which(candidate) or candidate
        try:
            result = subprocess.run(
                [resolved, "-c", f"import {name}"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=20,
                check=False,
            )
            if result.returncode == 0:
                return True
        except Exception:
            continue
    return False


def backend_availability() -> Dict[str, bool]:
    import os

    return {"mlx": _module_available("mlx"), "torch": _module_available("torch")}


class TrainingBackendUnavailable(RuntimeError):
    pass


def require_backend(name: str) -> None:
    if not backend_availability().get(name, False):
        raise TrainingBackendUnavailable(
            f"training backend {name!r} is not installed; see docs/training.md"
        )


def build_run_manifest(config: TrainingConfig, git_commit: str, dataset_version: str) -> Dict[str, Any]:
    if dataset_version != config.dataset_version:
        raise ValueError("dataset version mismatch")
    return {
        "config": dataclasses.asdict(config),
        "git_commit": git_commit,
        "dataset_version": dataset_version,
        "backend_availability": backend_availability(),
    }