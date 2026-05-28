#!/usr/bin/env python3
"""Validate portable agent content metadata and layout.

This intentionally avoids external dependencies so it can run locally, in CI, and
inside future flake checks without host-specific setup.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parents[1]

NAME_RE = re.compile(r"^[a-z0-9][a-z0-9-]*$")
ALLOWED_EXTENSION_PREFIX = "x-"


@dataclass(frozen=True)
class ContentType:
    name: str
    roots: tuple[str, ...]
    file_name: str
    required_frontmatter: tuple[str, ...]
    required_heading_prefix: str


CONTENT_TYPES = (
    ContentType(
        name="skill",
        roots=(".jcode/skills", "skills"),
        file_name="SKILL.md",
        required_frontmatter=("name", "description", "allowed-tools"),
        required_heading_prefix="# ",
    ),
    ContentType(
        name="permission",
        roots=("permissions",),
        file_name="PERMISSION.md",
        required_frontmatter=("name", "description"),
        required_heading_prefix="# ",
    ),
    ContentType(
        name="prompt",
        roots=("prompts",),
        file_name="PROMPT.md",
        required_frontmatter=("name", "description"),
        required_heading_prefix="# ",
    ),
    ContentType(
        name="spec",
        roots=("specs",),
        file_name="SPEC.md",
        required_frontmatter=("name", "description"),
        required_heading_prefix="# ",
    ),
)


def parse_frontmatter(path: Path) -> tuple[dict[str, str], str, list[str]]:
    text = path.read_text(encoding="utf-8")
    errors: list[str] = []
    if not text.startswith("---\n"):
        return {}, text, ["missing YAML frontmatter block starting with '---'"]
    try:
        _, raw_frontmatter, body = text.split("---\n", 2)
    except ValueError:
        return {}, text, ["unterminated YAML frontmatter block"]

    frontmatter: dict[str, str] = {}
    for line_no, line in enumerate(raw_frontmatter.splitlines(), start=2):
        if not line.strip():
            continue
        if line.startswith((" ", "\t", "-")):
            errors.append(
                f"frontmatter line {line_no}: nested/list YAML is not supported by the portable validator; use scalar 'key: value' fields"
            )
            continue
        if ":" not in line:
            errors.append(f"frontmatter line {line_no}: expected 'key: value'")
            continue
        key, value = line.split(":", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        if not key:
            errors.append(f"frontmatter line {line_no}: empty key")
            continue
        frontmatter[key] = value
    return frontmatter, body, errors


def iter_content_files(content_type: ContentType) -> Iterable[Path]:
    for root_name in content_type.roots:
        root = REPO_ROOT / root_name
        if not root.exists():
            continue
        yield from sorted(root.glob(f"*/{content_type.file_name}"))


def validate_content_file(path: Path, content_type: ContentType) -> list[str]:
    rel = path.relative_to(REPO_ROOT)
    errors: list[str] = []
    frontmatter, body, parse_errors = parse_frontmatter(path)
    errors.extend(f"{rel}: {error}" for error in parse_errors)

    for key in content_type.required_frontmatter:
        if not frontmatter.get(key):
            errors.append(f"{rel}: missing required frontmatter field '{key}'")

    name = frontmatter.get("name", "")
    if name:
        if not NAME_RE.fullmatch(name):
            errors.append(f"{rel}: frontmatter name '{name}' must match {NAME_RE.pattern}")
        if path.parent.name != name:
            errors.append(f"{rel}: directory name '{path.parent.name}' must match frontmatter name '{name}'")

    for key in frontmatter:
        if key not in content_type.required_frontmatter and not key.startswith(ALLOWED_EXTENSION_PREFIX):
            errors.append(
                f"{rel}: unsupported frontmatter field '{key}'; use an '{ALLOWED_EXTENSION_PREFIX}...' vendor-neutral extension key if needed"
            )

    if content_type.required_heading_prefix not in body:
        errors.append(f"{rel}: missing Markdown heading starting with '{content_type.required_heading_prefix}'")

    for line_no, line in enumerate(body.splitlines(), start=1):
        if str(REPO_ROOT) in line:
            errors.append(f"{rel}: body line {line_no} contains host-specific repo path {REPO_ROOT}")
        if "/Users/" in line or "C:\\Users\\" in line:
            errors.append(f"{rel}: body line {line_no} appears to contain a host-specific user path")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate portable agent content files")
    parser.add_argument("--quiet", action="store_true", help="only print errors")
    args = parser.parse_args()

    errors: list[str] = []
    counts: dict[str, int] = {}
    for content_type in CONTENT_TYPES:
        files = list(iter_content_files(content_type))
        counts[content_type.name] = len(files)
        for path in files:
            errors.extend(validate_content_file(path, content_type))

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        print(
            "agent content validation failed; fix the frontmatter/layout issue above or document an x-* extension field",
            file=sys.stderr,
        )
        return 1

    if not args.quiet:
        summary = ", ".join(f"{name}={count}" for name, count in sorted(counts.items()))
        print(f"agent content validation passed ({summary})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
