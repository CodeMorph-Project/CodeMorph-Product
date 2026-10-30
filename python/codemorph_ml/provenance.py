"""Provenance and license policy helpers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, List, Set


@dataclass
class LicensePolicy:
    allowed_licenses: Set[str]
    allow_unknown: bool = False

    def permits(self, license_name: str) -> bool:
        if license_name == "unknown":
            return self.allow_unknown
        return license_name in self.allowed_licenses


def check_mixture(licenses: Iterable[str], policy: LicensePolicy) -> List[str]:
    problems = []
    for lic in licenses:
        if not policy.permits(lic):
            problems.append(f"disallowed or unknown license: {lic}")
    return problems