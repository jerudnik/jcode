---
title: "Five-model council verdict: retire the stale-code-path prose regex, converge on link citations"
status: open
priority: medium
owner: maintainers
opened: 2026-08-28
related:
  - scripts/check_docs_references.py
  - scripts/measure_code_path_drift.py
  - scripts/test_docs_references.py
---

# The stale-code-path check polices the one dialect that has no rot

A five-model council (Opus-5, Kimi-K3, GLM-5.3, Grok-4.6, GPT-5.6 Sol)
independently reviewed the stale-code-path rule in
`scripts/check_docs_references.py` after this session's near-miss: a
decision-log entry citing a moved script was caught only because the
citation happened to be backticked with one of the four blessed prefixes.
Each member read the checker, measured the citation corpus, and delivered a
verdict. Four of five said the rule should not survive in its current form
(one KILL, two REPLACE, one FIX-then-converge); one said KEEP and fix the
selector. The measurements agree even where the verdicts differ.

## Agreed findings

- The rule is vacuous today. Every citation its regex can see is live
  (287 matches, 0 stale, baseline 0), so it runs on every PR with a
  measured true-positive rate of zero. This session's catch landed inside
  its window by luck.
- The rot it was built for is in its blind spot. The `CODE_PATH` regex
  (line 88) requires backticks, a `crates/|src/|scripts/|tests/` prefix,
  and a `.rs|.py|.sh|.nix` extension. Estimates of stale citations
  invisible to it ranged from ~40 (strict resolution, repo-anchored
  spans only) to ~180 (including crate-relative forms like
  `ambient/runner.rs`, the exact dialect the modularization produced) to
  ~390 (all citation shapes). `docs/AMBIENT_MODE.md` alone carries ~27.
  Pre-split `src/tui/...` citations, the rule's founding defect class
  (D01-F12), dodge it today in `docs/SOFT_INTERRUPT.md` and
  `docs/DESKTOP_CODEBASE_ARCHITECTURE.md`.
- Widening into unbackticked prose is infeasible. The original authors
  measured 847/3451 hits and the council's independent counts confirm the
  noise dominance. Nobody recommends it.
- The checker's own docstring has rotted: it asserts "25 such references
  exist today" and a 317-citation seeded baseline, while the shipped
  baseline reads zero. The freshness checker cannot see its own staleness
  because prose is not gated.
- `scripts/measure_code_path_drift.py` duplicates the same regex,
  already disagrees with the gate (84 vs 86 per its own comment), and
  keeps an exemption model the gate abolished. One policy, two divergent
  implementations.
- A wiring gap: `scripts/test_docs_references.py` is not collected by any
  test runner (`just test-python` globs only `tests/test_*.py`); it is
  exercised only through the non-vacuity plant. Already counted in the
  wire-or-retire audit issue.
- Removing the rule is cheap: `check_guard_nonvacuity.py` plants a
  broken-link finding, not a stale-code-path one, so the non-vacuity
  proof does not depend on it.

## The disagreement, stated fairly

Kimi's KEEP-and-fix position: a backtick span is already a citation
syntax by convention, the mechanism (exact match against `git ls-files`,
one-way ratchet, plant-tested) is the best-engineered part, and the rule
demonstrably repaired 317 seeded citations when introduced. GLM's version
of the same: drop the prefix allowlist, resolve candidates three ways
(exact, citing-file-relative, unique `crates/` suffix), seed at the
measured count. The counterargument, from Opus and Sol: any widened
baseline immediately absorbs hundreds of stale-but-accurate citations in
frozen records (GOVERNANCE_DECISIONS.md, dated audits), recreating the
rule-collision recorded at `docs/architecture/GOVERNANCE_DECISIONS.md:1694`,
and a per-file count ratchet loses identity, letting one stale citation
replace another at the same count.

## Recommended design (council synthesis)

End the prose-regex pattern; keep the guarantee, on ground where it can
be exact:

1. Convention: a repository path a doc wants kept true is written as a
   repo-relative markdown link whose text is the path itself.
   The existing broken-link rule already resolves links exactly, is
   already fatal, uses a parser rather than a prose regex, and needs no
   baseline. This is strictly stronger than the rule being retired.
2. Inversion, from Opus: add a diff-scoped reverse check at move time.
   When a PR renames or deletes a file, grep tracked docs for the old
   path and basename, and fail with the citing lines. This turns the hard
   problem (parse unknown prose for path-shaped tokens) into the easy one
   (exact search for a string known to have existed), covers every
   citation syntax including bare filenames, and needs no baseline or
   allowlist.
3. Retire the stale-code-path rule, its ratchet key in
   `scripts/docs_references_budget.json`, and
   `scripts/measure_code_path_drift.py` together, recording the accepted
   loss: prose citations that were already stale before the change land
   stay invisible until the file moves again. Fix the rotted docstring
   and the stale 847/3451 evidence comment in the tests in the same
   change.
4. Backticked prose citations remain legal and unchecked. They are
   prose, and prose rot in frozen records is history, not a defect.

The un-adopted alternative (widen the regex, triple-resolve, seed at the
measured count) stays viable if the reverse check proves too coarse;
GLM's report holds the concrete recipe.
