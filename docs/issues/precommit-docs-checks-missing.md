---
title: The pre-commit hook does not run the docs checks it could afford to run
status: open
priority: medium
owner: maintainers
opened: 2026-08-28
related:
  - scripts/git-hooks/pre-commit
  - scripts/git-hooks/check-pm-surface.sh
  - scripts/install-git-hooks.sh
  - scripts/lint_docs.py
  - scripts/check_docs_references.py
  - tests/test_install_git_hooks.py
---

# The pre-commit hook does not run the docs checks it could afford to run

Two documentation gates run only in pull request CI. Both are fast enough to
run at commit time, and both fail on defects an author introduces in the commit
that creates them. Today the author learns about the defect minutes later, from
CI, after pushing.

This is a small change. The hook infrastructure already exists and already
runs. What is missing is two invocations inside it.

## What already exists

Verified 2026-08-28 in a clean worktree. This section is written down because
an earlier survey of this same surface got it wrong, and the wrong version is
more intuitive than the right one.

`scripts/install-git-hooks.sh` installs a managed shim for **both** `pre-push`
and `pre-commit`. The pre-commit branch is at the end of the script and uses
the same not-clobbering-an-existing-hook logic as pre-push. Both hooks are
installed and live:

```console
$ git rev-parse --git-path hooks/pre-commit
.git/hooks/pre-commit
$ head -2 "$(git rev-parse --git-path hooks/pre-commit)"
#!/usr/bin/env bash
# Managed by scripts/install-git-hooks.sh for jcode
```

`scripts/git-hooks/pre-commit` is the dispatcher. It runs two checks:

| check | measured |
|---|---|
| `scripts/git-hooks/check-pm-surface.sh` | 0.10s |
| `scripts/check_agent_instructions.py` | 0.16s |
| total | ~0.26s |

So the dispatcher is not dead code, and this issue is not about wiring it. The
budget question is what fits in the roughly 1.7s of headroom under a 2s target.

## The gap

Two checks run in pull request CI and not at commit time:

- `scripts/lint_docs.py` runs vale over tracked Markdown.
- `scripts/check_docs_references.py` verifies that documentation points at
  things that exist: issue frontmatter, repository-relative links, machine-local
  paths, and cited source paths.

Both fail on author-introduced defects. Filing one issue document in this
session tripped `check_docs_references.py` four times: one `/Users/...` path
that no other reader can resolve, and three citations to a script the issue was
proposing to create. All four were fixable in under a minute, and all four cost
a CI round trip instead.

## Measurements

Taken 2026-08-28 in this worktree, over 118 tracked Markdown files. Each figure
is the median of three runs.

| invocation | time |
|---|---|
| `lint_docs.py`, whole repo, vale on `PATH` | 0.80s |
| `lint_docs.py`, one staged file via `--files-from` | 0.47s |
| `lint_docs.py`, whole repo, wrapped in `nix shell nixpkgs#vale` | 2.58s |
| `check_docs_references.py`, whole repo | 0.98s |

The third row is the important one. Wrapping the linter in `nix shell` costs
about 1.8 seconds of resolution overhead before vale reads a single file. That
is the entire hook budget spent on tool lookup.

Two consequences:

1. The hook must call vale from `PATH` and must not wrap it in `nix shell`.
2. `just pre-pr` and CI should keep the Nix wrapper, because they want the
   pinned reproducible toolchain and 1.8s is irrelevant against a full mirror.

These two layers should therefore make **opposite** choices on purpose. Record
that in both places so a later cleanup does not "fix" the inconsistency.

## Proposed change

Add a step to `scripts/git-hooks/pre-commit`, gated on what is staged.

**Gate.** Collect staged paths with `git diff --cached --name-only`. Follow the
portable loop already used in `check-pm-surface.sh`; macOS ships bash 3.2,
which has no `mapfile`.

Run the docs step when either of these holds:

- a tracked Markdown file is staged, or
- a non-Markdown file is staged as a delete or a rename
  (`--diff-filter=DR`).

The second condition matters and is easy to miss. `check_docs_references.py`
fails when documentation cites a source path that no longer exists. That defect
is introduced by the commit that **moves the code**, not by any commit touching
documentation. Gating only on Markdown would let exactly the defect the checker
exists to catch pass through.

**Steps, when the gate opens.**

1. `lint_docs.py --files-from <staged markdown>`, using the staged list rather
   than the whole repository. Roughly 0.47s for a typical commit.
2. `check_docs_references.py` over the whole repository. It validates
   cross-references, so it cannot be made incremental: a link in one document
   points at another, and the file that breaks it is often not the file being
   linted. 0.98s.

Total for a docs-touching commit: about 1.7s including the two existing checks.
A commit touching no documentation stays at 0.26s.

**Missing vale.** With vale absent, `lint_docs.py` currently dies on an
uncaught `FileNotFoundError` traceback. This was hit twice by two people in one
session. The hook must detect a missing vale first and print a one-line install
hint, then skip the lint step and continue. Failing the commit is wrong here:
the developer has no defect, only an unprovisioned machine.

Note that a missing vale is what produced the incorrect 0.97s timing in the
earlier survey of this surface. The script died instantly, and the number was
recorded as if it had done the work. A fast, silent success is the failure mode
worth guarding against, which is the same argument `lint_docs.py` makes in its
own module docstring about vale exiting 0 when handed no files.

**Bypass.** Follow the existing convention. `check-pm-surface.sh` honors
`PM_SURFACE_OK=1`. Provide an equivalent for the docs step.

## Why a hook and not only the pull request gate

The hook is feedback, not enforcement. Pull request CI stays authoritative and
this change does not alter it.

The split is about cost. Agents in this repository commit constantly, so a
check that costs 2 seconds at commit time is affordable and one that costs
minutes is not. A hook that runs the full test suite gets bypassed with
`--no-verify` within a day, and a 1.7s check deferred to pull request time
wastes the fast feedback it could have given for free.

For the same reason, the following belong in the pull request layer and must
not move into the hook: the Python test suite (about 2m50s), anything that
compiles, anything networked, and `scripts/classify_pr_paths.py`, which
classifies a pull request's changed paths and has no meaning for a single
commit.

## Known limitation

The hook sees one commit. A branch can still arrive at a broken state across
several commits where no single commit trips the gate: for example one commit
moves a source file while a later commit adds the citation. Only the pull
request gate sees the branch as a whole. This is a reason to keep the pull
request gate authoritative, not a reason to skip the hook.

## Acceptance

- [ ] `scripts/git-hooks/pre-commit` runs both docs checks when the gate opens.
- [ ] A commit touching no documentation and moving no files stays at about
      0.26s.
- [ ] A commit staging one Markdown file completes in under 2s.
- [ ] Deleting or renaming a cited source file trips the checker at commit time.
- [ ] With vale absent, the hook prints an install hint and the commit succeeds.
- [ ] A bypass environment variable skips the docs step.
- [ ] `tests/test_install_git_hooks.py` still passes.
