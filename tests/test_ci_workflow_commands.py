#!/usr/bin/env python3
"""Tests for ci_workflow_commands: justfile-backed command scripts.

The helper no longer scrapes workflow YAML. It now resolves a job or recipe
name to the canonical shell script stored in the repo justfile, and ci_local.sh
uses that script directly.
"""

from __future__ import annotations

import unittest
import sys
from pathlib import Path

# Borrowed, not donated. Leaving scripts/ on sys.path is exactly the shadowing
# hazard the guards were hardened against, and it leaks into every module that
# runs after this one: `python3 -m unittest tests.test_guard_nonvacuity
# tests.test_ci_workflow_commands` fails that suite's sys.path scrub assertion
# purely because this import ran last. Append rather than insert so the standard
# library keeps precedence even inside the window.
_SCRIPTS_DIR = str(Path(__file__).resolve().parent.parent / "scripts")
_BORROWED_PATH_ENTRY = _SCRIPTS_DIR not in sys.path
if _BORROWED_PATH_ENTRY:
    sys.path.append(_SCRIPTS_DIR)
try:
    import ci_workflow_commands as cwc
finally:
    if _BORROWED_PATH_ENTRY:
        sys.path.remove(_SCRIPTS_DIR)


class JustfileRecipeTests(unittest.TestCase):
    def test_job_aliases_resolve_to_full_test(self) -> None:
        self.assertEqual(cwc.resolve_recipe_name("macos"), "full-test")
        self.assertEqual(cwc.resolve_recipe_name("linux-tests"), "full-test")

    def test_check_recipe_uses_the_workspace_check_command(self) -> None:
        script = cwc.recipe_script("check")
        self.assertIn(
            "scripts/cargo_exec.sh check --locked --workspace --all-targets --all-features",
            script,
        )

    def test_test_recipe_compiles_without_running(self) -> None:
        script = cwc.recipe_script("test")
        self.assertIn(
            "scripts/cargo_exec.sh test --locked --workspace --lib --bins --no-run",
            script,
        )

    def test_full_test_recipe_covers_release_and_suite_smoke(self) -> None:
        script = cwc.recipe_script("full-test")
        for expected in (
            "JCODE_CI_TARGET",
            "rustc -vV",
            "scripts/cargo_exec.sh build --locked --release --target \"$target\"",
            '"./target/$target/release/jcode" --version',
            "scripts/cargo_exec.sh test --locked --target \"$target\" --workspace --lib --bins --no-run",
            "scripts/cargo_exec.sh test --locked --target \"$target\" --workspace --lib --bins --exclude jcode-tui --exclude jcode-app-core",
            "scripts/cargo_exec.sh test --locked --target \"$target\" -p jcode-tui --lib",
            "scripts/cargo_exec.sh test --locked --target \"$target\" -p jcode-app-core --lib",
            "scripts/cargo_exec.sh test --locked --target \"$target\" --test provider_matrix --test e2e --no-run",
            "scripts/cargo_exec.sh test --locked --target \"$target\" --test provider_matrix",
            "JCODE_E2E_REQUIRE_BINARY=1",
            "JCODE_E2E_BINARY=\"$PWD/target/$target/release/jcode\"",
        ):
            self.assertIn(expected, script)

    def test_package_recipe_uses_cargo_package(self) -> None:
        script = cwc.recipe_script("package")
        self.assertIn(
            "scripts/cargo_exec.sh package --locked -p jcode --allow-dirty --no-verify --list",
            script,
        )

    def test_release_check_recipe_builds_and_launches(self) -> None:
        script = cwc.recipe_script("release-check")
        self.assertIn(
            "scripts/cargo_exec.sh build --locked --release --target \"$target\" --bin jcode",
            script,
        )
        self.assertIn('"./target/$target/release/jcode" --version', script)

    def test_lint_docs_recipe_uses_vale_and_repository_config(self) -> None:
        # The recipe used to pipe `git ls-files` straight into vale. vale with
        # no input files prints its usage banner and exits 0, so an empty
        # pathspec read as a clean lint. scripts/lint_docs.py owns the same
        # pathspec and config now, and refuses to report success unless vale
        # says it read every file it was handed.
        script = cwc.recipe_script("lint-docs")
        self.assertIn("python3 -I scripts/lint_docs.py", script)
        self.assertNotIn("xargs -0 vale", script)

        runner = Path(_SCRIPTS_DIR, "lint_docs.py").read_text(encoding="utf-8")
        self.assertIn('":!scripts/phone-server/**"', runner)
        self.assertIn('".vale.ini"', runner)

    def test_lint_docs_recipe_runs_the_docs_reference_checker(self) -> None:
        # The checker is fatal-by-design and had no caller until it was wired
        # here; lint-docs is the recipe CI already runs. `-I` keeps scripts/ off
        # sys.path so a sibling module cannot shadow the standard library.
        script = cwc.recipe_script("lint-docs")
        self.assertIn("python3 -I scripts/check_docs_references.py", script)

    def test_missing_recipe_fails_loudly(self) -> None:
        with self.assertRaises(SystemExit):
            cwc.recipe_script("no-such-recipe")

    def test_command_source_is_the_repo_justfile(self) -> None:
        self.assertEqual(Path(cwc.JUSTFILE).name, "justfile")


if __name__ == "__main__":
    unittest.main()
