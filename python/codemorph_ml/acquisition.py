"""Local-first source acquisition with license and provenance checks."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Dict, Iterable, List

from .provenance import LicensePolicy

SOURCE_EXTENSIONS = {
    ".c",
    ".cc",
    ".cpp",
    ".cxx",
    ".h",
    ".hh",
    ".hpp",
    ".java",
    ".kt",
    ".py",
    ".rs",
    ".ts",
    ".js",
    ".cu",
}


@dataclass
class SourceRecord:
    source: str
    repository: str
    commit: str
    file_path: str
    language: str
    license: str
    retrieval_date: str
    content_hash: str
    local_path: str


def detect_license(root: Path) -> str:
    candidates = [p for p in root.iterdir() if p.is_file() and p.name.lower().startswith("license")]
    if not candidates:
        return "unknown"
    text = candidates[0].read_text(encoding="utf-8", errors="ignore")[:4000].upper()
    if "MIT" in text:
        return "MIT"
    if "APACHE LICENSE" in text or "APACHE-2.0" in text:
        return "Apache-2.0"
    if "BSD 3-CLAUSE" in text or "BSD-3" in text:
        return "BSD-3-Clause"
    if "BSD 2-CLAUSE" in text or "BSD-2" in text:
        return "BSD-2-Clause"
    if "GPL-3" in text:
        return "GPL-3.0"
    if "GPL-2" in text:
        return "GPL-2.0"
    return "unknown"


def current_commit(root: Path) -> str:
    try:
        result = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=True,
            text=True,
        )
        return result.stdout.strip()
    except Exception:
        return "unknown"


def language_for(path: Path) -> str | None:
    mapping = {
        ".c": "c",
        ".h": "c",
        ".cc": "cpp",
        ".cpp": "cpp",
        ".cxx": "cpp",
        ".hh": "cpp",
        ".hpp": "cpp",
        ".java": "java",
        ".kt": "kotlin",
        ".py": "python",
        ".rs": "rust",
        ".js": "javascript",
        ".ts": "typescript",
        ".cu": "cuda",
    }
    return mapping.get(path.suffix.lower())


def acquire_local_source(
    root: Path,
    policy: LicensePolicy,
    repository_name: str | None = None,
    max_files: int = 1000,
) -> List[SourceRecord]:
    root = root.resolve()
    license_name = detect_license(root)
    if not policy.permits(license_name):
        raise ValueError(f"repository license not permitted: {license_name}")
    commit = current_commit(root)
    retrieval_date = "unknown"
    from datetime import date

    retrieval_date = date.today().isoformat()
    records: List[SourceRecord] = []
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        if any(part.startswith(".") for part in path.relative_to(root).parts):
            continue
        if path.suffix.lower() not in SOURCE_EXTENSIONS:
            continue
        language = language_for(path)
        if language is None:
            continue
        rel = path.relative_to(root).as_posix()
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        records.append(
            SourceRecord(
                source="local",
                repository=repository_name or root.name,
                commit=commit,
                file_path=rel,
                language=language,
                license=license_name,
                retrieval_date=retrieval_date,
                content_hash=digest,
                local_path=str(path),
            )
        )
        if len(records) >= max_files:
            break
    return records


def write_manifest(records: Iterable[SourceRecord], output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    payload = [asdict(r) for r in records]
    output.write_text(json.dumps(payload, indent=2, ensure_ascii=False), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description="Acquire source files with provenance")
    parser.add_argument("--root", required=True, help="Repository root")
    parser.add_argument("--output", required=True, help="Manifest JSON path")
    parser.add_argument("--allow", default="MIT,Apache-2.0,BSD-2-Clause,BSD-3-Clause", help="Comma-separated allowed licenses")
    parser.add_argument("--max-files", type=int, default=1000)
    args = parser.parse_args()
    allowed = {x.strip() for x in args.allow.split(",") if x.strip()}
    policy = LicensePolicy(allowed_licenses=allowed, allow_unknown=False)
    records = acquire_local_source(Path(args.root), policy, max_files=args.max_files)
    write_manifest(records, Path(args.output))
    print(f"acquired {len(records)} source files")


if __name__ == "__main__":
    main()