#!/usr/bin/env python3
"""Route a pull request's changed paths to the CI legs that can judge them.

Two routes come out of one classification:

``docs_only``
    Every changed path is prose. The heavy legs skip entirely; this is the
    pre-existing route and its meaning is unchanged.

``product_impacting``
    Some changed path can alter what the product does or how it is built, so
    the legs that exercise the built artifact must run: the ``full-test``
    recipe, the Nix package build, and the release smoke check.

The routes are independent: a change can be neither docs-only nor
product-impacting, which is exactly the case this classifier exists for. A
one-line edit to a governance workflow is judged by actionlint, the
reusable-call and permission checkers, the workflow contract tests, `cargo
check` and the test-graph compile -- none of which are skipped here. Rebuilding
the release binary and re-running the smoke check add no signal about that
edit, and they are the expensive legs.

Classification is allowlist-only and fails closed. A path is inert only if it
matches this file's table; anything unrecognised -- a new top-level directory,
a build script, a file whose name nobody anticipated -- is product-impacting,
and so is an empty change set.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from typing import Iterable, Sequence


# Workflow files that define or route the product legs themselves. Editing one
# of these changes what the expensive legs *do*, so they have to run to judge
# the edit. Every other workflow file is inert to them.
PRODUCT_ROUTE_WORKFLOWS = frozenset(
    {
        "ci.yml",
        "pr.yml",
        "fork-ci.yml",
        "nix.yml",
        "freebsd-smoke.yml",
    }
)

# Non-workflow paths that cannot affect a cargo or nix build.
INERT_FILES = frozenset({".vale.ini", "scripts/required-checks.json"})
INERT_PREFIXES = ("docs/", ".vale/")


def is_docs(path: str) -> bool:
    """True for prose, matching the route the classifier has always used."""
    return path.startswith("docs/") or path.endswith(".md")


def is_inert(path: str) -> bool:
    """True when nothing this path contains can change the built artifact."""
    if is_docs(path):
        return True
    if path in INERT_FILES or path.startswith(INERT_PREFIXES):
        return True
    if path.startswith(".github/"):
        # Only workflow YAML is understood. Composite actions, scripts, or
        # anything else added under .github/ later is treated as impacting
        # until someone teaches this table about it.
        prefix, _, name = path.rpartition("/")
        if prefix == ".github/workflows" and name.endswith((".yml", ".yaml")):
            return name not in PRODUCT_ROUTE_WORKFLOWS
        return False
    return False


def classify(paths: Iterable[str]) -> dict[str, bool]:
    changed = [path for path in paths if path]
    if not changed:
        # An empty change set means the diff could not be read, not that
        # nothing changed: run everything.
        return {"docs_only": False, "product_impacting": True}
    return {
        "docs_only": all(is_docs(path) for path in changed),
        "product_impacting": any(not is_inert(path) for path in changed),
    }


def changed_paths(base: str, head: str) -> list[str]:
    completed = subprocess.run(
        ["git", "diff", "--name-only", base, head],
        check=True,
        capture_output=True,
        text=True,
    )
    return [line.strip() for line in completed.stdout.splitlines() if line.strip()]


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="Base commit of the pull request.")
    parser.add_argument("--head", help="Head commit of the pull request.")
    parser.add_argument(
        "--paths-from",
        help="Read newline-separated paths from this file, or - for stdin.",
    )
    args = parser.parse_args(argv)

    if args.paths_from:
        stream = sys.stdin if args.paths_from == "-" else open(args.paths_from, encoding="utf-8")
        with stream:
            paths = [line.strip() for line in stream if line.strip()]
    elif args.base and args.head:
        paths = changed_paths(args.base, args.head)
    else:
        parser.error("pass --base and --head, or --paths-from")

    routes = classify(paths)
    for name, value in routes.items():
        print(f"{name}={str(value).lower()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
