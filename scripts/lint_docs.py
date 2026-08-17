#!/usr/bin/env python3
"""Run vale over the repository's markdown, and prove it actually read it.

`vale` invoked with no input files prints its usage banner and exits 0:

    $ git ls-files -z -- 'docs-renamed/*.md' | xargs -0 vale --config .vale.ini
    vale - A command-line linter for prose.
    ...
    $ echo $?
    0

So the previous recipe -- `git ls-files ... | xargs -0 vale` -- reported
success in exactly the case where nothing was checked. A renamed directory, a
typo in the pathspec, or a run from the wrong working directory would all have
read as a clean docs lint. That is the same shape as the linter that reported
zero findings because one invalid frontmatter line aborted its run: the absence
of output taken for the absence of problems.

Two things are asserted here that vale cannot assert about itself:

* the pathspec selected at least one file, and
* vale reported linting exactly as many files as were handed to it, so a
  silently skipped file is a failure rather than a smaller denominator.

`--vale` and `--files-from` exist so scripts/check_guard_nonvacuity.py can run
this against a stub that under-reports its file count and confirm the check
rejects it.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

PATHSPEC = ["*.md", ":!scripts/phone-server/**"]

ANSI = re.compile(r"\x1b\[[0-9;]*m")
# `✔ 0 errors, 0 warnings and 0 suggestions in 120 files.`
SUMMARY = re.compile(r"\bin (\d+) files?\.")


def tracked_markdown(root: Path) -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "-z", "--", *PATHSPEC],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout
    return [path for path in out.split("\0") if path]


def linted_count(output: str) -> int | None:
    """The file count vale reports, or None if it reported no summary at all."""

    matches = SUMMARY.findall(ANSI.sub("", output))
    return int(matches[-1]) if matches else None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    parser.add_argument("--vale", default="vale")
    parser.add_argument(
        "--files-from",
        type=Path,
        help="newline-delimited file list to lint instead of asking git",
    )
    args = parser.parse_args(argv)
    root = args.root.resolve()

    if args.files_from:
        files = [line for line in args.files_from.read_text().splitlines() if line]
    else:
        files = tracked_markdown(root)

    if not files:
        print(
            f"error: the pathspec {PATHSPEC} selected no files under {root}. "
            f"vale would print its usage banner and exit 0, so the lint would "
            f"have passed without reading anything.",
            file=sys.stderr,
        )
        return 1

    result = subprocess.run(
        [args.vale, "--config", str(root / ".vale.ini"), *files],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    print(result.stdout, end="")

    if result.returncode != 0:
        return result.returncode

    linted = linted_count(result.stdout)
    if linted is None:
        print(
            f"error: vale exited 0 without reporting how many files it linted, "
            f"so there is no evidence it read the {len(files)} file(s) it was "
            f"given.",
            file=sys.stderr,
        )
        return 1
    if linted != len(files):
        print(
            f"error: vale linted {linted} file(s) but was given {len(files)}. "
            f"The missing file(s) are unchecked, and the clean result covers "
            f"only the ones it read.",
            file=sys.stderr,
        )
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
