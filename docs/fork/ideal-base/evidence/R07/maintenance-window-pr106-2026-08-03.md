# §4 maintenance window: PR #106 (stale code-path citations, ratchet re-armed)

Executed 2026-08-03, the fourth use of the procedure and the third on the same
day. Recorded because §4 makes every write and read-back in a window evidence.

## Why a window was needed

PR #106 edits `scripts/check_docs_references.py` and
`scripts/test_docs_references.py`, both protected. `Governance Root` fails by
design on a protected-path change: it runs from the pull request head and so
detects an unreviewed governance-path change on the PR that makes it, which
means a legitimate change cannot make it green from inside the PR. `--admin` is
refused because `protect-fork-rails` has `bypass_actors: []`, which binds the
owner-admin too.

The protected-path prediction was made before pushing and then confirmed
against real CI: predicted exactly these two files, and the failing step named
exactly these two files and nothing else.

Authorization was given by the repository owner in session, after the
pre-window state was posted and work stopped.

## Sequence executed

| # | step | result |
|---|---|---|
| 1 | confirm PR state | `mergeable=MERGEABLE state=BLOCKED`, sole non-pass `Governance Root` |
| 2 | capture pre-change ruleset | `protect-fork-rails`, `active`, `bypass_actors []`, 4 contexts |
| 3 | hash with the pinned encoder | `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b` |
| 4 | record pre-window main tip | `001e1a4e0403d98223c3753cbb042dd50594ec24` |
| 5 | pre-window `fork-health.sh --live` | all invariants hold |
| 6 | PUT dropping **only** `Governance Root` | 4 -> 3, read back `Fork CI Gate, Security Gate, Nix Gate` |
| 7 | SHA-conditioned merge to reviewed head | `merged=true`, merge commit `ca475f85f` |
| 8 | PUT restoring the exact pre-change body | read back all 4 contexts |
| 9 | body equality proof | pre == post == `43ba61a7a5…`, **RESTORED EXACTLY** |
| 10 | merge-commit bound | 1 merge commit created in the window |
| 11 | two-parent proof | parent1 == pre-window tip, parent2 == reviewed head `b0ec290fe` |
| 12 | post-window `fork-health.sh --live` | all invariants hold |

Window open `19:40:52Z`, closed `19:41:17Z`: 25 seconds.

Step 9 hashes a **fresh GET**, not the PUT response echo. A PUT response is the
server repeating what it was asked to store; only an independent read shows
what is actually stored.

## The hash is unchanged again, and that remains the expected result

This window's governed-body SHA-256 is `43ba61a7a5…`, byte-identical to the
#101 and #104 windows. Nothing between those windows and this one changed the
protected set, so the governed body did not change either.

The rule recorded in #101 and #104 still holds and is worth restating, because
its two halves are easy to conflate: equality across windows is evidence only
when the protected set was untouched in between, and inequality is evidence
only when it was. #100 changed the set and its hash differed (`efc2323f16…`).

Note that this PR did **not** change the protected set: it changes the contents
of two already-protected files. That is why the hash is expected to be equal
here even though the merged change touches governance-protected paths.

## Merge-count bound

Both forms were run, per the correction established in #101:

    git rev-list --count --merges <pre>..github/main              => 1
    git rev-list --merges --count <pre>..github/main --not <head> => 1

They agree because PR #106's branch was linear, with no merge in its own
history. This is the same reading as #104, and it is the case where the
correction is expected to make no difference. The forms disagree only when the
reviewed head is itself a merge commit, which is exactly what happened in #101.

## What landed

All 85 stale code-path citations are repaired; the `stale-code-path` ratchet now
reads 0 across 137 active documents.

Classification was by git history, not by basename. A basename heuristic tried
first mapped `src/tui/client.rs` to `crates/jcode-base/src/mcp/client.rs`, but
git shows that file was deleted in `c3184809e`, so writing that "fix" would have
planted a false citation into a gate whose entire purpose is to prevent false
citations. Rename chains were followed transitively and accepted only when the
endpoint is tracked today; spot checks compared top-level item names across each
rename to confirm content continuity rather than trusting `-M` similarity.

