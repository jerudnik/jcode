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


def git_out(cwd: Path, *args: str) -> str:
    proc = subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True)
    return proc.stdout.strip()


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
        self.env = dict(NO_GH_ENV)
        # The checker is committed on `main`, before any branch exists, so it
        # is present on every branch. Leaving it untracked does not work: the
        # `git add -A` in `branch()` sweeps it into that branch's commit, and
        # checking `main` back out then deletes it as a file `main` never had.
        # That produced a confusing "No such file or directory" from the
        # subprocess, which looked like a temp-directory lifetime problem and
        # is not one.
        commit(self.root, "seed")
        # `main` exists on the remote too, as it does in the real fork. Without
        # this the remote-tracking `main` is missing and the checker silently
        # falls back to the local ref, which is the very thing under test.
        git(self.root, "push", "-q", "-u", "github", "main")

    def branch(self, name: str, *, push: bool = False, merge: bool = False) -> None:
        git(self.root, "checkout", "-q", "-b", name, "main")
        commit(self.root, name.replace("/", "-"))
        if push:
            git(self.root, "push", "-q", "github", name)
        git(self.root, "checkout", "-q", "main")
        if merge:
            git(self.root, "merge", "-q", "--no-ff", name, "-m", f"merge {name}")

    def merge_on_remote(self, name: str) -> None:
        """Land `name` on the remote's `main` without touching local `main`.

        Uses a scratch clone so the local checkout's refs are untouched apart
        from the remote-tracking update, reproducing the real situation: the
        fork's `main` has moved and this working copy has not caught up.
        """
        scratch = self.root.parent / f"scratch-{name.replace('/', '-')}"
        git(self.root.parent, "clone", "-q", str(self.remote), str(scratch))
        git(scratch, "config", "user.email", "gate@example.invalid")
        git(scratch, "config", "user.name", "gate")
        git(scratch, "checkout", "-q", "main")
        git(scratch, "merge", "-q", "--no-ff", f"origin/{name}", "-m", f"merge {name}")
        git(scratch, "push", "-q", "origin", "main")
        # Update only the remote-tracking ref, as a plain `git fetch` would.
        git(self.root, "fetch", "-q", "github")

    def run(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(self.checker), *args],
            cwd=self.root,
            capture_output=True,
            text=True,
            check=False,
            env=self.env,
        )

    def install_stub_gh(self, payload: str = "[]") -> None:
        """Put a fake `gh` on PATH that answers the PR query with `payload`.

        Only used to reach the fully-checked verdict. Everything else runs
        degraded on purpose, so the tests stay offline.
        """
        bindir = self.root.parent / "stubbin"
        bindir.mkdir(exist_ok=True)
        stub = bindir / "gh"
        # Honour `--state` the way real `gh` does. A stub that returns every
        # PR regardless of the query makes tests vacuous: an earlier version
        # of this helper ignored the flag, so a mutation reverting the checker
        # to `--state open` still saw MERGED entries and the suite stayed
        # green. The stub must be able to hide what the caller did not ask for.
        stub.write_text(
            "#!/bin/sh\n"
            "state=open\n"
            'while [ $# -gt 0 ]; do\n'
            '  case "$1" in --state) state="$2"; shift 2 ;; *) shift ;; esac\n'
            "done\n"
            "payload=$(cat <<'JSON'\n"
            f"{payload}\n"
            "JSON\n"
            ")\n"
            'if [ "$state" = all ]; then printf %s "$payload"; else\n'
            "  printf %s \"$payload\" | /usr/bin/python3 -c \"import json,sys;"
            "print(json.dumps([e for e in json.load(sys.stdin)"
            " if e.get('state','OPEN')=='OPEN']))\"\n"
            "fi\n",
            encoding="utf-8",
        )
        stub.chmod(0o755)
        self.env = {**NO_GH_ENV, "PATH": f"{bindir}:{NO_GH_ENV['PATH']}"}


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
        """A branch landed on the remote's main is done, not stalled."""
        fix = self.fixture()
        fix.branch("automation/landed", push=True)
        fix.merge_on_remote("automation/landed")
        result = fix.run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("automation/landed", result.stdout)

    def test_merge_into_an_unpushed_local_main_is_not_landed(self):
        """Merging locally and never pushing does not count as landed.

        The false negative on the other side of the stale-base fix. Local
        `main` is not the yardstick: a branch merged only into a local `main`
        that never left the machine is precisely the stranded work this guard
        exists to find, and comparing against local `main` would call it done.
        """
        fix = self.fixture()
        fix.branch("automation/local-only", merge=True)
        result = fix.run()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("automation/local-only", result.stdout)

    def test_branch_merged_on_the_remote_is_not_flagged(self):
        """A branch merged upstream is landed, even if local `main` is stale.

        Regression test for a bug found by dogfooding the guard immediately
        after its own PR merged: the check compared against local `main`, which
        in that worktree still pointed at the pre-merge commit, so it reported
        the just-merged branch as "pushed, but no open PR". Comparing against
        local `main` makes every merge produce a false positive until someone
        remembers to fast-forward, which is exactly the kind of noise that
        trains people to ignore a gate.
        """
        fix = self.fixture()
        fix.branch("automation/landed-upstream", push=True)
        # The remote merges it; the local checkout has not caught up.
        fix.merge_on_remote("automation/landed-upstream")
        stale = git_out(fix.root, "rev-parse", "main")
        # `gh` must answer here. Run degraded, the pushed branch is skipped
        # before the base ref is ever consulted, and the test passes whether
        # or not the bug is present -- which is how an earlier version of this
        # test was vacuous.
        fix.install_stub_gh()
        result = fix.run()
        self.assertEqual(
            git_out(fix.root, "rev-parse", "main"),
            stale,
            "the check must not fetch or move local refs",
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("automation/landed-upstream", result.stdout)

    def test_merged_pr_lands_a_branch_even_when_every_local_ref_is_stale(self):
        """A merged PR is landed, even if the remote-tracking ref is stale too.

        `base_ref()` fixed the case where local `main` lagged the remote, but
        the remote-tracking ref is only as fresh as the last fetch, and
        `gh pr merge` does not touch local refs at all. So the original
        dogfooding scenario survived that fix: merge a PR, do not fetch, and
        the guard still told you to open a PR for work that was already in
        `main`. Asking GitHub for the PR state is the only way to know this
        without fetching.
        """
        fix = self.fixture()
        fix.branch("automation/merged-elsewhere", push=True)
        before = git_out(fix.root, "rev-parse", "github/main")
        fix.install_stub_gh(
            '[{"number": 41, "headRefName": "automation/merged-elsewhere",'
            ' "state": "MERGED", "mergeStateStatus": "UNKNOWN"}]'
        )
        result = fix.run()
        self.assertEqual(
            git_out(fix.root, "rev-parse", "github/main"),
            before,
            "the check must not fetch to confirm the merge",
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("automation/merged-elsewhere", result.stdout)

    def test_pr_closed_without_merging_is_a_stall(self):
        """A closed-unmerged PR strands work exactly like never opening one.

        Found while testing the merged case: querying `--state all` to see
        merged PRs also surfaces closed ones, and a branch whose PR was closed
        without merging is the same stranded work this guard exists to find.
        Treating any non-open PR as "landed" would have been a false negative.
        """
        fix = self.fixture()
        fix.branch("automation/abandoned", push=True)
        fix.install_stub_gh(
            '[{"number": 42, "headRefName": "automation/abandoned",'
            ' "state": "CLOSED", "mergeStateStatus": "UNKNOWN"}]'
        )
        result = fix.run()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("automation/abandoned", result.stdout)
        self.assertIn("closed unmerged", result.stdout)

    def test_pushed_branch_without_gh_is_not_a_false_positive(self):
        """Without `gh`, a pushed branch cannot be judged, so it is not flagged.

        Flagging it would fail every closeout on an unauthenticated machine,
        which trains people to ignore the gate. But passing must not be
        reported as an all-clear either: PR-state stalls were never checked,
        so the verdict is partial and has to say so.
        """
        fix = self.fixture()
        fix.branch("automation/pushed", push=True)
        result = fix.run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("PARTIAL CHECK ONLY", result.stdout)
        self.assertIn("gh unavailable", result.stdout)
        self.assertNotIn("all on a path to main", result.stdout)

    def test_unqualified_all_clear_requires_a_complete_check(self):
        """The unqualified all-clear is printed only when nothing was skipped.

        Guards the other side of the partial-verdict rule: with `gh` present
        and answering, the verdict is complete and may say so plainly. Without
        this, the unqualified all-clear branch would be unreachable and the
        distinction between "clean" and "did not look" would be cosmetic.
        """
        fix = self.fixture()
        fix.branch("automation/pushed", push=True)
        fix.install_stub_gh('[{"number":1,"headRefName":"automation/pushed","mergeStateStatus":"CLEAN"}]')
        result = fix.run()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("all on a path to main", result.stdout)
        self.assertNotIn("PARTIAL", result.stdout)

    def test_pr_state_stall_is_reported_when_gh_answers(self):
        """A PR that is open but not mergeable is still a stall."""
        fix = self.fixture()
        fix.branch("automation/behind", push=True)
        fix.install_stub_gh('[{"number":9,"headRefName":"automation/behind","mergeStateStatus":"BEHIND"}]')
        result = fix.run()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("PR #9 is open but BEHIND", result.stdout)

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
