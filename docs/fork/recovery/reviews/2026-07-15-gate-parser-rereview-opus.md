# Phase 0 Parser-Fix Series Re-Review (bounded)

Scope: verify resolution of prior IMPORTANT-1 (swallowed-error misattribution) via
commit separation + wording, final-content identity to prior-reviewed
`6d80a8e319060a0094118ee1f0a58694b50ae014`, trailers/order, clean tree, tests.
Read-only. No tracked files modified, no descendants spawned.

## Verdict: APPROVE — IMPORTANT-1 resolved, no new CRITICAL/IMPORTANT

The writer split the series so the swallowed-error baseline correction is now its
own commit with accurate, non-parser-attributing wording, while the parser commit
explicitly excludes swallowed changes. Final tree content is byte-identical to the
previously reviewed commit. All 17 tests pass, tree is clean, trailers/order correct.

## Evidence

### IMPORTANT-1 resolved (was: swallowed delta misattributed as parser-driven)
Commit separation now clean:
- `0bcb7ca49` "scripts: broaden cfg test production filter" — touches ONLY
  `scripts/panic_budget.json` (3 lines), `scripts/rust_production_filter.py`,
  `tests/test_rust_production_filter.py`. Body: "Correct the parser-semantic panic
  baseline ... dropping the test-only oauth.rs entries, changing panic total 34 to
  31. Swallowed-error budget changes are intentionally excluded from this
  parser-semantic commit." No swallowed JSON in the diffstat (confirmed). ✔
- `2456111b5` "scripts: tighten stale swallowed-error baseline" — touches ONLY
  `scripts/swallowed_error_budget.json` (6 lines). Body: "Restore the
  swallowed-error ratchet to the original f67 baseline counts: total 2988 to 2987,
  unwrap_or_default 782 to 781, ... todos_view.rs unwrap_or_default 4 to 3. This is
  a pre-existing stale-baseline correction at f67 under both the old and new
  classifiers. It is not a parser-semantic effect and is not a current-tree
  rebaseline." Wording matches prior report's factual finding exactly. ✔
- `c53022f4d` "ci: run rust production filter tests before gates" — ONLY
  `.github/workflows/ci.yml` (+4), `fork-ci.yml` (+3). ✔

The misattribution defect is gone: the swallowed change no longer claims a parser
cause and is physically decoupled from the parser commit. The prior report's own
recommended remedy ("split the stale-baseline refresh from the parser fix, or
acknowledge the bundled baseline refresh explicitly") is satisfied by the split.

### Final-content identity to 6d80a8e3 (all 8 files MATCH)
`git show HEAD:<f> | sha256sum` == `git show 6d80a8e3:<f> | sha256sum` for:
rust_production_filter.py, check_panic_budget.py, check_swallowed_error_budget.py,
panic_budget.json, swallowed_error_budget.json, test_rust_production_filter.py,
ci.yml, fork-ci.yml — all MATCH. The rewrite changed only commit topology/wording,
not resulting bytes; all prior functional/correctness evidence carries over. ✔

### Order / trailers / tree
- Order after unchanged `c3c3dd760`: `0bcb7ca49` -> `2456111b5` -> `c53022f4d`,
  matching the specified sequence. ✔
- All three carry `Co-authored-by: agent <agent@rudnik.online>`; author/committer
  `John Rudnik`. ✔
- `git status --porcelain` empty; `git diff --check c3c3dd760..HEAD` exit 0 (no
  whitespace/conflict markers). ✔
- Tests: `python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'`
  => Ran 17, OK. ✔

## Reused prior evidence (not re-run)
Parser correctness, policy preservation (`is_test_rust_file` verbatim), whole-tree
OLD-vs-NEW differential (only oauth.rs test-only `.expect` newly excluded), gate
fail-closed on current drift, baseline replay passing at f67, and no-production-
hiding — all validated in the prior review against content that is byte-identical
here, so they remain valid without repetition.

## No new findings
No new CRITICAL or IMPORTANT. The two prior open questions are now answered by the
commit wording itself (2456111b5 explicitly labels the swallowed change a
pre-existing stale-baseline correction under both classifiers, not a parser effect
and not a current-tree rebaseline).

## Confidence
High. Content identity is cryptographically confirmed; the sole prior blocker was a
wording/scope defect now directly addressed by the split and matching commit bodies.

## What was not checked (deliberately, bounded re-review)
- Did not re-run the full baseline replay / whole-tree differential (content
  identical to prior pass; would be redundant).
- Did not execute `--update` write path or GitHub Actions end-to-end (static YAML +
  step order relied on from prior review, content unchanged).
- Did not re-fuzz raw-string / cfg edge cases (parser bytes unchanged).
