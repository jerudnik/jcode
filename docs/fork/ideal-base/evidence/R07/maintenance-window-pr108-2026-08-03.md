# Maintenance window: PR #108, 2026-08-03

Fifth execution of the §4 ruleset maintenance procedure recorded in
`docs/fork/ideal-base/evidence/R07/design.md`.

PR #108 removes `CODE_PATH_EXEMPT` from `scripts/check_docs_references.py`.
Both that file and `scripts/test_docs_references.py` are governance-protected,
so `Governance Root` fails by design and no change to the PR can clear it. The
window is the only path.

Authorization was given by the repository owner in session, after the
pre-window state was posted and work stopped.

## Sequence executed

| # | step | result |
|---|---|---|
| 1 | capture identity before any write | head `56da865052df…`, base `4f2add9079fe…`, `mergeable=True state=blocked` |
| 1 | `Governance Root` fails on exactly `head_sha` | `failure`, `head_sha` match confirmed |
| 1 | independent diff review at `head_sha` | 5 files, exactly 2 protected: the two `docs_references` scripts |
| 2 | capture pre-change ruleset | `protect-fork-rails`, `active`, `bypass_actors []`, 4 contexts |
| 2 | hash under the repo's pinned `sanitize`/`canonical` | `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b` |
| 2 | pre-window `fork-health.sh --live` | all invariants hold |
| 3 | PUT dropping **only** `Governance Root` | 4 -> 3, read back `Fork CI Gate, Security Gate, Nix Gate` |
| 4 | SHA-conditioned merge to the reviewed head | `merged=true`, merge commit `487221f0ed17…` |
| 5 | main tip == `merge_sha` | true |
| 5 | ordered two-parent proof | `[4f2add9079fe…, 56da865052df…]` == `[base, head]` |
| 6 | PUT restoring the literal pre-change body | read back all 4 contexts |
| 6 | body equality, hashed from a **fresh GET** | pre == post == `43ba61a7a5…`, **RESTORED EXACTLY** |
| 7 | `post_restore_main_sha == merge_sha` | true |
| 7 | `rev-list --first-parent --merges base..post` | exactly 1 entry, equal to `merge_sha` |
| 8 | post-merge comparator | all invariants hold |
| 8 | pre-window comparator at `4f2add907` | all invariants hold |

Window open `20:14:48Z`, closed `20:14:54Z`: **6 seconds**.

Step 6 hashes a fresh GET, never the PUT response. A PUT response is the server
repeating what it was asked to store; only an independent read shows what is
actually stored.

## Two errors I made in this window, both caught before they mattered

**1. I reported a head SHA I never read.** The pre-window state I posted named
the reviewed head as `d16c9a0b0`. Step 1 read the actual head as
`56da865052df…`, and `git cat-file -t d16c9a0b0` returns *not a valid object
name*: the SHA I published never existed in any repository. It was fabricated,
not misread. The correct head has been the branch tip since the push, is what
CI ran, and is what step 1's diff review covered, so the window merged exactly
what was reviewed. But had I trusted my own report instead of reading the API,
the SHA-conditioned merge in step 4 would have failed closed against a
nonexistent object, which is the protection working. The lesson is narrower
than "be careful": an identity capture step exists precisely because prose
about a SHA is not a SHA.

**2. I hashed with a hand-rolled sanitizer and got a different answer.** My
first step-2 hash was `7805298476…`, not the `43ba61a7a5…` recorded in the
#101, #104 and #106 windows. Rather than accept a hash that disagreed with
three prior windows, I traced it: my sanitizer dropped only `url` and
`contexts_url`, while the repo's pinned `VOLATILE_KEYS` in
`scripts/governance_compare.py` drops eleven keys including `updated_at`. A
full-response hash could never satisfy pre == post, since `updated_at` changes
on every PUT. Re-hashing by importing the repo's own `sanitize`/`canonical`
reproduced `43ba61a7a5…` exactly. This is the same error the #101 window
already recorded and discarded a hash for; it recurred because I reimplemented
instead of importing. Every hash in the table above comes from the pinned
functions.

## The hash is unchanged, and that remains the expected result

`43ba61a7a5…` is byte-identical to the #101, #104 and #106 windows. This PR
changes the *contents* of two already-protected files; it does not change the
protected *set*. Equality is evidence only because the set was untouched, which
is why #100 legitimately differs (`efc2323f16…`): #100 changed the set itself.

## Merge-count forms

Both `--first-parent --merges` and the plain `--merges` form returned 1. They
agree here because the reviewed branch is linear. They disagree only when the
reviewed head is itself a merge, as in #101, which is why the procedure pins
`--first-parent`: it measures merges onto `main`, not merges that legitimately
exist inside the reviewed branch.

## Post-merge verification on main

`CODE_PATH_EXEMPT` is absent from merged `main` (0 occurrences). The checker
exits 0 at `137 active documents, 0 machine-local, 317 stale-code-path at
baseline`, and all 34 tests pass.

The control was re-run on merged `main` reading the process exit status
directly, never through a pipe: planting a stale citation in
`docs/fork/patch-ledger.md`, one of the six formerly exempt files, exits 1 with
`3 reference(s), baseline allows 2`; restoring exits 0; the tree is clean.
Under the old exemption that identical citation was silent.

## What this window does not claim

§114's revert-between-samples gap is unchanged: a change made and reverted
between two `--live` samples would not be caught, so this record does not claim
continuous coverage of the interval, only that the bounded transaction opened,
merged exactly the reviewed head, and closed restored.
