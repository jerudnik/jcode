#!/usr/bin/env python3
"""Regression tests for the multi-model pipeline's parsing and routing.

WIRE verdict: ``just test-python`` collects this module through the
``tests/test_*.py`` glob.

These cover the parts that must not break silently. A verdict that fails to
parse degrades the gate to "no answer", and a mis-scoped allowlist silently
prevents the reviewer from running the tests at all -- both were observed for
real during bring-up, so both are pinned here.

Run: python3 -m unittest tests.test_pipeline
"""

from __future__ import annotations

import importlib.util
import json
import sys
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "pipeline.py"
SPEC = importlib.util.spec_from_file_location("pipeline", SCRIPT)
assert SPEC and SPEC.loader
pipeline = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = pipeline
SPEC.loader.exec_module(pipeline)


class PipelineTests(unittest.TestCase):
    def check_verdict(
        self, name: str, raw: str, want_verdict: str | None
    ) -> None:
        """Assert what verdict (if any) is recovered from a reviewer's raw reply."""
        with self.subTest(name=name):
            out = pipeline._extract_json(raw)
            if want_verdict is None:
                self.assertEqual(out, "")
                return
            try:
                got = json.loads(out).get("verdict")
            except (json.JSONDecodeError, AttributeError):
                got = f"<unparseable: {out[:60]}>"
            self.assertEqual(got, want_verdict)

    def test_extract_json(self) -> None:
        self.check_verdict("plain object", '{"verdict":"pass"}', "pass")
        self.check_verdict(
            "prose before", 'I reviewed it.\n{"verdict":"pass"}', "pass"
        )
        self.check_verdict(
            "markdown fence", '```json\n{"verdict":"fail"}\n```', "fail"
        )
        self.check_verdict(
            "prose after", '{"verdict":"pass"}\nHope that helps.', "pass"
        )
        self.check_verdict(
            "nested objects", '{"verdict":"fail","meta":{"a":{"b":1}}}', "fail"
        )

        # The reviewer echoes the schema it was given, which also contains the
        # literal key "verdict"; the real answer is the later object.
        self.check_verdict(
            "schema echoed before verdict",
            'Schema: {"type":"object","properties":{"verdict":{"type":"string"}}}\n'
            '{"verdict":"pass"}',
            "pass",
        )

        # Observed live: a reviewer suggesting code wrote a stray brace after
        # the verdict, which desynchronised a naive backward brace scan.
        self.check_verdict(
            "stray brace in trailing prose",
            '{"verdict":"pass"}\nNote: use `}` to close the block.',
            "pass",
        )
        self.check_verdict(
            "regex braces in suggestion",
            '{"verdict":"fail","issues":[]}\n'
            "Try re.sub(r'[^a-z]{1,3}', '-', s) instead.",
            "fail",
        )
        self.check_verdict(
            "json in a fenced suggestion after the verdict",
            '{"verdict":"pass"}\nExample config:\n'
            '```json\n{"unrelated": true}\n```',
            "pass",
        )

        # Absence of a usable verdict must be reported, never guessed at.
        self.check_verdict("no json at all", "Looks fine to me.", None)
        self.check_verdict(
            "truncated object", '{"verdict":"pass", "evidence":"bla', None
        )
        self.check_verdict(
            "object without a verdict key", '{"status":"done"}', None
        )
        self.check_verdict("empty input", "", None)

    def test_allow_for(self) -> None:
        """The reviewer must be granted exactly the command it has to run."""
        self.assertIn("Bash(python3:*)", pipeline._allow_for("python3 run_tests.py"))
        self.assertIn("Bash(cargo:*)", pipeline._allow_for("cargo test -p jcode-tui"))
        self.assertLessEqual(
            {"Read", "Grep", "Glob"}, set(pipeline._allow_for("cargo test"))
        )

        # An absolute path must resolve to the binary name, or the grant misses.
        self.assertIn(
            "Bash(pytest:*)", pipeline._allow_for("/usr/local/bin/pytest -q")
        )

        # No command should never produce a wildcard grant.
        self.assertFalse(
            any(
                allow.startswith("Bash(") and ":*)" in allow and "Bash(:*)" in allow
                for allow in pipeline._allow_for("")
            )
        )

    def test_roles(self) -> None:
        """Routing must match the agreed division of labour."""
        self.assertEqual(pipeline.ROLES["author"].cli, "codex")
        self.assertEqual(pipeline.ROLES["author"].model, "gpt-5.6-sol")
        self.assertEqual(pipeline.ROLES["reviewer"].cli, "claude")
        self.assertEqual(pipeline.ROLES["reviewer"].model, "claude-opus-5")
        self.assertEqual(pipeline.ROLES["consultant"].model, "claude-fable-5")
        self.assertEqual(pipeline.ROLES["author-alt"].cli, "claude")
        self.assertEqual(pipeline.ROLES["reviewer-alt"].cli, "codex")
        self.assertEqual({role.effort for role in pipeline.ROLES.values()}, {"high"})


if __name__ == "__main__":
    unittest.main()
