#!/usr/bin/env python3
"""Contract tests for the required local pre-PR gate."""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
WIRED_GUARDS = (
    "scripts/check_env_lease_drop_order.py",
    "scripts/check_tui_render_lock.py",
    "scripts/check_wildcard_reexport_budget.py",
    "scripts/check_config_env_lease.py",
)


class PrePrGateWiringTests(unittest.TestCase):
    def test_pre_pr_recipe_runs_preflight(self) -> None:
        justfile = (ROOT / "justfile").read_text(encoding="utf-8")
        self.assertIn("pre-pr:", justfile)
        self.assertIn("scripts/preflight.sh --no-branch-handoff", justfile)

    def test_preflight_runs_every_wired_guard(self) -> None:
        preflight = (ROOT / "scripts/preflight.sh").read_text(encoding="utf-8")
        for guard in WIRED_GUARDS:
            with self.subTest(guard=guard):
                self.assertIn(guard, preflight)

    def test_ci_rust_job_reaches_ambient_roots_through_preflight(self) -> None:
        workflow = (ROOT / ".github/workflows/pr.yml").read_text(
            encoding="utf-8"
        )
        preflight = (ROOT / "scripts/preflight.sh").read_text(encoding="utf-8")
        marker = "      - name: Run ratchet preflight\n"
        step = marker + workflow.split(marker, 1)[1].split("\n      - ", 1)[0]

        self.assertIn(
            "        run: scripts/preflight.sh --ratchets-only "
            "--no-branch-handoff\n",
            step,
        )
        self.assertNotIn("continue-on-error", step)
        self.assertNotIn(
            "\n    continue-on-error:", workflow.split("  rust:\n", 1)[1]
        )
        self.assertIn("scripts/check_ambient_roots.sh", preflight)


if __name__ == "__main__":
    unittest.main()
