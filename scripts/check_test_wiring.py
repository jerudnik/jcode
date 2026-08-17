#!/usr/bin/env python3
"""Every file in tests/ must be a unittest module that something actually runs.

Three ways a test file stops testing anything, all of which look like success:

1. Nothing invokes it. A file can sit in tests/ for months, pass when run by
   hand, and never run in CI. When this check was written, 14 of the 17 files
   in tests/ were in that state, including the tests for three guards that CI
   itself runs.
2. `unittest` collects zero tests from it, because the file is script-style
   (`def test_x()` plus a `__main__` block) rather than a `TestCase`. The run
   reports `Ran 0 tests ... OK` and the exit status is 0.
3. It needs a live instance or a network service, so it fails everywhere and
   gets quietly dropped from the wiring rather than fixed.

The rule enforced here is deliberately narrow and mechanical:

    every tests/test_*.py is a unittest module, collects at least one test,
    and its module name appears on an execution line in the justfile or in a
    workflow.

A probe that needs a running jcode does not meet that rule and belongs in
scripts/ (see scripts/probe_*.py). That is the point: the rule forces the
question when the file is added, instead of leaving it for whoever eventually
audits the directory.

`--root` runs the same checks against another tree, which is how
scripts/check_guard_nonvacuity.py plants a defect and proves this guard
rejects it.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# A reference only counts as wiring if it sits on a line that also runs
# something. A bare mention in a comment or a docs table is not wiring.
EXECUTION_MARKERS = ("unittest", "python3", "python ")

MODULE_NAME = re.compile(r"\btest_[a-z0-9_]+\b")

# `for file in tests/test_*.py` wires every module at once, and is the shape
# the justfile uses: nothing has to be remembered when a test file is added.
GLOB = re.compile(r"tests/test_\*\.py")


def wiring_sites(root: Path) -> list[Path]:
    sites = [root / "justfile"]
    sites += sorted((root / ".github" / "workflows").glob("*.yml"))
    return [path for path in sites if path.is_file()]


def executed_modules(root: Path, modules: list[str]) -> dict[str, list[str]]:
    """Map each test module name to the places that appear to execute it."""

    found: dict[str, list[str]] = {}
    for site in wiring_sites(root):
        for lineno, line in enumerate(site.read_text().splitlines(), 1):
            if not any(marker in line for marker in EXECUTION_MARKERS):
                continue
            where = f"{site.relative_to(root)}:{lineno}"
            names = modules if GLOB.search(line) else MODULE_NAME.findall(line)
            for name in names:
                found.setdefault(name, []).append(where)
    return found


def collected_count(root: Path, module: str) -> int:
    """How many test cases `unittest` collects from <root>/tests/<module>.py.

    Counted in a subprocess rooted at `root`. Importing the modules here would
    put them in this process's module cache under the same `tests.<name>` key
    regardless of which tree they came from, so a `--root` run would silently
    count the repository's own files.
    """

    probe = (
        "import unittest,sys;"
        f"print(unittest.defaultTestLoader.loadTestsFromName('tests.{module}')"
        ".countTestCases())"
    )
    result = subprocess.run(
        [sys.executable, "-c", probe],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip().splitlines()[-1] if result.stderr else "unknown error")
    return int(result.stdout.strip())


def problems(root: Path) -> list[str]:
    """Every reason the tests/ directory is not fully wired, in report order."""

    tests_dir = root / "tests"
    if not tests_dir.is_dir():
        return ["tests/ does not exist"]

    modules = sorted(path.stem for path in tests_dir.glob("test_*.py"))
    if not modules:
        return ["tests/ contains no test_*.py files"]

    executed = executed_modules(root, modules)
    found: list[str] = []

    for module in modules:
        if module not in executed:
            found.append(
                f"{module}: nothing runs it. Add it to the `check` recipe in the "
                f"justfile, or move it to scripts/ if it is a probe that needs a "
                f"live instance."
            )
            continue

        try:
            count = collected_count(root, module)
        except Exception as exc:  # noqa: BLE001 - report, never mask
            found.append(f"{module}: could not be loaded by unittest: {exc}")
            continue

        if count == 0:
            found.append(
                f"{module}: unittest collects 0 tests from it, so running it "
                f"reports success without checking anything. Give it a "
                f"unittest.TestCase; a `def test_*` with a `__main__` block is "
                f"not collected."
            )

    return found


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(argv)
    root = args.root.resolve()

    found = problems(root)
    if found:
        print("error: tests/ contains files that do not actually test:", file=sys.stderr)
        for problem in found:
            print(f"  - {problem}", file=sys.stderr)
        return 1

    modules = sorted(path.stem for path in (root / "tests").glob("test_*.py"))
    total = sum(collected_count(root, module) for module in modules)
    executed = executed_modules(root, modules)
    sites = len({site for module in modules for site in executed.get(module, ())})
    print(
        f"ok: {len(modules)} test module(s) in tests/, {total} test case(s), "
        f"all wired across {sites} execution site(s)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
