# §4 maintenance window: PR #100 (D01-F09 docs gate enforcement)

Executed 2026-08-03. Recorded because §4 states that all writes and read-backs
in a maintenance window are evidence, and because this was the first use of the
procedure since activation.

## Why a window was needed

PR #100 wires `check_docs_references.py` into `Quality Guardrails` and adds two
paths to the protected set. Touching `.github/workflows/**` and the governance
artifacts turns `Governance Root` red **by design**: it is an audit gate that
"detects an unreviewed governance-path change on the PR that makes it rather
than preventing one" (its own header).

A legitimate governance change therefore cannot make that context green. It can
only be reviewed and merged deliberately. `gh pr merge --admin` does **not**
work: `protect-fork-rails` has `bypass_actors: []`, so the ruleset binds the
owner-admin too, exactly as `design.md` §345 says it should. The two prior
governance-path merges (#49, #92) both show `Governance Root: failure`.

Authorization for this window was given by the repository owner in session.

## Sequence executed

| # | step | result |
|---|---|---|
| 1 | capture pre-change ruleset body | `protect-fork-rails`, enforcement `active`, `bypass_actors []`, 4 required contexts |
| 2 | hash it, sanitized and pinned-encoder | `af472a78a61620aa80373f701c0302a37ea7d0f96f0a9f2a6ce36b1cf2ed3eb8` |
| 3 | record pre-window main tip | `ef302f5463ba6224aebde5c4bb70433a28452075` |
| 4 | PUT dropping **only** `Governance Root` | required contexts 4 -> 3, read back as `Fork CI Gate, Security Gate, Nix Gate` |
| 5 | SHA-conditioned merge | `sha=edfec25d3...`, `merged=true`, merge commit `1f02e06f2` |
| 6 | PUT restoring the exact pre-change body | read back `Governance Root, Fork CI Gate, Security Gate, Nix Gate` |
| 7 | body equality proof | pre and post governed-body SHA-256 both `efc2323f1689c34c2f25a8a0787cbd148224e4b085e63f54e666ec6c5a515cda` |
| 8 | exactly-one-merge-commit proof | 5 commits added, `git rev-list --count --merges` = **1** |
| 9 | two-parent proof | parent1 == pre-window tip, parent2 == reviewed head `edfec25d3` |
| 10 | post-window comparator | `=== Governance: snapshot matches the manifest ===`, 31 protected paths enforced |

Steps 7 and 9 are the ones that make the window bounded rather than merely
brief. Step 7 compares the governed body rather than the whole response,
because `updated_at` necessarily changes across a window; the sanitization and
encoder are the ones `design.md` pins.

## Window duration

Ruleset weakened `03:32:02-04:00`, restored `03:32:29-04:00`. Roughly 27
seconds, containing exactly one merge commit whose two parents are both
accounted for. No other write occurred in the window.

## What this window does not prove

`design.md` §114 is explicit that a change made and reverted *between* two
`--live` samples is not guaranteed to be caught by the audit boundary alone.
This record does not change that. It bounds this window by the merge-commit
count and the parent identities, which is the part §4 says closes the
reverted-during-window case, and it does not claim continuous coverage.
