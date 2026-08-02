#!/usr/bin/env python3
"""Self-tests for scripts/check_warning_budget.sh.

The gate this covers was vacuous for its entire life. It counted with
``rg -c '^warning:' || printf '0\\n'`` on runners that have no ripgrep, so
``rg: command not found`` was swallowed by the ``||`` and the gate printed
"Warning budget OK: current=0 baseline=0" having counted nothing. Observed in
CI run 30769863400 / job 91553341773 (Quality Guardrails), where the
"command not found" line sits directly above the "OK" line.

These tests drive the script with a stub ``cargo`` on PATH, so they assert what
the script does with output rather than what the current tree happens to
contain. A tree with zero warnings cannot distinguish a working counter from a
broken one, which is exactly how the defect survived.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "check_warning_budget.sh"
BASELINE = REPO_ROOT / "scripts" / "warning_budget.txt"


def run_with_stub_cargo(stub_body: str) -> subprocess.CompletedProcess[str]:
    """Run the real script with a stub `cargo` and a PATH that has no ripgrep.

    The PATH deliberately excludes ripgrep: that is the CI condition, and the
    script must not depend on a tool that may be absent.
    """
    with tempfile.TemporaryDirectory() as tmp:
        stub = Path(tmp) / "cargo"
        stub.write_text("#!/usr/bin/env bash\n" + textwrap.dedent(stub_body))
        stub.chmod(0o755)
        env = dict(os.environ)
        env["PATH"] = f"{tmp}:/usr/bin:/bin"
        return subprocess.run(
            ["bash", str(SCRIPT)],
            capture_output=True,
            text=True,
            env=env,
            cwd=str(REPO_ROOT),
        )


class WarningBudgetCounting(unittest.TestCase):
    def test_warnings_are_counted_without_ripgrep(self) -> None:
        """The regression: three warnings, no `rg` on PATH, must be caught.

        Pre-fix this printed "Warning budget OK: current=0 baseline=0" and
        exited 0.
        """
        result = run_with_stub_cargo(
            """
            echo "warning: unused variable: \\`x\\`"
            echo "warning: unused import: \\`Foo\\`"
            echo "   --> src/lib.rs:1:1"
            echo "warning: function is never used: \\`bar\\`"
            exit 0
            """
        )
        self.assertIn("current=3", result.stdout + result.stderr)
        self.assertNotEqual(result.returncode, 0, "3 warnings over a 0 baseline must fail")

    def test_a_clean_build_still_reports_zero(self) -> None:
        """`grep -c` exits 1 on no matches; that must read as 0, not as an error."""
        result = run_with_stub_cargo("exit 0\n")
        self.assertIn("current=0", result.stdout + result.stderr)
        self.assertEqual(result.returncode, 0)

    def test_a_failed_build_is_not_reported_as_a_clean_budget(self) -> None:
        """A compile failure emits no `warning:` lines.

        The old shape read that as zero warnings and passed. An unmeasurable
        count must fail rather than be reported as a good one.
        """
        result = run_with_stub_cargo(
            """
            echo "error[E0308]: mismatched types" >&2
            exit 101
            """
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not measurable", result.stdout + result.stderr)

    def test_only_leading_warning_lines_count(self) -> None:
        """Continuation lines mentioning "warning:" must not inflate the count."""
        result = run_with_stub_cargo(
            """
            echo "warning: unused variable: \\`x\\`"
            echo "  note: this warning: originates in a macro"
            exit 0
            """
        )
        self.assertIn("current=1", result.stdout + result.stderr)


class WarningBudgetToolAvailability(unittest.TestCase):
    def test_the_script_does_not_depend_on_ripgrep(self) -> None:
        """Naming the cause directly, so the defect cannot silently return.

        `rg` is not present on the GitHub runner image used by the workflows
        that invoke this script, and the script has no way to install it.
        """
        self.assertNotRegex(
            SCRIPT.read_text(),
            r"^\s*[^#\n]*\brg\s+-",
            "check_warning_budget.sh must not count with ripgrep",
        )

    def test_the_counting_tool_actually_exists(self) -> None:
        """Whatever it counts with must be resolvable, or the gate is vacuous."""
        self.assertIsNotNone(shutil.which("grep"), "grep must be present to count")


class WarningBudgetBaseline(unittest.TestCase):
    def test_baseline_is_a_plain_integer(self) -> None:
        self.assertRegex(BASELINE.read_text().strip(), r"^\d+$")


if __name__ == "__main__":
    unittest.main()
