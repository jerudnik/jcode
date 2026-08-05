# §4 maintenance window: PR #130 (critical-path file-count refresh)

Executed 2026-08-05, the seventh use of the procedure under the ordinal series
that restarted at PR #100, and the fourteenth recorded window overall. Recorded
because §4 makes every write and read-back in a window evidence.

## Why a window was needed

PR #130 edits `scripts/check_critical_path_budget.py` and
`.github/workflows/fork-ci.yml`, both on the protected list. `Governance Root`
fails by design on a protected-path change: it detects an unreviewed
governance-path change on the PR that makes it, so a legitimate change cannot
make it green. `--admin` is refused because `protect-fork-rails` has
`bypass_actors: []`, which binds the owner-admin too.

This is the same protected pair as the PR #68 window, and for the same
structural reason: the file-count pin and the digest that seals it live on
opposite sides of the protected boundary, so refreshing one always requires
refreshing the other.

The gate trip was predicted before pushing, not discovered from CI:

    git diff --quiet <merge-base> HEAD -- .github/workflows scripts/check_critical_path_budget.py

returned 0 for the committed state and 1 once the two edits were included. CI
then named exactly those two files and nothing belonging to anybody else's work,
which was read from the failure log rather than assumed.

Authorization was given by the repository owner in session, after the failure
was diagnosed and the pre-window state was captured.

## A pre-flight check that failed, and had to

Before opening, the restore body was hashed offline and compared to the
pre-window governed hash. It did **not** match:

    pre-window governed hash : 43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b
    restore body hash        : 6f700457b6246b82276da6effcd9553adf7d0f0f164a8d79110580863a693b8a  MISMATCH

The cause was a real defect in the restore body, not in the hash: the PUT body
had been built from five keys and omitted `bypass_actors`. Restoring it would
have silently dropped the empty bypass list, the very field that makes the
ruleset bind the owner-admin. Diffing the sanitized key sets located it exactly:

    sanitized GET keys     : bypass_actors, conditions, enforcement, name, rules, target
    sanitized RESTORE keys : conditions, enforcement, name, rules, target
    missing from restore   : bypass_actors

After adding the key, the sanitized bodies were compared field by field rather
than by hash alone, since the GET and PUT shapes are not required to be
identical documents:

    key sets equal (restore)  : True
    differing values (restore): NONE  -> restore is exact
    differing values (drop)   : ['rules']  (must be exactly this, and is)

This is the check earning its place. Every prior window proved restoration
*after* the fact, when a bad body would already have been live. Proving the
restore body offline moves the failure to before the window opens, where it
costs nothing.

## Sequence executed

| # | step | result |
|---|---|---|
| 1 | confirm PR state | `mergeable=MERGEABLE state=BLOCKED`, sole failure `Governance Root` |
| 2 | capture pre-change ruleset | `protect-fork-rails` (id 18509013), `active`, `bypass_actors []`, 4 contexts |
| 3 | hash with the pinned encoder | `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b` |
| 4 | record pre-window main tip | `49786d646241f1d2bca17fcdf0eb0d2ae87b81e0` |
| 5 | pre-window `fork-health.sh --live` | all invariants hold, exit 0 |
| 6 | PUT dropping **only** `Governance Root` | 4 -> 3, read back `Fork CI Gate, Security Gate, Nix Gate` |
| 7 | SHA-conditioned merge to reviewed head | exit 0, merge commit `f13f08ff3` |
| 8 | PUT restoring the exact pre-change body | read back all 4 contexts, `active`, `bypass_actors []` |
| 9 | body equality proof | pre == post == `43ba61a7a5…`, **RESTORED EXACTLY** |
| 10 | merge-commit bound | 1 merge commit created in the window |
| 11 | two-parent proof | parent1 == pre-window tip, parent2 == reviewed head `4c16d8ab4` |
| 12 | post-window `fork-health.sh --live` | all invariants hold, exit 0 |

Window open `21:08:06Z`, closed `21:08:11Z`: **5 seconds**, the shortest of the
series. The script is `set -uo pipefail` rather than `-e` specifically so that
step 8 still runs if step 7 fails.

The merge was verified independently of the script, because step 7's own
verification query was malformed: it requested a `merged` field that
`gh pr view` does not expose, and printed a field list instead of a result.
That output is a broken query, not a broken merge. The merge was confirmed
separately:

    state=MERGED mergedAt=2026-08-05T21:08:10Z mergeCommit=f13f08ff3f11…

## The hash matches the four prior windows, which is the expected result

This window's governed-body SHA-256 is `43ba61a7a5…`, byte-identical to #101,
#104 and #119. Nothing between those windows and this one changed the protected
set, so the governed body did not change either.

The rule those records established holds in both directions: equality across
windows is evidence only when the protected set was untouched in between, and
inequality is evidence only when it was. #100's `efc2323f16…` differs precisely
because that window changed the set.

The hash was NOT computed with a hand-rolled sanitizer. It uses the repository's
own `scripts/governance_compare.py` `sanitize()` and `canonical()`, imported
rather than reimplemented. The ten volatile keys it drops are recorded here so
the number is reproducible: `_links, contexts_url, created_at,
current_user_can_bypass, id, node_id, source, source_type, updated_at, url`.

## Merge-count bound

Both forms were run, per the correction established in #101:

    git rev-list --count --merges <pre>..github/main              => 1
    git rev-list --count --merges <pre>..github/main --not <head> => 1

They agree here, as PR #130's branch was linear. A correction should change the
answer exactly in the case it was written for and leave every other case alone.

## What landed, and why it needed the window at all

`scripts/test_critical_path_budget.py` was failing in Quality Guardrails: the
`provider_infrastructure` file count in `EXPECTED_FILE_COUNTS` still read 19
while the tree held 20. The added file is exactly one,
`crates/jcode-provider-core/src/rate_limit_headers.rs`, extracted during
D01-FIX-3; none were dropped. The domain has four roots
(`jcode-provider-core/`, `jcode-provider-env/`, `jcode-provider-metadata/`,
`jcode-auth-types/`), so the count is not provider-core alone.

The enforcement gate stayed green while the self-test failed, because
enforcement compares ceilings and the digest, and the digest seals the pinned
values as configuration rather than checking them against the tree. Only the
self-test compares the pin to reality. This is why preflight passing is not
evidence that CI will pass.

This is an inventory refresh, not a debt re-baseline. No ceiling, target, or
high-water mark moved; headroom remains 2 on lifecycle and 2 on tui. The control
distinguishing the two was run by temporarily mutating the module global and
restoring it: with the tree at 20 and one real in-scope provider file simulated
away (20 -> 19), the stale pin of 19 was **NOT FLAGGED**, and the corrected pin
of 20 **WAS FLAGGED**. The pin therefore detects scope drift in both directions
rather than merely accommodating it.

Relocating the new file out of the critical set to dodge the protected paths was
considered and rejected: that is the antipattern the checker's own message names,
"Debt that leaves the critical set is not debt that was fixed". Folding it back
into `lib.rs` was rejected by measurement (1328 + 456 exceeds the 1632 code-size
baseline).

Verified on merged main, with exit status read directly rather than through a
pipe: enforcement with the refreshed digest
`6da4a35be70e6a8c842a1115260dd87dc23402aed698f1bca3139ce7c5ecd195` exits 0 with
all five domains `(OK)` and `provider_infrastructure current=20 expected=20`,
and the checker self-tests exit 0.
