#!/usr/bin/env python3
"""Measure documentation citations of code paths that no longer exist (D01-F12).

Not a gate. This is the measurement that sizes the eventual ratchet, kept as a
script so the number in the audit register can be re-derived instead of quoted
from a transcript.

Dated audit snapshots are reported separately rather than folded into the
total: they are point-in-time records of a tree that has since moved, so their
stale paths are accurate history, not drift.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

CITATION = re.compile(
    r"`((?:crates|src|scripts|tests)/[A-Za-z0-9_./-]+\.(?:rs|py|sh|nix))(?::(\d+))?`"
)

# Frozen or append-only trees: their contents are evidence, not current claims.
SKIP_PREFIXES = (
    "docs/fork/recovery/",
    "docs/fork/normalization/",
    "docs/archive/",
    "docs/fork/ideal-base/",
)

# Point-in-time audits. Counted, but reported apart from live drift.
SNAPSHOTS = (
    "docs/CODE_QUALITY_AUDIT_2026-04-18.md",
    "docs/PROVIDER_SESSION_SHARED_CONTRACT_AUDIT.md",
)


def tracked_files() -> set[str]:
    out = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    )
    return set(out.stdout.split())


def documents() -> list[Path]:
    found = list(ROOT.glob("docs/**/*.md")) + list(ROOT.glob("*.md"))
    live = []
    for path in found:
        rel = path.relative_to(ROOT).as_posix()
        if not any(rel.startswith(prefix) for prefix in SKIP_PREFIXES):
            live.append(path)
    return sorted(live)


def main() -> int:
    tracked = tracked_files()
    total = fragile = out_of_range = 0
    stale_live: dict[str, int] = {}
    stale_snapshot: dict[str, int] = {}

    for path in documents():
        rel = path.relative_to(ROOT).as_posix()
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for match in CITATION.finditer(text):
            cited, line = match.group(1), match.group(2)
            total += 1
            if cited not in tracked:
                bucket = stale_snapshot if rel in SNAPSHOTS else stale_live
                bucket[rel] = bucket.get(rel, 0) + 1
            elif line:
                length = len((ROOT / cited).read_text(encoding="utf-8").splitlines())
                if int(line) > length:
                    out_of_range += 1
                else:
                    fragile += 1

    live_total = sum(stale_live.values())
    snap_total = sum(stale_snapshot.values())

    print(f"cited code paths (live documents): {total}")
    print(f"  stale, dated audit snapshots:    {snap_total}  (frozen by nature)")
    print(f"  stale, live documentation:       {live_total}  across {len(stale_live)} files")
    print(f"  path:line out of range:          {out_of_range}")
    print(f"  path:line resolving today:       {fragile}  (fragile: correct until the file shifts)")

    if stale_live:
        print("\nstale citations by file:")
        for name, count in sorted(stale_live.items(), key=lambda kv: -kv[1]):
            print(f"  {count:4d}  {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
