#!/usr/bin/env python3
# Verdict: KEEP-MANUAL
# Operator: the session coordinator runs this at local closeout, before opening
# or merging pull requests. It inspects workstation-local refs and GitHub state,
# so a shared CI checkout cannot own its result.
"""Report `automation/**` work that is not on a path to `main`.

Work in this repository lands through a PR from an `automation/**` branch;
direct pushes to `main` are rejected. Railway worker briefs tell workers not to
touch git at all, so the parent session owns every push, PR, and merge. That
isolation is sound, but it makes shipping depend on one session surviving long
enough to finish the handoff. When a coordinator ends or is compacted midway,
the work stops one step short of `main` and nothing says so: the branch is
local, the railway state still reads `pending`, and the loss is invisible until
somebody goes looking.

That is not hypothetical. Three branches carrying reviewed, gate-passing work
sat unpushed for two days (F24, F25, and a wave-scope checkpoint); one tip
commit was literally `fix(f25): close durability review blockers`, so the work
had been through review and still never shipped.

The stall has three distinct shapes, and a check that only knows the first one
gives false assurance once the work advances past it:

  unpushed      branch exists locally, absent from the remote
  no PR         branch reached the remote, but no PR was ever opened
  stalled PR    PR exists but is BEHIND or BLOCKED and nobody re-ran it

All three leave finished work unmerged, so all three are reported.

Exit status is non-zero when any branch is stalled, so this can run as a
closeout gate. It is read-only: it never fetches, pushes, or mutates refs.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BRANCH_PREFIX = "automation/"
BASE_BRANCH = "main"

# Remote names that have held the canonical fork, most-canonical first. The
# checkout's remote is *discovered*, never assumed: CI (actions/checkout) names
# it `origin`, while the canonical local clone names it `github`. Hardcoding
# `origin` is a bug this repository has already paid for once, in
# `ideal_base_railway.py`, where a fixed `refs/remotes/origin/main` made every
# bare railway invocation fail on the canonical checkout (commit 5ee7149b7).
CANONICAL_REMOTES = ("github", "origin")

# Merge states that mean a PR exists but cannot merge as it stands.
STALLED_MERGE_STATES = {"BEHIND", "BLOCKED", "DIRTY"}

# States that are a real answer meaning "this can merge". Everything outside
# both sets -- notably UNKNOWN -- is GitHub saying it has not finished
# computing, which is not the same as saying the PR is fine. The original
# version of this comment lumped UNKNOWN in with CLEAN as "not worth blocking
# a closeout over", and that produced a false all-clear: GitHub recomputes
# mergeability for every open PR whenever `main` moves, so a run seconds
# after a merge saw UNKNOWN for two genuinely stalled PRs and reported
# "all on a path to main". Unsettled state is reported as a gap, not silence.
SETTLED_MERGE_STATES = {"CLEAN", "UNSTABLE", "HAS_HOOKS"}


def git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def canonical_remote() -> str | None:
    """The configured remote most likely to be the canonical fork."""
    proc = git("remote")
    if proc.returncode != 0:
        return None
    configured = proc.stdout.split()
    for name in CANONICAL_REMOTES:
        if name in configured:
            return name
    return configured[0] if configured else None


def remote_branches(remote: str) -> set[str] | None:
    """Branch names present on `remote`, or None if it cannot be reached.

    One `ls-remote` for the whole run: per-branch queries would turn a closeout
    check into a series of network round trips.
    """
    proc = git("ls-remote", "--heads", remote)
    if proc.returncode != 0:
        return None
    names = set()
    for line in proc.stdout.splitlines():
        _, _, ref = line.partition("\t")
        if ref.startswith("refs/heads/"):
            names.add(ref[len("refs/heads/") :])
    return names


def base_ref() -> str:
    """What counts as "landed": the remote's `main`, falling back to local.

    Local `main` is the wrong yardstick and this was caught by dogfooding: the
    moment PR #67 merged, the guard reported its own just-merged branch as
    "pushed, but no open PR", because the local `main` in that worktree still
    pointed at the pre-merge commit. Every merge would have produced a false
    positive until someone remembered to fast-forward.

    The remote-tracking ref also closes a false *negative*: a branch merged
    only into a local `main` that was never pushed is not actually landed, and
    comparing against local `main` would call it done.

    Read-only by design. This never fetches; a check that mutates the object
    store to make itself pass is not a check.
    """
    remote = canonical_remote()
    if remote:
        tracking = f"{remote}/{BASE_BRANCH}"
        if git("rev-parse", "--verify", "--quiet", tracking).returncode == 0:
            return tracking
    return BASE_BRANCH


def unmerged_automation_branches(base: str) -> list[tuple[str, int]]:
    """Local `automation/**` branches with commits not yet in `base`."""
    proc = git("for-each-ref", "--format=%(refname:short)", f"refs/heads/{BRANCH_PREFIX}")
    if proc.returncode != 0:
        return []
    branches = []
    for branch in proc.stdout.split():
        counted = git("rev-list", "--count", f"{base}..{branch}")
        if counted.returncode != 0:
            continue
        ahead = int(counted.stdout.strip() or 0)
        if ahead:
            branches.append((branch, ahead))
    return branches


def usable_repo() -> str | None:
    """Reason this checkout cannot be inspected, or None when it is fine.

    Without this, running from a path where `REPO_ROOT` is not a git repository
    (or where `main` does not exist) makes every query fail, every branch list
    come back empty, and the gate print a confident all-clear. A check that
    cannot distinguish "nothing is stalled" from "I looked in the wrong place"
    reproduces the exact false assurance it exists to prevent, so an
    uninspectable checkout is an error rather than a pass.
    """
    if git("rev-parse", "--git-dir").returncode != 0:
        return f"{REPO_ROOT} is not a git repository"
    if git("rev-parse", "--verify", "--quiet", BASE_BRANCH).returncode != 0:
        return f"no {BASE_BRANCH} branch in {REPO_ROOT}"
    return None


def pull_requests() -> dict[str, dict] | None:
    """PRs keyed by head branch, or None when `gh` cannot answer.

    Degrades rather than fails: without `gh` the unpushed shape is still
    detectable from git alone, and a closeout on a machine with no GitHub auth
    should report what it can instead of erroring out.

    Merged PRs are included deliberately. `gh pr merge` does not touch local
    refs, so a branch can be landed on the remote while every local ref still
    says otherwise, and asking GitHub is the only read-only way to know. First
    entry wins if a branch was reused, and `gh` lists most recent first.
    """
    if shutil.which("gh") is None:
        return None
    proc = subprocess.run(
        [
            "gh",
            "pr",
            "list",
            "--state",
            "all",
            "--limit",
            "100",
            "--json",
            "number,headRefName,state,mergeStateStatus",
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return None
    try:
        entries = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return None
    prs: dict[str, dict] = {}
    for entry in entries:
        prs.setdefault(entry["headRefName"], entry)
    return prs


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="print only stalled branches, not the all-clear line",
    )
    args = parser.parse_args()

    if reason := usable_repo():
        print(f"branch handoff: cannot inspect this checkout ({reason})", file=sys.stderr)
        return 2

    base = base_ref()
    branches = unmerged_automation_branches(base)
    if not branches:
        if not args.quiet:
            print("branch handoff: no unmerged automation/** branches")
        return 0

    remote = canonical_remote()
    published = remote_branches(remote) if remote else None
    prs = pull_requests()

    findings: list[str] = []
    # PRs whose mergeability GitHub had not finished computing. Reported as a
    # gap on both the passing and failing paths, never silently dropped.
    unknown: list[str] = []
    for branch, ahead in sorted(branches):
        commits = f"{ahead} commit{'s' if ahead != 1 else ''}"

        if published is not None and branch not in published:
            findings.append(
                f"  {branch}: {commits} ahead of {base}, not on {remote}\n"
                f"      -> git push -u {remote} {branch} && gh pr create"
            )
            continue

        if prs is None:
            # Pushed, but PR state is unknowable here. Not a finding: reporting
            # it as stalled would fail every closeout on an unauthenticated
            # machine.
            continue

        pr = prs.get(branch)
        if pr is None:
            findings.append(
                f"  {branch}: {commits} ahead of {base}, pushed, but no open PR\n"
                f"      -> gh pr create --head {branch}"
            )
            continue

        if pr.get("state") == "MERGED":
            # Landed on the remote; the local refs just have not caught up.
            # Not stalled work, so not a finding. Do not fetch to confirm: a
            # check that mutates the object store to make itself pass is not
            # a check.
            continue

        if pr.get("state") == "CLOSED":
            findings.append(
                f"  {branch}: {commits} ahead of {base}, PR #{pr['number']} was closed unmerged\n"
                f"      -> reopen it, open a new PR, or delete the branch"
            )
            continue

        state = pr.get("mergeStateStatus", "UNKNOWN")
        if state in STALLED_MERGE_STATES:
            findings.append(
                f"  {branch}: PR #{pr['number']} is open but {state}\n"
                f"      -> gh pr checks {pr['number']}, then update the branch"
            )
        elif state not in SETTLED_MERGE_STATES:
            # GitHub computes mergeability asynchronously and reports UNKNOWN
            # while it recomputes, which it does for every open PR whenever
            # `main` moves. Treating that as fine is a false all-clear at the
            # worst possible moment: right after a merge, when this check is
            # most likely to be run. It is not a finding either, because the
            # PR may well be fine. It is a gap in what this run could see.
            unknown.append(f"PR #{pr['number']} ({branch}) reported {state}")

    # What could not be checked. A clean run that skipped a whole class of
    # stall is a PARTIAL result, not an all-clear, so these are reported on
    # both paths. Saying "all on a path to main" while silently not having
    # looked is the same false assurance that `usable_repo` exists to prevent.
    unchecked: list[str] = []
    if published is None and remote:
        unchecked.append(f"could not read {remote}; unpushed branches were not checked")
    if prs is None:
        unchecked.append("gh unavailable; PR-state stalls were not checked")
    for note in unknown:
        unchecked.append(f"{note}; GitHub had not finished computing mergeability")
    if unknown:
        unchecked.append("re-run in a minute to get a settled answer")

    if not findings:
        if not args.quiet:
            scope = f"{len(branches)} automation branch{'es' if len(branches) != 1 else ''}"
            if unchecked:
                print(f"branch handoff: {scope} unmerged, PARTIAL CHECK ONLY")
                for note in unchecked:
                    print(f"  (note: {note})")
            else:
                print(f"branch handoff: {scope} unmerged, all on a path to {BASE_BRANCH}")
        return 0

    print(f"branch handoff: work is not on a path to {BASE_BRANCH}\n")
    print("\n".join(findings))
    print(
        f"\nEach branch above holds commits that are not in {BASE_BRANCH} and are not"
        "\nmoving toward it. Land them, or delete the branch if it is obsolete."
    )
    for note in unchecked:
        print(f"(note: {note})")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
