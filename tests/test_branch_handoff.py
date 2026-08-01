#!/usr/bin/env python3
"""Executable guard for the branch-handoff check.

The check exists because work stalls silently on its way to `main`, so the one
thing it must never do is report an all-clear it did not earn. Every test here
plants a failure mode in a synthetic repository and observes the gate going
red; a gate that has only ever been seen green proves nothing.

The synthetic repositories are real git repositories with a real bare remote,
not mocks. The bug that motivated the `usable_repo` guard (see
`test_uninspectable_checkout_is_an_error`) surfaced precisely because a
hand-run trial placed the script where `REPO_ROOT` resolved to a
non-repository, and it printed a confident "no unmerged automation/** branches"
and exited 0. Mocking git would have hidden that.

`gh` is removed from PATH in every synthetic case, so the tests never depend on
network access or GitHub auth. That exercises the degraded path deliberately:
the unpushed shape must still be caught, and PR-state stalls must be disclosed
as unchecked rather than silently treated as clean.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CHECKER = REPO_ROOT / "scripts" / "check_branch_handoff.py"

# A PATH with no `gh`, so every synthetic case runs the degraded path.
NO_GH_ENV = {"PATH": "/usr/bin:/bin", "HOME": "/tmp"}


def git(cwd: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True)


def commit(cwd: Path, name: str) -> None:
    (cwd / name).write_text(name, encoding="utf-8")
    git(cwd, "add", "-A")
    git(cwd, "commit", "-qm", name)


class BranchHandoffFixture:
    """A throwaway repository with a bare remote and the checker installed.

    The checker is copied to `<repo>/scripts/`, mirroring its real location,
    because it derives the repository root from its own path.
    """

    def __init__(self, tmp: Path) -> None:
        self.root = tmp / "repo"
        self.root.mkdir()
        self.remote = tmp / "remote.git"
        git(tmp, "init", "-q", "--bare", str(self.remote))
        git(self.root, "init", "-q", "-b", "main")
        git(self.root, "config", "user.email", "gate@example.invalid")
        git(self.root, "config", "user.name", "gate")
        git(self.root, "remote", "add", "github", str(self.remote))
        scripts = self.root / "scripts"
        scripts.mkdir()
        (scripts / CHECKER.name).write_text(CHECKER.read_text(encoding="utf-8"), encoding="utf-8")
        self.checker = scripts / CHECKER.name
        # The checker is committed on `main`, before any branch exists, so it
        # is present on every branch. Leaving it untracked does not work: the
        # `git add -A` in `branch()` sweeps it into that branch's commit, and
        # checking `main` back out then deletes it as a file `main` never had.
        # That produced a confusing "No such file or directory" from the
        # subprocess, which looked like a temp-directory lifetime problem and
        # is not one.
        commit(self.root, "seed")

    def branch(self, name: str, *, push: bool = False, merge: bool = False) -> None:
        git(self.root, "checkout", "-q", "-b", name, "main")
        commit(self.root, name.replace("/", "-"))
        if push:
            git(self.root, "push", "-q", "github", name)
        git(self.root, "checkout", "-q", "main")
        if merge:
            git(self.root, "merge", "-q", "--no-ff", name, "-m", f"merge {name}")

    def run(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(self.checker), *args],
            cwd=self.root,
            capture_output=True,
            text=True,
            check=False,
            env=NO_GH_ENV,
        )


class BranchHandoffTest(unittest.TestCase):
    def fixture(self) -> BranchHandoffFixture:
        """A fresh repository whose lifetime is tied to this test.

        `addCleanup` rather than a `with` block: the tree must outlive every
        subprocess the test launches. An earlier version used an ExitStack that
        closed at the end of the block and deleted the directory while the
        checker was still being invoked, which failed seven tests with a
        misleading "No such file or directory".
        """
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        return BranchHandoffFixture(Path(tmp.name))

    def test_unpushed_branch_is_reported(self):
        """The original failure: finished work that never left the laptop."""
        fix = self.fixture()
        fix.branch("automation/stranded")
        result = fix.run()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("automation/stranded", result.stdout)
        self.assertIn("not on github", result.stdout)
        # The remedy must name the discovered remote, not a guessed one.
        self.assertIn("git push -u github automation/stranded", result.stdout)

    def test_merged_branch_is_ignored(self):
        """A branch already in main is done, not stalled."""
        fix = self.fixture()
        fix.branch("automation/landed", merge=True)
        result = fix.run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("automation/landed", result.stdout)

    def test_pushed_branch_without_gh_is_not_a_false_positive(self):
        """Without `gh`, a pushed branch cannot be judged, so it is not flagged.

        Flagging it would fail every closeout on an unauthenticated machine,
        which trains people to ignore the gate.
        """
        fix = self.fixture()
        fix.branch("automation/pushed", push=True)
        result = fix.run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("all on a path to main", result.stdout)

    def test_gh_absence_is_disclosed_when_something_is_wrong(self):
        """A partial verdict must say what it could not check."""
        fix = self.fixture()
        fix.branch("automation/stranded")
        fix.branch("automation/pushed", push=True)
        result = fix.run()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("gh unavailable", result.stdout)

    def test_uninspectable_checkout_is_an_error(self):
        """Looking in the wrong place must not read as an all-clear.

        Regression test for a real bug: with the script outside a repository,
        every git query failed, the branch list came back empty, and the gate
        printed "no unmerged automation/** branches" and exited 0, which is the
        same false assurance it exists to prevent.
        """
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        loose = Path(tmp.name) / "check_branch_handoff.py"
        loose.write_text(CHECKER.read_text(encoding="utf-8"), encoding="utf-8")
        result = subprocess.run(
            [sys.executable, str(loose)],
            cwd=tmp.name,
            capture_output=True,
            text=True,
            check=False,
            env=NO_GH_ENV,
        )
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("cannot inspect", result.stderr)

    def test_remote_name_is_discovered_not_assumed(self):
        """CI names its remote `origin`; the canonical clone names it `github`.

        A hardcoded `origin` is a bug this repository already paid for once, in
        `ideal_base_railway.py` (commit 5ee7149b7), where a fixed
        `refs/remotes/origin/main` broke every bare invocation on the canonical
        checkout.
        """
        for remote_name in ("origin", "github", "fork-mirror"):
            with self.subTest(remote=remote_name):
                fix = self.fixture()
                if remote_name != "github":
                    git(fix.root, "remote", "rename", "github", remote_name)
                fix.branch("automation/stranded")
                result = fix.run()
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn(f"not on {remote_name}", result.stdout)

    def test_non_automation_branch_is_out_of_scope(self):
        """Only automation/** is subject to the PR-handoff contract."""
        fix = self.fixture()
        git(fix.root, "checkout", "-q", "-b", "scratch", "main")
        commit(fix.root, "scratch-work")
        git(fix.root, "checkout", "-q", "main")
        result = fix.run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("scratch", result.stdout)

    def test_no_automation_branches_is_clean(self):
        fix = self.fixture()
        result = fix.run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_quiet_suppresses_only_the_all_clear(self):
        fix = self.fixture()
        self.assertEqual(fix.run("--quiet").stdout, "")
        fix.branch("automation/stranded")
        noisy = fix.run("--quiet")
        self.assertEqual(noisy.returncode, 1, noisy.stdout + noisy.stderr)
        self.assertIn("automation/stranded", noisy.stdout)

    def test_real_tree_is_inspectable(self):
        """The gate must be runnable here, whatever today's verdict is.

        Deliberately not asserting a pass: this checkout legitimately carries
        in-flight branches. Exit 2 (uninspectable) is the real failure.
        """
        result = subprocess.run(
            [sys.executable, str(CHECKER)],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertIn(result.returncode, (0, 1), result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
