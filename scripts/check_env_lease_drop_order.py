#!/usr/bin/env python3
"""Fail when a test fixture releases its environment lease before restoring env vars.

Tuple fields drop in **declaration order**. A fixture that returns its exclusion
lease before an `EnvVarGuard` therefore tears down in exactly the wrong order:

    (lease, temp, env_guard)   # lease released FIRST, env restored AFTER

Between those two drops the fixture no longer holds exclusion but is still about
to write the environment. Whichever test acquires the lease in that window has
its `JCODE_HOME` (or `HOME`) silently overwritten by the previous test's
teardown. The victim then resolves paths against the wrong home and fails an
assertion that names nothing relevant.

That is not hypothetical. It is how
`build_resume_command_uses_imported_jcode_session_for_codex` failed on Linux CI
after the F28 parallelism restoration: binary resolution fell through to
`current_exe()` and the assert saw `jcode_tui-<hash>` instead of `jcode`. The
same defect existed independently in `isolated_launcher_env`.

The rule is simple and mechanical: in a returned tuple, every lease must come
after every environment guard, so it is dropped last.

Status: WIRE. `scripts/preflight.sh` runs this guard through `just pre-pr`.

Exit status is non-zero on violation. Pure text scan, costs no compilation.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SCAN_ROOTS = (REPO_ROOT / "src", REPO_ROOT / "crates")

# Types that provide mutual exclusion between tests.
LEASE_TYPE = re.compile(
    r"\b(?:TestEnvWriteScope|TestEnvWriteLease|TestEnvReadLease|TestEnvScope|TestRenderScope|TestEnvFixtureLease)\b"
)
# Types whose Drop restores ambient environment variables.
ENV_GUARD_TYPE = re.compile(r"\bEnvVarGuard\b")

# `fn name(...) -> (A, B, C) {`
TUPLE_RETURNING_FN = re.compile(
    r"\bfn\s+(\w+)\s*(?:<[^>]*>)?\s*\([^)]*\)\s*->\s*\((?P<ret>[^{;]*?)\)\s*\{",
    re.S,
)


def split_tuple_elements(ret: str) -> list[str]:
    """Split a tuple return type on top-level commas.

    Generic parameters contain commas of their own (`Result<A, B>`), so a naive
    split would mis-associate positions and make the gate report nonsense.
    """
    parts, depth, current = [], 0, []
    for ch in ret:
        if ch in "<([":
            depth += 1
        elif ch in ">)]":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(current).strip())
            current = []
        else:
            current.append(ch)
    if "".join(current).strip():
        parts.append("".join(current).strip())
    return parts


def scan_source(path: Path, source: str) -> list[str]:
    violations = []
    for match in TUPLE_RETURNING_FN.finditer(source):
        ret = match.group("ret")
        if not (LEASE_TYPE.search(ret) and ENV_GUARD_TYPE.search(ret)):
            continue
        parts = split_tuple_elements(ret)
        lease_idx = [i for i, p in enumerate(parts) if LEASE_TYPE.search(p)]
        guard_idx = [i for i, p in enumerate(parts) if ENV_GUARD_TYPE.search(p)]
        if not lease_idx or not guard_idx:
            continue
        if min(lease_idx) < max(guard_idx):
            line = source[: match.start()].count("\n") + 1
            rel = path.relative_to(REPO_ROOT)
            violations.append(
                f"{rel}:{line}: fixture `{match.group(1)}` returns its lease at "
                f"position {min(lease_idx)} but an EnvVarGuard at position "
                f"{max(guard_idx)}. Tuple fields drop in declaration order, so "
                f"the lease is released while the environment is still being "
                f"restored. Move the lease to the end of the tuple."
            )
    return violations


def iter_sources():
    for root in SCAN_ROOTS:
        if not root.exists():
            continue
        for path in root.rglob("*.rs"):
            if "/target/" in str(path):
                continue
            yield path, path.read_text(encoding="utf-8", errors="replace")


def self_test() -> int:
    """Prove the gate reports the bad order and accepts the good one."""
    failures = []

    bad = "fn f() -> (TestEnvWriteScope, tempfile::TempDir, EnvVarGuard) {\n}"
    good = "fn f() -> (EnvVarGuard, tempfile::TempDir, TestEnvWriteScope) {\n}"
    unrelated = "fn f() -> (String, tempfile::TempDir) {\n}"
    # Only a lease, no env guard: nothing to order against.
    lease_only = "fn f() -> (TestEnvWriteScope, tempfile::TempDir) {\n}"
    # A generic with an internal comma must not shift positions.
    generic = "fn f() -> (EnvVarGuard, Result<A, B>, TestEnvWriteScope) {\n}"

    cases = [
        ("bad order is reported", bad, 1),
        ("good order is accepted", good, 0),
        ("unrelated fixtures ignored", unrelated, 0),
        ("lease without env guard ignored", lease_only, 0),
        ("generic commas do not confuse positions", generic, 0),
    ]
    for label, src, expected in cases:
        got = len(scan_source(REPO_ROOT / "x.rs", src))
        if got != expected:
            failures.append(f"{label}: expected {expected} violation(s), got {got}")

    parts = split_tuple_elements("EnvVarGuard, Result<A, B>, TestEnvWriteScope")
    if parts != ["EnvVarGuard", "Result<A, B>", "TestEnvWriteScope"]:
        failures.append(f"tuple split mishandles generics: {parts}")

    for f in failures:
        print(f"SELF-TEST FAIL: {f}")
    if failures:
        return 1
    print("self-test: all assertions passed")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()

    violations = []
    scanned = 0
    for path, source in iter_sources():
        scanned += 1
        violations.extend(scan_source(path, source))

    if violations:
        print("Test fixture drops its environment lease before restoring the environment:")
        for v in violations:
            print(f"  - {v}")
        return 1

    print(f"env lease drop order: ok ({scanned} files scanned)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
