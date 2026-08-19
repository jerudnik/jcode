#!/usr/bin/env python3
"""Tests for classify_pr_paths: which CI legs a change set is entitled to skip.

A misclassification here is silent by construction -- the skipped leg reports
success -- so the cases below fix both directions: what must stay expensive,
and the narrow set that is allowed to become cheap.
"""

from __future__ import annotations

import io
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

# Borrowed, not donated: scripts/ on sys.path is the shadowing hazard the
# guards were hardened against, and it leaks into every module that runs after
# this one. Append so the standard library keeps precedence inside the window.
_SCRIPTS_DIR = str(Path(__file__).resolve().parent.parent / "scripts")
_BORROWED_PATH_ENTRY = _SCRIPTS_DIR not in sys.path
if _BORROWED_PATH_ENTRY:
    sys.path.append(_SCRIPTS_DIR)
try:
    import classify_pr_paths as classifier
finally:
    if _BORROWED_PATH_ENTRY:
        sys.path.remove(_SCRIPTS_DIR)


class ProductImpactTests(unittest.TestCase):
    def assert_impacting(self, *paths: str) -> None:
        self.assertTrue(
            classifier.classify(paths)["product_impacting"],
            f"{paths} must run the product legs",
        )

    def assert_inert(self, *paths: str) -> None:
        self.assertFalse(
            classifier.classify(paths)["product_impacting"],
            f"{paths} must not need the product legs",
        )

    def test_rust_sources_and_build_inputs_always_run_the_product_legs(self) -> None:
        for path in (
            "src/main.rs",
            "crates/jcode-tui/src/lib.rs",
            "Cargo.toml",
            "Cargo.lock",
            "build.rs",
            "flake.nix",
            "flake.lock",
            "rust-toolchain.toml",
            "justfile",
            "scripts/cargo_exec.sh",
        ):
            with self.subTest(path=path):
                self.assert_impacting(path)

    def test_a_governance_workflow_edit_does_not_need_the_product_legs(self) -> None:
        self.assert_inert(
            ".github/workflows/governance-root.yml",
            "scripts/required-checks.json",
        )

    def test_workflows_that_define_the_product_legs_do_need_them(self) -> None:
        for name in ("ci.yml", "pr.yml", "fork-ci.yml", "nix.yml", "freebsd-smoke.yml"):
            with self.subTest(workflow=name):
                self.assert_impacting(f".github/workflows/{name}")

    def test_one_impacting_path_outvotes_any_number_of_inert_ones(self) -> None:
        self.assert_impacting(
            "docs/guide.md",
            ".github/workflows/security.yml",
            "src/session.rs",
        )

    def test_unrecognised_paths_are_impacting(self) -> None:
        for path in (
            ".github/actions/setup/action.yml",
            ".github/workflows/nested/thing.yml",
            "tools/newthing/main.go",
            "Makefile",
            ".cargo/config.toml",
        ):
            with self.subTest(path=path):
                self.assert_impacting(path)

    def test_an_unreadable_change_set_runs_everything(self) -> None:
        self.assertEqual(
            classifier.classify([]),
            {"docs_only": False, "product_impacting": True},
        )

    def test_no_build_input_actually_lives_under_an_inert_prefix(self) -> None:
        # The table calls whole directories inert, which is only true while no
        # crate code lives in them. Checked against the repository rather than
        # asserted, so parking a crate under docs/ fails here instead of
        # silently exempting it from the product legs.
        root = Path(__file__).resolve().parent.parent
        for prefix in classifier.INERT_PREFIXES:
            directory = root / prefix
            if not directory.is_dir():
                continue
            found = [
                path.relative_to(root).as_posix()
                for path in directory.rglob("*")
                if path.suffix == ".rs" or path.name == "Cargo.toml"
            ]
            self.assertEqual(found, [], f"build inputs under inert prefix {prefix}")


class DocsOnlyTests(unittest.TestCase):
    def test_prose_only_change_sets_keep_the_docs_only_route(self) -> None:
        for paths in (
            ("README.md",),
            ("docs/architecture/GOVERNANCE_DECISIONS.md",),
            ("docs/issues/one.md", "CONTRIBUTING.md"),
            ("docs/images/diagram.png",),  # docs/ is prose by path, as it always was
        ):
            with self.subTest(paths=paths):
                self.assertTrue(classifier.classify(paths)["docs_only"])

    def test_any_non_prose_path_leaves_the_docs_only_route(self) -> None:
        self.assertFalse(classifier.classify(["README.md", "src/main.rs"])["docs_only"])
        self.assertFalse(
            classifier.classify([".github/workflows/governance-root.yml"])["docs_only"]
        )


class OutputTests(unittest.TestCase):
    def test_output_is_the_github_output_key_value_form(self) -> None:
        buffer = io.StringIO()
        with redirect_stdout(buffer):
            code = classifier.main(["--paths-from", "-"])
        self.assertEqual(code, 0)
        self.assertEqual(
            sorted(buffer.getvalue().splitlines()),
            ["docs_only=false", "product_impacting=true"],
        )

    def setUp(self) -> None:
        self._stdin = sys.stdin
        sys.stdin = io.StringIO("src/main.rs\n")
        self.addCleanup(setattr, sys, "stdin", self._stdin)


