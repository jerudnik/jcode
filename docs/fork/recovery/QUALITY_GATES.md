# Phase 0 quality-gate audit

This record fixes the interpretation of the fork quality gates before any
synchronization or remediation. Baseline tightening and parser corrections are
kept distinct from current-tree debt.

## Fixed refs and attribution slices

- Original ratchet baseline: `f67e7b45dddcbf4c4329f553b2df209dea2d5733`
- Pre-curated checkpoint: `64e29aefc`
- Curated-sync checkpoint: `6d96552cb`
- Recovery starting head: `6ca1fcf2ec2366c7abc99664a485c40d60cec80e`
- Upstream comparison: `802f6909825809e882d9c2d575b7e478dce57d3b`

The slices are attribution aids, not ownership conclusions. The curated-sync
slice can contain imported upstream behavior, conflict resolution, and fork
adaptations.

## Gate truth table

| Gate | Baseline | Recovery result | Verdict | Trust result |
|---|---:|---:|---|---|
| Warning budget | repository ratchet | `0` | green | Trusted shell gate. |
| Wildcard re-export budget | repository ratchet | `16` | green | Trusted path/count ratchet. |
| Dependency boundaries | workspace metadata | pass | green | Passed through the pinned `nix develop` shell. |
| Production size | per-file baseline | 60 violations; +6,604 net LOC across violating files | red | Failure reproduced and structurally credible. |
| Test size | per-file baseline | 31 violations; +3,679 net LOC across violating files | red | Failure reproduced and structurally credible. |
| Panic-prone usage | `31` | `46` | red by 15 | The duplicated classifier was invalid; the shared repaired classifier is independently approved. |
| Swallowed-error-like usage | `2,987` | `3,077` | red by 90 | The shared classifier is independently approved; the original ratchet was already stale by one. |

No command used `--update`.

## Parser-semantic correction

The original panic JSON was generated with a classifier that failed on
multiline test-item signatures. Evaluating the repaired classifier at the
original baseline changes only the panic ratchet:

- Panic: `34 -> 31`.
- Remove `crates/jcode-base/src/auth/oauth.rs: 3` because all three occurrences
  are inside test-only multiline functions.

The old parser with the corrected panic JSON fails at the original baseline,
which proves this correction is required by the parser semantics.

## Independent stale-baseline tightening

The swallowed-error JSON was already one above the source at the original
baseline under both the old and repaired classifiers. The separate ratchet
commit therefore tightens:

- Total: `2,988 -> 2,987`.
- `unwrap_or_default`: `782 -> 781`.
- `crates/jcode-tui/src/tui/app/todos_view.rs`: `4 -> 3`.

This is not a parser effect and is not a current-tree rebaseline. It restores
the ratchet to the value already present at `f67e7b45...`.

## Historical debt attribution

| Metric | Baseline | Pre-curated | Curated-sync | Recovery head | Net drift | Pre slice | Curated slice | Post slice |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Panic count | 31 | 35 | 45 | 46 | +15 | +4 (26.7%) | +10 (66.7%) | +1 (6.7%) |
| Swallowed count | 2,987 | 3,009 | 3,075 | 3,077 | +90 | +22 (24.4%) | +66 (73.3%) | +2 (2.2%) |
| Net LOC in current production-size violations | baseline | +120 | +5,866 | +6,604 | +6,604 | +120 (1.8%) | +5,746 (87.0%) | +738 (11.2%) |
| Net LOC in current test-size violations | baseline | +103 | +3,341 | +3,679 | +3,679 | +103 (2.8%) | +3,238 (88.0%) | +338 (9.2%) |

Fork-only pre/post slices account for 13.0% of production-size growth, 12.0% of
test-size growth, 33.3% of panic drift, and 26.7% of swallowed-error drift.
This does not make the curated slice authoritative upstream behavior. Seam
review must separate imported, composed, and fork-specific changes inside it.

## Repaired classifier contract

- Masks nested block comments, line comments, escaped strings and characters,
  raw strings, byte and C strings while preserving offsets and newlines.
- Excludes direct items whose `cfg(...)` logically requires `test`, including
  multiline attributes and bodies.
- Supports direct `mod`, `fn`, `impl`, `struct`, `enum`, `trait`, `type`,
  `const`, `static`, `use`, `extern`, `macro_rules!`, and `macro` definitions.
- Excludes files with a leading inner `#![cfg(...)]` that requires `test`.
- Retains `cfg(any(test, production-condition))`, `cfg(not(test))`, and
  `cfg_attr(...)` because they can compile in production.
- Conservatively counts arbitrary macro invocations and statement or expression
  attributes where a lightweight lexer cannot safely infer boundaries. This
  can overcount but must not undercount.
- Both gate scripts import the same implementation. Both quality workflows run
  the 17 adversarial unit tests before the two ratchets.

## Integrated commits and reviews

| Integrated commit | Isolated source | Purpose |
|---|---|---|
| `fb1168a6a` | `c3c3dd760` | Share the production/test classifier. |
| `0508e3f7b` | `0bcb7ca49` | Broaden direct-item handling and correct the parser-semantic panic baseline. |
| `0674fe53d` | `2456111b5` | Tighten the independently stale swallowed-error baseline. |
| `f9c70d1be` | `c53022f4d` | Run classifier tests in both quality workflows. |

The review sequence is preserved rather than collapsed:

- [Initial Opus review](reviews/2026-07-15-gate-parser-initial-opus.md)
- [Final Opus review that requested the attribution split](reviews/2026-07-15-gate-parser-final-opus.md)
- [Bounded Opus re-review that approved the split](reviews/2026-07-15-gate-parser-rereview-opus.md)

The final re-review found no critical or important issue and cryptographically
confirmed that the split changed commit topology and wording, not final bytes.

## Reproduction

```bash
python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'
python3 -m py_compile scripts/rust_production_filter.py \
  scripts/check_panic_budget.py scripts/check_swallowed_error_budget.py \
  tests/test_rust_production_filter.py
python3 scripts/check_panic_budget.py
python3 scripts/check_swallowed_error_budget.py
nix develop -c bash -c \
  '/Library/Developer/CommandLineTools/usr/bin/python3 \
   scripts/check_dependency_boundaries.py && cargo fmt --all -- --check'
```

The current-tree panic and swallowed-error commands must remain red. Overlaying
the repaired scripts and corrected JSON onto an archive of `f67e7b45...` makes
both gates pass exactly at panic `31` and swallowed-error `2,987`.
