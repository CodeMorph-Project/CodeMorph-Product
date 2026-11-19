#!/usr/bin/env python3
"""Explicitly opted-in external model backend.

This module is never selected automatically. It is used only when a model
artifact declares `backend: cloud-http` in its manifest and the caller sets
CODEMORPH_ALLOW_EXTERNAL=1. Translation remains neural; correctness is still
decided by the pipeline's compiler/behavior verifier.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.parse
import urllib.error
import urllib.request
from pathlib import Path

SYSTEM_PROMPT = (
    "You are a C++ to Rust translator. Output only standalone Rust source code. "
    "No explanations, no markdown fences. Use only the Rust standard library. "
    "The generated program must compile with rustc and preserve the C++ program's "
    "stdin/stdout/exit-code behavior."
)


def extract_code(text: str) -> str:
    text = text.strip()
    fences = re.findall(r"```(?:rust)?\s*(.*?)```", text, flags=re.S)
    if fences:
        return fences[-1].strip() + "\n"
    return text + "\n"


class RejectRedirects(urllib.request.HTTPRedirectHandler):
    """Keep source and credentials bound to the configured endpoint."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model-dir", required=True)
    ap.add_argument("--source-text", help="Read from stdin when omitted.")
    args = ap.parse_args()

    if os.environ.get("CODEMORPH_ALLOW_EXTERNAL") != "1":
        raise SystemExit("external model backend requires CODEMORPH_ALLOW_EXTERNAL=1")

    model_dir = Path(args.model_dir).resolve()
    config_path = model_dir / "cloud_config.json"
    if not config_path.exists():
        raise SystemExit(f"missing {config_path}")
    try:
        cfg = json.loads(config_path.read_text(encoding="utf-8"))
        url = cfg["chat_completions_url"]
        endpoint = urllib.parse.urlsplit(url)
        loopback = endpoint.hostname in {"localhost", "127.0.0.1", "::1"}
        if not endpoint.hostname or endpoint.username is not None or (
            endpoint.scheme != "https" and not (endpoint.scheme == "http" and loopback)
        ):
            raise ValueError("endpoint requires HTTPS or loopback HTTP without URL credentials")
        model = cfg["model"]
        if not isinstance(model, str) or not model.strip():
            raise ValueError("model must be a non-empty string")
        key_env = cfg.get("api_key_env", "")
        if not isinstance(key_env, str):
            raise ValueError("api_key_env must be a string")
        timeout = int(cfg.get("timeout_seconds", 120))
        if not 1 <= timeout <= 590:
            raise ValueError("timeout_seconds must be between 1 and 590")
        options = cfg.get("options", {})
        allowed = {"temperature", "max_tokens", "max_completion_tokens"}
        if not isinstance(options, dict) or options.keys() - allowed:
            raise ValueError("options must contain supported generation parameters")
        for name in allowed:
            if name in cfg:
                options.setdefault(name, cfg[name])
    except (OSError, ValueError, KeyError, TypeError) as exc:
        raise SystemExit(f"invalid cloud_config.json: {exc}") from None

    headers = {"Content-Type": "application/json"}
    if key_env:
        api_key = os.environ.get(key_env)
        if not api_key:
            raise SystemExit(f"missing API key environment variable {key_env}")
        headers["Authorization"] = f"Bearer {api_key}"

    source = args.source_text if args.source_text is not None else sys.stdin.read()
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": source},
        ],
        **options,
    }
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers=headers,
        method="POST",
    )
    try:
        opener = urllib.request.build_opener(RejectRedirects())
        with opener.open(request, timeout=timeout) as res:
            response = res.read(4 * 1024 * 1024 + 1)
        if len(response) > 4 * 1024 * 1024:
            raise SystemExit("external model response exceeded 4 MiB")
        body = json.loads(response)
    except urllib.error.HTTPError as exc:
        exc.close()
        raise SystemExit(f"external model HTTP {exc.code}") from None
    except (OSError, ValueError) as exc:
        raise SystemExit(f"external model request failed ({type(exc).__name__})") from None

    try:
        choice = body["choices"][0]
        text = choice["message"]["content"]
    except (KeyError, IndexError, TypeError):
        raise SystemExit("unexpected external model response") from None
    if choice.get("finish_reason") in {"length", "content_filter"}:
        raise SystemExit("external model response was incomplete")
    if not isinstance(text, str) or not text.strip():
        raise SystemExit("external model returned no source code")
    print(extract_code(text), end="")


if __name__ == "__main__":
    main()
