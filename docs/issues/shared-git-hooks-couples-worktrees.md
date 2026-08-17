---
title: "Shared .git/hooks couples worktrees: hooks repointed into another session's worktree"
status: open
priority: medium
owner: unassigned
opened: 2026-08-17
---

# Shared `.git/hooks` couples worktrees

## Symptom

While looking for the PM-surface hook, `.git/hooks/pre-commit` in the primary
checkout (`/Users/jrudnik/labs/jcode`) was found to exec out of a *different*
worktree:

    $ cat .git/hooks/pre-commit
    #!/usr/bin/env bash
    # Managed by scripts/install-git-hooks.sh for jcode
    exec "/Users/jrudnik/labs/jcode-guardrail/scripts/git-hooks/pre-commit" "$@"

Same for `pre-push`. Both files were rewritten at `Aug 17 05:41`.
`/Users/jrudnik/labs/jcode-guardrail` is a linked worktree belonging to a
concurrent agent session, checked out on `automation/guard-nonvacuity`.

## Verified

- Both hook scripts were diffed against the primary checkout's own copies
  (`scripts/git-hooks/pre-commit`, `pre-push`) and are **byte-identical**. No
  behavioural divergence has occurred, so nothing has actually misbehaved yet.
- `.git/hooks/check-backlog-tracking.sh` is a symlink to
  `/Users/jrudnik/labs/jcode/scripts/git-hooks/check-backlog-tracking.sh`,
  which does not exist — a pre-existing broken symlink, unrelated to the above
  but noticed at the same time. It is not referenced by `pre-commit`, so it is
  inert.

## Mechanism

Linked worktrees share the main repository's `.git/hooks` directory. Git does
not give a worktree its own hooks unless `core.hooksPath` is set per-worktree.
`scripts/install-git-hooks.sh` writes absolute paths into the shared directory,
so **whichever worktree last ran the installer wins for every worktree.**

## Why it matters

1. A concurrent session working on guard/hook scripts can change the hooks that
   the primary checkout's commits run, without touching the primary checkout.
2. If that worktree is removed, `exec` fails and commits in the primary checkout
   break with a confusing error pointing at a path the user may not recognise.
3. The failure is silent in the direction that matters: the hooks *look*
   installed and correct, and today they even *are* correct, so nothing prompts
   anyone to check whose copy is running.

This is the same class as two other problems seen today: a control that appears
present while pointing somewhere unintended, where the absence of a difference
is read as evidence of correctness.

## Options

- Set `core.hooksPath` per worktree so each checkout runs its own hooks.
- Make `install-git-hooks.sh` write relative paths, or resolve via
  `git rev-parse --show-toplevel` at hook runtime rather than baking an absolute
  path at install time.
- Have the installer refuse to run from a linked worktree, or warn that it is
  about to rebind hooks for every worktree.
- At minimum, have the hook assert that the script it is about to exec lives
  inside the worktree the commit is being made in, and fail loudly if not.

## Notes

No change has been made to the hooks. Recorded rather than fixed because it
touches `scripts/git-hooks/` while a governance transaction on the ruleset is
paused and a concurrent session is holding commits.
