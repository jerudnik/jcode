#!/usr/bin/env python3
"""Regression tests for the multi-model pipeline's parsing and routing.

These cover the parts that must not break silently. A verdict that fails to
parse degrades the gate to "no answer", and a mis-scoped allowlist silently
prevents the reviewer from running the tests at all -- both were observed for
real during bring-up, so both are pinned here.

Run: python3 scripts/pipeline_tests.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from pipeline import ROLES, _allow_for, _extract_json  # noqa: E402

FAILURES: list[str] = []


def check(name: str, got, want) -> None:
    if got != want:
        FAILURES.append(f"{name}\n     got: {got!r}\n    want: {want!r}")


def check_verdict(name: str, raw: str, want_verdict: str | None) -> None:
    """Assert what verdict (if any) is recovered from a reviewer's raw reply."""
    out = _extract_json(raw)
    if want_verdict is None:
        check(name, out, "")
        return
    try:
        got = json.loads(out).get("verdict")
    except (json.JSONDecodeError, AttributeError):
        got = f"<unparseable: {out[:60]}>"
    check(name, got, want_verdict)


def test_extract_json() -> None:
    check_verdict("plain object", '{"verdict":"pass"}', "pass")
    check_verdict("prose before", 'I reviewed it.\n{"verdict":"pass"}', "pass")
    check_verdict("markdown fence", '```json\n{"verdict":"fail"}\n```', "fail")
    check_verdict("prose after", '{"verdict":"pass"}\nHope that helps.', "pass")
    check_verdict("nested objects",
                  '{"verdict":"fail","meta":{"a":{"b":1}}}', "fail")

    # The reviewer echoes the schema it was given, which also contains the
    # literal key "verdict"; the real answer is the later object.
    check_verdict(
        "schema echoed before verdict",
        'Schema: {"type":"object","properties":{"verdict":{"type":"string"}}}\n'
        '{"verdict":"pass"}',
        "pass")

    # Observed live: a reviewer suggesting code wrote a stray brace after the
    # verdict, which desynchronised a naive backward brace scan.
    check_verdict("stray brace in trailing prose",
                  '{"verdict":"pass"}\nNote: use `}` to close the block.',
                  "pass")
    check_verdict("regex braces in suggestion",
                  '{"verdict":"fail","issues":[]}\n'
                  "Try re.sub(r'[^a-z]{1,3}', '-', s) instead.",
                  "fail")
    check_verdict("json in a fenced suggestion after the verdict",
                  '{"verdict":"pass"}\nExample config:\n'
                  '```json\n{"unrelated": true}\n```',
                  "pass")

    # Absence of a usable verdict must be reported, never guessed at.
    check_verdict("no json at all", "Looks fine to me.", None)
    check_verdict("truncated object", '{"verdict":"pass", "evidence":"bla',
                  None)
    check_verdict("object without a verdict key", '{"status":"done"}', None)
    check_verdict("empty input", "", None)


def test_allow_for() -> None:
    """The reviewer must be granted exactly the command it has to run."""
    check("python allowlist grants python3",
          "Bash(python3:*)" in _allow_for("python3 run_tests.py"), True)
    check("cargo allowlist grants cargo",
          "Bash(cargo:*)" in _allow_for("cargo test -p jcode-tui"), True)
    check("read tools always granted",
          {"Read", "Grep", "Glob"} <= set(_allow_for("cargo test")), True)

    # An absolute path must resolve to the binary name, or the grant misses.
    check("absolute path resolves to binary",
          "Bash(pytest:*)" in _allow_for("/usr/local/bin/pytest -q"), True)

    # No command should never produce a wildcard grant.
    check("empty command grants no bash",
          any(a.startswith("Bash(") and ":*)" in a and "Bash(:*)" in a
              for a in _allow_for("")), False)


def test_roles() -> None:
    """Routing must match the agreed division of labour."""
    check("author is codex", ROLES["author"].cli, "codex")
    check("author model", ROLES["author"].model, "gpt-5.6-sol")
    check("reviewer is claude", ROLES["reviewer"].cli, "claude")
    check("reviewer model", ROLES["reviewer"].model, "claude-opus-5")
    check("consultant is fable", ROLES["consultant"].model, "claude-fable-5")
    # Swapping roles is a supported move, so the inverted pair must exist.
    check("author-alt inverts to claude", ROLES["author-alt"].cli, "claude")
    check("reviewer-alt inverts to codex", ROLES["reviewer-alt"].cli, "codex")
    check("all roles use high effort",
          {r.effort for r in ROLES.values()}, {"high"})


def main() -> int:
    for test in (test_extract_json, test_allow_for, test_roles):
        test()
    if FAILURES:
        print(f"FAILED ({len(FAILURES)}):")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("OK: all pipeline tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
