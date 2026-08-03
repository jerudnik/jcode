# §4 maintenance window: PR #104 (agent prompt budget, CLAUDE.md drift)

Executed 2026-08-03, the third use of the procedure and the second on the same
day. Recorded because §4 makes every write and read-back in a window evidence.

## Why a window was needed

PR #104 edits `scripts/check_agent_instructions.py`, which became protected in
#100. `Governance Root` fails by design on a protected-path change: it detects
an unreviewed governance-path change on the PR that makes it, so a legitimate
change cannot make it green. `--admin` is refused because `protect-fork-rails`
has `bypass_actors: []`, which binds the owner-admin too.

Authorization was given by the repository owner in session, after the
pre-window state was posted on the PR and work stopped.

## Sequence executed

| # | step | result |
|---|---|---|
| 1 | confirm PR state | `mergeable=MERGEABLE state=BLOCKED`, sole failure `Governance Root` |
| 2 | capture pre-change ruleset | `protect-fork-rails`, `active`, `bypass_actors []`, 4 contexts |
| 3 | hash with the pinned encoder | `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b` |
| 4 | record pre-window main tip | `c228570aec0cc0806c76a5a68292879eb7d93e80` |
| 5 | pre-window `fork-health.sh --live` | all invariants hold |
| 6 | PUT dropping **only** `Governance Root` | 4 -> 3, read back `Fork CI Gate, Security Gate, Nix Gate` |
| 7 | SHA-conditioned merge to reviewed head | `merged=true`, merge commit `f544a45c3` |
| 8 | PUT restoring the exact pre-change body | read back all 4 contexts |
| 9 | body equality proof | pre == post == `43ba61a7a5…`, **RESTORED EXACTLY** |
| 10 | merge-commit bound | 1 merge commit created in the window |
| 11 | two-parent proof | parent1 == pre-window tip, parent2 == reviewed head `d886933cd` |
| 12 | post-window `fork-health.sh --live` | all invariants hold |

Window open `16:24:27Z`, closed `16:24:51Z`: 24 seconds.

## The hash is identical to the #101 window, and that is the expected result

This window's governed-body SHA-256 is `43ba61a7a5…`, byte-identical to the one
recorded for #101 earlier the same day. That is the correct outcome: nothing
between the two windows changed the protected set, so the governed body did not
change either.

The #101 record warned that the hash is **not** comparable across windows when
the protected set changes, using #100 (`efc2323f16…`) as the example. This
window is the other half of that rule: when the set does not change, the hash
does not change. Equality across windows is evidence only when the protected
set was untouched in between; inequality is evidence only when it was.

## The merge-count correction, tested against the case it was written for

The #101 record established that `git rev-list --count --merges <pre>..main`
over-reports when the reviewed head is itself a merge commit, and that the
correct bound adds `--not <reviewed-head>`.

This window is the control for that claim. PR #104's branch was linear, with no
merge in its own history, so both forms were run:

    git rev-list --count --merges <pre>..github/main              => 1
    git rev-list --merges --count <pre>..github/main --not <head> => 1

They agree here and disagreed in #101. That is what a correction should look
like: the fix changes the answer exactly in the case it was written for and
leaves every other case alone. Had both forms disagreed here too, the
explanation given in the #101 record would have been wrong.

## What landed

The prompt budget had 22 bytes free of 8192; it now has 759 (projected 7433,
compiled 7181). The bytes came from removing duplication, not policy: every
rule dropped from `main.instructions.md` was checked to still have an owner in
another prompt-loaded file.

The change also closes a silent gap. `CLAUDE.md` is compiled from the same
three root primitives as `AGENTS.md`, but the checker validated only
`AGENTS.md`, and `compiled_body` could not even parse `CLAUDE.md` because it
required a generated footer that `CLAUDE.md` does not carry. The drift was
found by causing it: editing the primitives left `CLAUDE.md` holding the
pre-edit text while the checker still reported ok.

Re-verified on merged main, checking the process exit status directly rather
than through a pipe, because the first attempt read `tail`'s status and
reported a meaningless `exit=0` next to a failure message:

    hand-edited CLAUDE.md -> exit 1, "CLAUDE.md is stale; run apm compile"
    restored              -> exit 0

## What this window does not prove

`design.md` §114 still applies: a change made and reverted between two `--live`
samples is not guaranteed to be caught by the audit boundary alone. This record
bounds the window by the merge-commit count and the parent identities and does
not claim continuous coverage.

Separately, the budget counts `.jcode/swarm-prompt.md`, but a `jcode serve`
process started outside the repository loads the built-in default instead,
because `load_swarm_prompt(None)` resolves `.` against the process working
directory. The budget figure is correct for a project-rooted agent and is not a
claim about every running session.

## Post-merge state

`main` = `f544a45c3`. `check_agent_instructions.py` ok at 7433/8192 projected
and 7181/8192 compiled, `docs-references` OK at 137 documents / 0 machine-local
/ 85 stale at baseline, and `d01_scoreboard.sh` TOTAL **0**.
