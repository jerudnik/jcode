#!/usr/bin/env python3
"""Reject legacy assignment-authority vocabulary outside its recorded allowlist."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEGACY = re.compile(r"capability(?:_| )?tier|CapabilityTier|SESSION_CAPABILITIES", re.IGNORECASE)

# These files define the classification rule and therefore must name the old
# vocabulary. Historical issue lines are pinned separately below.
RULE_FILES = {
    "docs/grant-rename-allowlist.md",
    "scripts/check_grant_vocabulary.py",
}

HISTORICAL_LINES = {
    "docs/issues/capability-tier-deferred-gaps.md": {
        'title: "Capability tier follow-ups deferred from the initial enforcement layer"',
        "# Capability tier deferred gaps",
    },
    "docs/issues/swarm-runaway-growth.md": {
        "- Capability tiers per node kind: explore/verify nodes get read-only tool",
        "budgets, capability tiers, or working-directory enforcement that failures 1",
        "decisions: capability tiers per node kind (read-only enforcement at the tool",
    },
}


def tracked_files() -> list[str]:
    output = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT)
    return [path for path in output.decode().split("\0") if path]


def matching_lines(path: Path) -> list[tuple[int, str]]:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    return [
        (line_number, line)
        for line_number, line in enumerate(text.splitlines(), 1)
        if LEGACY.search(line)
    ]


def main() -> int:
    violations: list[str] = []
    observed_historical: dict[str, set[str]] = {}

    for relative in tracked_files():
        matches = matching_lines(ROOT / relative)
        if not matches or relative in RULE_FILES:
            continue
        if relative in HISTORICAL_LINES:
            observed_historical[relative] = {line.strip() for _, line in matches}
            continue
        violations.extend(
            f"{relative}:{line_number}:{line.strip()}" for line_number, line in matches
        )

    for relative, expected in HISTORICAL_LINES.items():
        observed = observed_historical.get(relative, set())
        if observed != expected:
            missing = sorted(expected - observed)
            added = sorted(observed - expected)
            if missing:
                violations.append(f"{relative}: missing allowlisted historical lines: {missing}")
            if added:
                violations.append(f"{relative}: unlisted historical lines: {added}")

    if violations:
        print("grant_vocabulary_scope: FAIL", file=sys.stderr)
        for violation in violations:
            print(f"  {violation}", file=sys.stderr)
        print(
            "Rename assignment-authority vocabulary to grant or classify the hit in "
            "docs/grant-rename-allowlist.md.",
            file=sys.stderr,
        )
        return 1

    print("grant_vocabulary_scope: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
