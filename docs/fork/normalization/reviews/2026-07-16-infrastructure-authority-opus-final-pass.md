# Normalization Infrastructure: Final Authority Check (Opus)

Read-only one-line operational authority re-review of the restore-contract finalization.

## Fixed refs

- Fixed head reviewed: `9264b37dfe507064c2c58c4c46d9fa416cd6eed6`
  ("docs(fork): finalize normalization restore contract")
- Prior head: `e94842b0045063e0a8a29a8985e551b8f02e70b3`
- Diff range: `e94842b0..9264b37d` (single commit)
- Repo HEAD at review: `9264b37dfe507064c2c58c4c46d9fa416cd6eed6` (branch `recovery/2026-07-15`)
- Scope honored: reviewed only the `BASELINE.md` diff and current snippet. I did
  NOT read anything under `docs/fork/normalization/reviews/`, including the two
  newly added `*-hardening-{fable,opus}-pass.md` files.

## Verdict

**PASS.** Zero unresolved CRITICAL or IMPORTANT findings. The commit adds exactly
one defensive shell line that fail-closes the disposable restore verification when
`RESTORE_REPO` is undefined, before any `cd`. It resolves the undefined-variable
MINOR, is valid POSIX/bash syntax, and introduces no mutation path and no approval
bypass.

## Item verification (live read-only)

| Requirement | Result | Evidence |
|---|---|---|
| `: "${RESTORE_REPO:?...}"` fail-closes before `cd "$RESTORE_REPO"` | PASS | The guard is line 65; `cd "$RESTORE_REPO"` is line 67 (inside the subshell that follows). Behavioral test: with `RESTORE_REPO` unset, `bash -c ': "${RESTORE_REPO:?...}"; echo REACHED_CD'` prints the error and exits non-zero (127) without ever reaching the `cd`; with it set, the guard passes through. The `:?` expansion aborts the shell on unset/empty before the subshell runs. |
| Valid shell syntax | PASS | `bash -n` on the extracted block returns clean ("SYNTAX OK"). `: "${VAR:?msg}"` is a standard no-op-with-guard idiom. |
| Resolves the undefined-variable minor | PASS | Previously `cd "$RESTORE_REPO"` would run with an empty variable (`cd ""` is a no-op staying in the current dir, silently running `cat-file` against the wrong repo). The guard now forces the operator to set `RESTORE_REPO` to the disposable restored repo, making the payload-presence check meaningful rather than accidentally passing against the live repo. |
| No mutation or approval bypass | PASS | `RESTORE_REPO` is referenced only twice, both read-only: the guard and `cd "$RESTORE_REPO"`; the subshell body is a single `git cat-file -e` (existence check, no write). The added line is a pure precondition assertion. No destructive verb, no approval-gate text, and no other file changed. The surrounding "Deleting the archive refs is not authorized until final cleanup approval and successful restoration testing" gate is untouched. |
| Authority derivation | PASS | `git log -1 --format='%H' -- docs/fork/normalization/COMPLETION_STANDARD.md` = `e94842b0...` (unchanged by this commit, correctly the last commit that touched it) and is an ancestor of HEAD `9264b37d`. All four authority files remain tracked. |
| Sole dirty prompt state | PASS | `git status --short` shows only `M docs/fork/recovery/ORCHESTRATOR_PROMPT.md`; HEAD = `9264b37d`. |

## New findings

None (CRITICAL, IMPORTANT, or otherwise blocking).

- INFORMATIONAL-1: The guard is inside a fenced runbook example, so it protects a
  human/agent copy-pasting the restore verification. It is a strict improvement
  and cannot regress any automated path (there is none). The `cd` is already
  scoped in a subshell, so a failed guard also cannot leave the operator's shell
  in the wrong directory.

## Validation performed (all read-only)

- `git cat-file -t`, `git log/diff --name-only` on the range: single commit, three
  files (BASELINE + two reviews/ files not opened).
- Full `git diff` of BASELINE: exactly one added line.
- `bash -n` syntax check of the runbook block.
- Behavioral fail-close test: unset `RESTORE_REPO` -> non-zero exit before `cd`;
  set -> passes.
- `grep -n RESTORE_REPO` on the current file: two read-only uses only.
- Authority derivation + ancestor check; four authority files tracked.
- `git status --short`; HEAD confirmation.

## Confidence

High. The change is a single, well-understood shell idiom, verified both
statically (`bash -n`) and behaviorally (fail-close), with no mutation surface.

## What I did NOT check

- Contents of `docs/fork/normalization/reviews/*` (excluded by instruction),
  including the two new hardening-pass files.
- Unchanged authority content already confirmed in prior reviews (no other lines
  changed in this diff).
- Actual restore/bundle execution (approval-gated, read-only-first; not run).
