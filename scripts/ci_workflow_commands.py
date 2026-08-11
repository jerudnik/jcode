#!/usr/bin/env python3
"""Read the canonical command script for a job or recipe from `justfile`.

The old helper scraped GitHub workflow YAML so ci_local.sh could mimic CI. The
new source of truth is the repo `justfile`, so this module now resolves a job
name to a just recipe and returns the recipe body as a shell script.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
JUSTFILE = REPO_ROOT / "justfile"
JOB_TO_RECIPE = {
    "macos": "full-test",
    "linux-tests": "full-test",
}

RECIPE_RE = re.compile(r"^(?P<name>[A-Za-z0-9_-]+):(?!\=)(?P<rest>.*)$")


def resolve_recipe_name(selector: str) -> str:
    """Map a ci_local job name to the matching recipe name."""

    return JOB_TO_RECIPE.get(selector, selector)


def _recipe_body_lines(recipe: str, justfile: Path = JUSTFILE) -> list[str]:
    lines = justfile.read_text(encoding="utf-8").splitlines()
    in_recipe = False
    body: list[str] = []

    for line in lines:
        match = RECIPE_RE.match(line)
        if match and not line[:1].isspace():
            if in_recipe:
                break
            if match.group("name") == recipe:
                in_recipe = True
            continue

        if not in_recipe:
            continue

        if line.strip() == "":
            body.append("")
            continue
        if line[:1].isspace():
            body.append(line)
            continue
        break

    if not body:
        raise SystemExit(f"ci_workflow_commands: recipe {recipe!r} not found in {justfile}")

    non_empty = [len(re.match(r"^[ \t]*", line).group(0)) for line in body if line.strip()]
    trim = min(non_empty) if non_empty else 0
    return [line[trim:] if len(line) >= trim else "" for line in body]


def recipe_script(recipe: str, justfile: Path = JUSTFILE) -> str:
    return "\n".join(_recipe_body_lines(recipe, justfile)).rstrip()


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("selector", help="job name or recipe name, e.g. macos or full-test")
    ap.add_argument(
        "--justfile",
        default=str(JUSTFILE),
        help="path to the repository justfile",
    )
    args = ap.parse_args()

    justfile = Path(args.justfile)
    recipe = resolve_recipe_name(args.selector)
    try:
        script = recipe_script(recipe, justfile)
    except SystemExit as exc:
        print(str(exc), file=sys.stderr)
        return 1

    print(script)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

