# Final authority check: RESTORE_REPO guard in normalization BASELINE

- Reviewer: Fable (bounded one-line authority check; `docs/fork/normalization/reviews/` contents not read, diff names only)
- Date: 2026-07-16T14:55Z (UTC)
- Mode: read-only. No file, ref, index, worktree, stash, service, process, runtime link, or host state was modified. Syntax and behavior tests ran only on a `/tmp` copy of the committed snippet and env-var subshells.

## Fixed refs

| Item | Value |
|---|---|
| Reviewed head | `9264b37dfe507064c2c58c4c46d9fa416cd6eed6` (`docs(fork): finalize normalization restore contract`) |
| Diff base | `e94842b0045063e0a8a29a8985e551b8f02e70b3` |
| Branch state | `recovery/2026-07-15` = HEAD = `9264b37d...` |
| Diff scope | `BASELINE.md` +1 line; two preserved hardening-pass review files added (not read) |

## Verdict

**PASS.** Zero CRITICAL or IMPORTANT findings. The guard resolves the sole outstanding non-blocking finding, is valid Bash and POSIX sh, fails closed, and introduces no regression. One clarifying note on the authority-derivation value below.

## Guard verification

- The exact BASELINE.md diff is a single added line before the restore subshell:
  `: "${RESTORE_REPO:?set RESTORE_REPO to the disposable restored repository}"`.
  This directly resolves my prior MINOR note (undefined `$RESTORE_REPO`).
- Syntax: the full committed snippet block extracted to `/tmp` passes both `bash -n` and `sh -n`. The `${var:?word}` form with the null colon utility is POSIX-specified, so the guard is valid in the documented `bash` block and in plain sh.
- Behavior (tested in disposable subshells): with `RESTORE_REPO` unset, the expansion aborts with the clear message `RESTORE_REPO: set RESTORE_REPO to the disposable restored repository` and a non-zero exit before the `cd` runs (fails closed, cannot silently `cd ""`). With it set, the guard is a no-op and execution proceeds.
- Placement is correct: guard precedes the `(cd "$RESTORE_REPO" ...)` subshell, and `:?` (not `?`) also rejects empty string, the stricter and correct choice.
- No other change to any authority file: `git diff --stat` confirms BASELINE.md is the only authority-file edit. The `7c68ef5f` assertion, stash table, count-transition paragraph, and all anchors are byte-identical otherwise. No gate weakened.

## Authority derivation

- HEAD, `recovery/2026-07-15`, and the fixed head all equal `9264b37dfe507064c2c58c4c46d9fa416cd6eed6`. The commit containing the current BASELINE.md is `9264b37d...` (`git log -1 --format='%H' -- docs/fork/normalization/BASELINE.md` returns it).
- Clarifying note (MINOR, non-blocking): the *documented* derivation command, `git log -1 --format='%H' -- docs/fork/normalization/COMPLETION_STANDARD.md`, returns `e94842b0...`, not `9264b37d...`, because this commit did not touch COMPLETION_STANDARD.md. This violates no documented requirement: the BRIEF only requires that the derived authority commit be reachable from branch HEAD (it is, as the parent), all four authority files are tracked, and files are read at HEAD. Read as designed, the derivation pins the last change to the binding *standard*, which is a coherent anchor that intentionally does not move on BASELINE-only appends (which will recur every session). Anyone needing the newest authority-content commit should derive per-file, as above. No action required; worth remembering when recording evidence.

## Validation performed (all read-only against the repo)

- Exact `git diff e94842b0..9264b37d -- docs/fork/normalization/BASELINE.md` and full `--stat`.
- Snippet extraction from the committed blob; `bash -n` and `sh -n`; unset/set behavior tests in throwaway subshells.
- HEAD/branch/derivation identity checks at the fixed head; verified worktree still shows only the preserved prompt as dirty.
- Existence-only listing of the two added hardening-pass review files.

## What was not checked

- Contents of anything under `docs/fork/normalization/reviews/` (excluded by instruction).
- Everything outside this one-line diff; the prior reviews at `1f938b7e` and `e94842b0` remain the authority for unchanged content.

## Confidence

High. The change is one line, and both its syntax and runtime failure mode were exercised directly.
