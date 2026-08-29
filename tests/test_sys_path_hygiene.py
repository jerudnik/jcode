#!/usr/bin/env python3
"""The scripts/ directory is borrowed by test modules, never donated.

A test module that leaves `scripts/` on `sys.path` re-creates the shadowing
hazard the guards are hardened against (D036 lineage): every module imported
afterwards can resolve a bare `import x` to `scripts/x.py`. The borrow pattern
(append, import, remove — see tests/test_ci_workflow_commands.py) confines the
window to the module's own imports.

This test imports every scripts-importing test module in one process, in
sorted order, and asserts none of them leaves a scripts/ entry behind. It was
observed red against the tree before the borrow pattern was applied to the six
donating modules (each left `<repo>/scripts` at sys.path[0]), and green after.

The repository root can be overridden with JCODE_HYGIENE_ROOT so the test can
be pointed at another checkout for a control run.
"""

from __future__ import annotations

import importlib.util
import os
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(os.environ.get("JCODE_HYGIENE_ROOT", "")) if os.environ.get(
    "JCODE_HYGIENE_ROOT"
) else Path(__file__).resolve().parent.parent

SCRIPTS_IMPORTING_TEST_MODULES = (
    "test_ci_metrics.py",
    "test_ci_workflow_commands.py",
    "test_classify_pr_paths.py",
    "test_nix_distribution_policy.py",
    "test_reusable_workflow_calls.py",
    "test_workflow_permissions.py",
)


class SysPathHygieneTests(unittest.TestCase):
    def test_no_test_module_donates_scripts_to_sys_path(self) -> None:
        scripts_dir = str(REPO_ROOT / "scripts")
        self.assertNotIn(
            scripts_dir,
            sys.path,
            "precondition: scripts/ already on sys.path before any module loaded",
        )
        for name in SCRIPTS_IMPORTING_TEST_MODULES:
            path = REPO_ROOT / "tests" / name
            if not path.exists():
                continue
            spec = importlib.util.spec_from_file_location(
                f"hygiene_probe_{name.removesuffix('.py')}", path
            )
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            self.assertNotIn(
                scripts_dir,
                sys.path,
                f"{name} left {scripts_dir} on sys.path (donated, not borrowed)",
            )


if __name__ == "__main__":
    unittest.main()
