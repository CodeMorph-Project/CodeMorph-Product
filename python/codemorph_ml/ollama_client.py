"""Ollama teacher backend for synthetic data generation."""

from __future__ import annotations

import hashlib
import json
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional


@dataclass
class OllamaConfig:
    host: str = "http://localhost:11434"
    model: str = "beam"
    timeout_seconds: int = 300
    temperature: float = 0.2
    top_p: float = 0.95
    num_predict: int = 2048
    repeat_penalty: float = 1.05

    def generation_options(self) -> Dict[str, Any]:
        return {
            "temperature": self.temperature,
            "top_p": self.top_p,
            "num_predict": self.num_predict,
            "repeat_penalty": self.repeat_penalty,
        }


@dataclass
class TeacherCandidate:
    text: str
    model: str
    latency_ms: int
    input_hash: str
    generation: Dict[str, Any]
    raw_response: Dict[str, Any] = field(default_factory=dict)


class OllamaTeacher:
    def __init__(self, config: Optional[OllamaConfig] = None):
        self.config = config or self.from_env()

    @staticmethod
    def from_env() -> OllamaConfig:
        import os

        return OllamaConfig(
            host=os.getenv("CODEMORPH_OLLAMA_HOST", "http://localhost:11434").rstrip("/"),
            model=os.getenv("CODEMORPH_TEACHER_MODEL", "beam"),
            timeout_seconds=int(os.getenv("CODEMORPH_OLLAMA_TIMEOUT", "300")),
        )

    def _request(self, endpoint: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        url = f"{self.config.host}{endpoint}"
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        started = time.monotonic()
        try:
            with urllib.request.urlopen(req, timeout=self.config.timeout_seconds) as response:
                raw = response.read().decode("utf-8")
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"Ollama HTTP {exc.code}: {body}") from exc
        latency = int((time.monotonic() - started) * 1000)
        parsed = json.loads(raw)
        parsed.setdefault("_latency_ms", latency)
        return parsed

    def health(self) -> Dict[str, Any]:
        # /api/tags uses GET.
        req = urllib.request.Request(f"{self.config.host}/api/tags", method="GET")
        with urllib.request.urlopen(req, timeout=self.config.timeout_seconds) as response:
            return json.loads(response.read().decode("utf-8"))

    def generate_candidates(self, prompt: str, candidate_count: int = 1) -> List[TeacherCandidate]:
        prompt_hash = hashlib.sha256(prompt.encode("utf-8")).hexdigest()
        candidates: List[TeacherCandidate] = []
        for index in range(max(1, candidate_count)):
            payload = {
                "model": self.config.model,
                "prompt": prompt,
                "stream": False,
                "options": self.config.generation_options(),
            }
            raw = self._request("/api/generate", payload)
            text = raw.get("response", "")
            candidates.append(
                TeacherCandidate(
                    text=text,
                    model=self.config.model,
                    latency_ms=int(raw.get("_latency_ms", 0)),
                    input_hash=prompt_hash,
                    generation={
                        "candidate_index": index,
                        "options": self.config.generation_options(),
                    },
                    raw_response=raw,
                )
            )
        return candidates