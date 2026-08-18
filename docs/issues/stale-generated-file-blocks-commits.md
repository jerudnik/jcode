---
title: "A gitignored generated file can block every commit, and the named remedy is not runnable"
status: open
priority: medium
owner: unassigned
opened: 2026-08-18
---

# Stale `CLAUDE.md` blocks all commits with no in-repo way to fix it

## Symptom

Every `git commit` in the working tree failed the `agent-instructions` hook:

    CLAUDE.md is stale; run apm compile

`apm` is not on `PATH` on this host, and nothing in the repository provides
it. The checker names a command the operator cannot run.

## What was verified

- `command -v apm` → not found (exit 1).
- `git check-ignore -v CLAUDE.md` → `.gitignore:27`. The file is **not
  tracked**. Its mode is `0600`.
- `scripts/check_agent_instructions.py:145` guards on
  `claude_path.exists()`: a *missing* `CLAUDE.md` is fine, a *present and
  stale* one is fatal. Deleting the file would satisfy the hook.
- The drift was real. The compiled body was missing a whole
  `## Documentation surfaces` section and carried a stale `## Child DOX
  index`. Regenerating it in place from `ROOT_PRIMITIVES`, reusing the
  checker's own `primitive()` / `compiled_body()` helpers, took the checker
  to `rc=0`.
- `AGENTS.md` — the tracked twin, compiled from the same primitives — was
  already current. Only the untracked copy had drifted.

## Mechanism

`CLAUDE.md` is a *generated, gitignored, per-machine* artifact. It is derived
from tracked sources (`ROOT_PRIMITIVES`), but the derivation is performed by a
tool that lives outside the repository. Consequences:

- Nothing in the repo can regenerate it, so a mismatch has no in-tree remedy.
- Because it is gitignored, it never appears in a diff, never appears in a
  PR, and is never checked by CI. It can only drift locally, and can only be
  discovered locally — at commit time, blocking the commit.
- Because it is per-machine, a fix on one host does not help any other.

The failure is not the checker being wrong. The checker was right: the file
*was* stale. The gap is that a mandatory gate depends on an artifact the
repository cannot produce.

## Absence read as success

Two variants showed up while diagnosing this:

- The checker's real exit code was masked. A bare `echo "rc=$?"` after a
  pipeline reports the *pipeline's* status. The checker was exiting 1
  throughout while the shell reported 0.
- After editing `CLAUDE.md`, `git diff --stat` showed nothing. For a
  gitignored file that is expected, but it reads as "the edit did not
  apply". `git check-ignore -v <path>` distinguishes the two.

## Why it matters

A stale generated file that no in-repo command can regenerate turns a
routine local state into a hard stop on all work, with a remedy that is
inert. The correct response — fix the cause, not bypass the hook — is only
available to someone willing to reimplement the generator inline.

## Suggested direction

Not prescriptive; any one of these closes it:

- Ship the compile step in-repo (a `just` recipe or a script) so the error
  message names something runnable, and have the checker point at that.
- Or have the checker regenerate the gitignored artifact itself when it is
  stale, since it already owns `primitive()` and `compiled_body()` and can
  therefore produce the correct bytes. It currently computes the right answer
  and then declines to write it.
- Or, if the file is genuinely optional, treat stale exactly like absent for
  the untracked copy: report it, do not block on it.

The tracked `AGENTS.md` should keep failing hard. That one is in the diff, is
seen by CI, and has a reviewable remedy.