class RenameTests(unittest.TestCase):
    """A rename must be judged by both of its ends, not just where it landed.

    `git diff --name-only` reports a detected rename as the destination alone.
    A change set that moved a Rust source into `docs/` therefore read as prose:
    the deleted path was never shown to the classifier, `docs_only` came back
    true, and every leg that would have noticed the file leaving the build was
    skipped. The check reported success because nothing ran.

    These run real `git` against a scratch repository rather than a recorded
    fixture, because the defect was in what git reports, not in how this file
    handles what it is given. A fixture of the old output would have passed
    against the old code and proved nothing.
    """

    def setUp(self) -> None:
        self.repo = Path(tempfile.mkdtemp(prefix="classify-rename-"))
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        self.git("init", "-q", ".")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "test")

    def git(self, *args: str) -> str:
        return subprocess.run(
            ["git", *args],
            cwd=self.repo,
            check=True,
            capture_output=True,
            text=True,
        ).stdout

    def write(self, relative: str, text: str) -> None:
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def commit(self, message: str) -> str:
        self.git("add", "-A")
        self.git("commit", "-qm", message)
        return self.git("rev-parse", "HEAD").strip()

    def move(self, source: str, destination: str) -> None:
        # git mv will not create the destination directory for you.
        (self.repo / destination).parent.mkdir(parents=True, exist_ok=True)
        self.git("mv", source, destination)

    def changed(self, base: str, head: str) -> list[str]:
        cwd = os.getcwd()
        os.chdir(self.repo)
        try:
            return classifier.changed_paths(base, head)
        finally:
            os.chdir(cwd)

    def test_a_source_file_renamed_into_docs_still_runs_the_product_legs(self) -> None:
        self.write("crates/jcode/src/main.rs", "fn main() {}\n")
        base = self.commit("base")
        self.move("crates/jcode/src/main.rs", "docs/main.md")
        head = self.commit("move the source into docs")

        paths = self.changed(base, head)
        self.assertIn("crates/jcode/src/main.rs", paths)
        self.assertIn("docs/main.md", paths)
        route = classifier.classify(paths)
        self.assertFalse(route["docs_only"], "a deleted Rust source is not prose")
        self.assertTrue(route["product_impacting"])

    def test_a_prose_rename_keeps_the_docs_only_route(self) -> None:
        self.write("docs/a.md", "a\n")
        base = self.commit("base")
        self.move("docs/a.md", "docs/b.md")
        head = self.commit("rename prose")

        route = classifier.classify(self.changed(base, head))
        self.assertTrue(route["docs_only"], "prose moving within docs stays cheap")
        self.assertFalse(route["product_impacting"])

    def test_paths_containing_spaces_survive_the_parse(self) -> None:
        self.write("crates/a file.rs", "fn main() {}\n")
        base = self.commit("base")
        self.move("crates/a file.rs", "crates/b file.rs")
        head = self.commit("rename with spaces")

        paths = self.changed(base, head)
        self.assertIn("crates/a file.rs", paths)
        self.assertIn("crates/b file.rs", paths)

    def test_renames_are_read_alongside_ordinary_changes(self) -> None:
        self.write("crates/jcode/src/main.rs", "fn main() {}\n")
        self.write("docs/keep.md", "old\n")
        self.write("root.txt", "z\n")
        base = self.commit("base")
        self.move("crates/jcode/src/main.rs", "docs/main.md")
        self.write("docs/keep.md", "old\nnew\n")
        self.git("rm", "-q", "root.txt")
        head = self.commit("a mixed change set")

        self.assertEqual(
            sorted(self.changed(base, head)),
            [
                "crates/jcode/src/main.rs",
                "docs/keep.md",
                "docs/main.md",
                "root.txt",
            ],
        )


class RoutingInvocationTests(unittest.TestCase):
    """The routing invocation must be isolated, because it decides whether the
    check that would detect a shadow runs at all.

    Python puts a script's own directory first on sys.path, so a single added
    file scripts/<name>.py rebinds one of this module's imports. A rebound
    classifier can print docs_only=true, which skips Fork CI -- and Fork CI is
    where check_guard_nonvacuity.py, the guard that rejects exactly this class
    of shadow, runs. The guard is correct; it is simply sequenced after the
    step it protects, so the routing step cannot rely on it and has to be
    isolated at the point of invocation.

    -I drops the script directory from sys.path on 3.4+. PYTHONSAFEPATH and -P
    need 3.11 and silently no-op on older runners, so they are not equivalent.
    """

    def test_the_pr_workflow_invokes_the_classifier_isolated(self) -> None:
        workflow = (
            Path(__file__).resolve().parent.parent
            / ".github/workflows/pr.yml"
        ).read_text()
        # An *invocation* runs the script; a *mention* may just name its path,
        # as the routing-critical change-set scan does. Counting mentions made
        # this test fail on a workflow that had not gained a second invocation
        # at all, so it is narrowed to lines that actually execute python3 --
        # and correspondingly widened from "the first one is isolated" to
        # "every one is", which the mention-counting version never checked.
        invocations = [
            line.strip()
            for line in workflow.splitlines()
            if "classify_pr_paths.py" in line and "python3" in line
        ]
        self.assertEqual(len(invocations), 1, invocations)
        for invocation in invocations:
            self.assertTrue(
                invocation.startswith("python3 -I scripts/classify_pr_paths.py"),
                f"routing invocation is not isolated: {invocation!r}",
            )


if __name__ == "__main__":
    unittest.main()