- **67 renamed** by the crates/ restructure, repointed to current paths.
- **11 deleted.** Three frozen records of the retired upstream-tracking model
  are exempted rather than repaired: each is already headed historical and
  superseded, and `patch-ledger.md` states in its own text that some rows
  intentionally describe sync machinery that no longer exists. Repointing those
  would falsify the record. The rest are rephrased to record the deletion.
- **7 never tracked as written.** One was real: `jade_relay.rs` exists under
  `crates/jcode-app-core/src/server/` and holds the cited helpers, so my own
  NEVER label was wrong and was corrected. "Never tracked at that path" is not
  "never existed", and finding that forced re-testing the other six the same
  way instead of assuming. The rest are illustrative or forward-looking paths in
  plan documents, unbacktick-ed so they stop rendering as citations a reader can
  open.

`MODULAR_ARCHITECTURE_RFC.md` needed more than paths. Its "current chokepoints"
line counts were stale and its claim that "the root crate still has" those
modules was false, because the split the RFC proposes has largely happened: the
root crate now retains only `main.rs`, `lib.rs`, `bin/`, and `cli/`. Correct
paths under a false sentence are still wrong, so the section was re-measured.
`protocol.rs` is now a one-line re-export of the extracted `jcode-protocol`
crate, which is the shape the RFC targets for the remaining modules.

## The defect that reaching zero exposed

Driving the rule to 0 disarmed the ratchet. A rule at zero has an **empty**
per-file dict, and `write_baselines` read an empty dict as "never measured",
which is the branch that lets a rule's first measurement establish its ceiling.
So the next `--update` would have accepted any number of new stale citations as
a first measurement.

Completing the cleanup would have been what unlocked the regression. Rule state
is now keyed off the `<key>_total` the baseline already records on every
refresh, which distinguishes "measured, and the answer was zero" from "never
measured" without changing the baseline file format.

This is the same defect class this program keeps finding: a guard that silently
answers fine. It is the third instance recorded here, after the `is None`
sentinel and the unvalidated `CLAUDE.md`.

## Controls, re-run on merged main

Each plants the bad state rather than re-reading the fix, and the exit status is
read directly rather than through a pipe, because an earlier window recorded a
meaningless `exit=0` that was really `tail`'s status:

    new stale citation, plain check   -> exit 1
    same, --update                    -> exit 1   (refuses 0 -> 1)
    same, with the guard REVERTED     -> exit 0   (proves the guard is what bites)
    restored                          -> exit 0

The third line is the one that matters: without it, the second line only shows
that something rejected the regression, not that the new guard did.

33 tests pass, and exactly one fails when the guard is reverted.

## Errors made in this window's work, recorded rather than hidden

- The protected-path predictor initially reported **0 protected paths** because
  it read keys that do not exist in `scripts/required-checks.json`. The real
  list is inline in `.github/workflows/governance-root.yml`. A predictor that
  reports "no protected paths" on a repo with 32 of them is worse than none, and
  this is the second time this same predictor has been wrong by guessing at a
  file's shape instead of reading it.
- A `git checkout docs/WINDOWS.md` used to undo a planted control silently
  discarded 17 uncommitted repairs in that file, because `git checkout <file>`
  restores from HEAD, not from the pre-plant working state. Caught on re-read,
  reapplied, and the remaining controls used file backups instead.

## What this window does not prove

`design.md` §114 still applies: a change made and reverted between two `--live`
samples is not guaranteed to be caught by the audit boundary alone. This record
does not claim continuous coverage across the 25-second window; it claims that
the two writes made were the two writes recorded, bounded by the merge-commit
count and the two-parent proof.

The three newly exempted documents are now invisible to the `stale-code-path`
rule, including for any future drift. That is acceptable only because they are
frozen historical records; if any of them is ever un-frozen, the exemption must
be removed with it.
