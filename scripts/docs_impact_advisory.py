#!/usr/bin/env python3
"""Build a lightweight DOX review packet for a Git comparison."""

from __future__ import annotations

import argparse
import fnmatch
import os
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

from check_agent_instructions import primitive


ROOT = Path(__file__).resolve().parent.parent
MAX_PATHS_PER_SCOPE = 20


@dataclass(frozen=True)
class ImpactGroup:
    pattern: str
    paths: tuple[str, ...]
    sources: tuple[str, ...]


def discover_scopes(root: Path) -> dict[str, tuple[str, ...]]:
    """Return APM applyTo patterns mapped to their tracked source primitives."""
    grouped: dict[str, list[str]] = defaultdict(list)
    primitive_dir = root / ".apm" / "instructions"
    for source in sorted(primitive_dir.glob("*.instructions.md")):
        relative = source.relative_to(root)
        pattern, _ = primitive(relative, root)
        grouped[pattern].append(relative.as_posix())

    if not grouped:
        raise ValueError("no APM instruction scopes found")
    if "**" not in grouped:
        raise ValueError('no root APM scope found; expected applyTo: "**"')
    return {pattern: tuple(paths) for pattern, paths in grouped.items()}


def changed_paths(root: Path, base: str, head: str) -> list[str]:
    """Return every path changed by base...head, including both sides of renames."""
    command = [
        "git",
        "-C",
        str(root),
        "diff",
        "--name-only",
        "-z",
        "--no-renames",
        f"{base}...{head}",
        "--",
    ]
    completed = subprocess.run(command, check=True, capture_output=True)
    return sorted(
        {
            raw.decode("utf-8", errors="surrogateescape")
            for raw in completed.stdout.split(b"\0")
            if raw
        }
    )


def matches(pattern: str, path: str) -> bool:
    """Match a standard glob without allowing a single star to cross `/`."""
    pattern_parts = tuple(pattern.split("/"))
    path_parts = tuple(path.split("/"))

    @lru_cache(maxsize=None)
    def walk(pattern_index: int, path_index: int) -> bool:
        if pattern_index == len(pattern_parts):
            return path_index == len(path_parts)
        part = pattern_parts[pattern_index]
        if part == "**":
            return walk(pattern_index + 1, path_index) or (
                path_index < len(path_parts) and walk(pattern_index, path_index + 1)
            )
        return (
            path_index < len(path_parts)
            and fnmatch.fnmatchcase(path_parts[path_index], part)
            and walk(pattern_index + 1, path_index + 1)
        )

    return walk(0, 0)


def specificity(pattern: str) -> tuple[int, int, int]:
    """Rank narrower patterns above broad wildcard patterns."""
    literal = re.sub(r"[*?\[\]]", "", pattern)
    literal_segments = sum(
        1 for segment in pattern.split("/") if segment and not re.search(r"[*?\[]", segment)
    )
    return literal_segments, len(literal), len(pattern)


def group_impacts(
    paths: list[str], scopes: dict[str, tuple[str, ...]]
) -> list[ImpactGroup]:
    grouped_paths: dict[str, list[str]] = defaultdict(list)
    grouped_sources: dict[str, set[str]] = defaultdict(set)

    for path in paths:
        matching = [pattern for pattern in scopes if matches(pattern, path)]
        owning_pattern = max(matching, key=specificity)
        grouped_paths[owning_pattern].append(path)
        for pattern in matching:
            grouped_sources[owning_pattern].update(scopes[pattern])

    return [
        ImpactGroup(
            pattern=pattern,
            paths=tuple(sorted(grouped_paths[pattern])),
            sources=tuple(sorted(grouped_sources[pattern])),
        )
        for pattern in sorted(grouped_paths, key=specificity)
    ]


def documentation_paths(paths: list[str]) -> list[str]:
    """Return changed paths that are documentation or instruction sources."""
    return [
        path
        for path in paths
        if path.endswith(".md")
        or path.startswith((".apm/", ".jcode/"))
        or path in {"apm.yml", "apm.lock.yaml"}
    ]


def short_revision(revision: str) -> str:
    return revision[:12] if len(revision) > 12 else revision


def render_markdown(base: str, head: str, paths: list[str], groups: list[ImpactGroup]) -> str:
    docs = documentation_paths(paths)
    lines = [
        "# DOX impact advisory",
        "",
        "> This check is advisory. It prepares a review packet but does not decide whether documentation must change.",
        "",
        f"**Comparison:** `{short_revision(base)}...{short_revision(head)}`  ",
        f"**Changed paths:** {len(paths)}  ",
        f"**Documentation or instruction paths changed:** {len(docs)}  ",
        "**Scope matching:** best-effort, segment-aware interpretation of tracked APM `applyTo` globs; APM compilation remains authoritative.  ",
        "",
    ]

    if not paths:
        lines.extend(["No changed paths were found for this comparison.", ""])
    else:
        lines.extend(["## Affected instruction scopes", ""])
        for group in groups:
            lines.extend(
                [
                    f"### `{group.pattern}`",
                    "",
                    "Applicable instruction sources:",
                    *[f"- `{source}`" for source in group.sources],
                    "",
                    f"Changed paths ({len(group.paths)}):",
                    *[
                        f"- `{path}`"
                        for path in group.paths[:MAX_PATHS_PER_SCOPE]
                    ],
                ]
            )
            remaining = len(group.paths) - MAX_PATHS_PER_SCOPE
            if remaining > 0:
                lines.append(f"- ... and {remaining} more")
            lines.append("")

    if docs:
        lines.extend(
            [
                "## Documentation and instruction changes in this diff",
                "",
                *[f"- `{path}`" for path in docs[:MAX_PATHS_PER_SCOPE]],
            ]
        )
        remaining = len(docs) - MAX_PATHS_PER_SCOPE
        if remaining > 0:
            lines.append(f"- ... and {remaining} more")
        lines.append("")

    lines.extend(
        [
            "## Review prompt",
            "",
            "Before merging, review the complete branch diff and decide whether it changes any durable contract:",
            "",
            "- purpose, scope, ownership, or responsibilities",
            "- user-visible behavior, inputs, outputs, permissions, constraints, or side effects",
            "- architecture, durable structure, workflows, operating rules, or produced artifacts",
            "- tool routing, validation, or verification requirements",
            "- durable user preferences or the instruction hierarchy itself",
            "",
            "If none changed, no documentation edit is required. Record that conclusion and a short reason in the PR description or review. If a contract changed, update the nearest authoritative documentation in this PR.",
            "",
        ]
    )
    return "\n".join(lines)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="base revision for a three-dot diff")
    parser.add_argument("--head", required=True, help="head revision for a three-dot diff")
    parser.add_argument(
        "--summary",
        type=Path,
        help="write Markdown to this file instead of standard output",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    scopes = discover_scopes(ROOT)
    paths = changed_paths(ROOT, args.base, args.head)
    groups = group_impacts(paths, scopes)
    packet = render_markdown(args.base, args.head, paths, groups)

    summary = args.summary
    if summary is None and os.environ.get("GITHUB_STEP_SUMMARY"):
        summary = Path(os.environ["GITHUB_STEP_SUMMARY"])
    if summary is None:
        print(packet)
    else:
        summary.parent.mkdir(parents=True, exist_ok=True)
        with summary.open("a", encoding="utf-8") as handle:
            handle.write(packet)

    if os.environ.get("GITHUB_ACTIONS") == "true":
        print(
            "::notice title=DOX review advisory::"
            f"Review {len(paths)} changed path(s) across {len(groups)} instruction scope(s)."
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
