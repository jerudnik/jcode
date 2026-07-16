# Phase 0 Parser-Fix Series Review

Scope: `recovery/fix-gate-parser-2026-07-15`, commits
`c3c3dd76..6d80a8e3` on parent `d756d6a2`.
Files: `scripts/rust_production_filter.py` (new), `scripts/check_panic_budget.py`,
`scripts/check_swallowed_error_budget.py`, `scripts/panic_budget.json`,
`scripts/swallowed_error_budget.json`, `tests/test_rust_production_filter.py` (new),
`.github/workflows/ci.yml`, `.github/workflows/fork-ci.yml`.

## Verdict: CHANGES REQUESTED (one IMPORTANT finding)

The parser rewrite is correct, conservative, well-tested, and CI-wired. All
functional checks pass: 17 tests green, both gates fail on the drifted current
tree (real regressions, no crash), and the corrected gate passes cleanly at the
original baseline `f67e7b45`. The single blocker is a scope/attribution defect:
the swallowed-error JSON delta is a stale-baseline correction, not a
parser-semantics correction, contradicting the commit message and the stated
"parser-only" scope.

## Findings

### IMPORTANT-1: Swallowed-error JSON delta is not parser-driven (misattributed)
Evidence:
- `swallowed_error_budget.json`: `total 2988->2987`, `unwrap_or_default 782->781`,
  `todos_view.rs unwrap_or_default 4->3`.
- Commit `5b969547e` body: "Correct the original-baseline budget JSON entries
  invalidated **solely by the parser semantics**."
- Both OLD and NEW parsers count `unwrap_or_default = 781` on the entire
  `f67e7b45` production tree, and `todos_view.rs = 3` under both parsers
  (baseline source at f67e7b45 has only 3 `unwrap_or_default`, all above the
  single `#[cfg(test)]` at line 562; none excluded by either parser).
- Running OLD parser + baseline(f67) JSON at the f67 tree reports the swallowed
  gate as "improved" (2988->2987), i.e. the baseline was already stale before
  any parser change.
Conclusion: the swallowed JSON change is a correct stale-baseline fix but is NOT
caused by parser semantics. It exceeds the requested "parser-only correction"
and the commit rationale is inaccurate for this entry. The change itself is safe
and the gate passes either way; the defect is scope/attribution, not correctness.
Recommended: amend commit wording (or split the stale-baseline refresh from the
parser fix), or acknowledge the bundled baseline refresh explicitly.

### Confirmed correct (no finding)
- Panic JSON delta IS parser-driven: baseline had `oauth.rs = 3`
  (`crates/jcode-base/src/auth/oauth.rs:1350,1371,1381`, `.expect(` inside
  `#[cfg(test)] fn build_claude_*` items whose signatures span multiple lines so
  the OLD line-based `ITEM_START_RE`/`brace_delta` parser failed to exclude them).
  NEW parser correctly yields 0; JSON drops the oauth entry, `total 34->31`.
  OLD parser + NEW JSON at baseline FAILS (31->34), proving the fix was required.
- Full-tree adversarial diff (OLD vs NEW classifier over every production `.rs`
  at f67e7b45, all four patterns): the ONLY file where NEW excludes more than OLD
  is `oauth.rs` (panic 3->0). No other production code is hidden. No production
  hiding introduced beyond the one legitimate cfg(test) case.

## Policy / correctness checks (all pass)
- `is_test_rust_file` policy preserved byte-for-byte (moved verbatim; test
  `test_test_rust_file_policy_is_preserved`).
- Direct item boundaries handled: mod/fn/impl/struct/enum/trait/type/const/
  static/use/extern/macro/macro_rules, braced and `;`-terminated, brace-on-next-
  line, where-clauses, generics with `<...>`, `[0;3]` array semicolons. Verified
  no over-run into trailing production code (custom adversarial cases all keep
  the following production item).
- Conservative limitations verified as documented, not undercounting: cfg(test)
  on macro invocations (`lazy_static!{...}`, `thread_local!{...}`), on non-item
  statements (`let`), and statement/expression attributes are all retained
  (kept in production), never silently dropped.
- cfg logic: `cfg(test)`, `all(...)` requires-test if any arg, `any(...)` requires
  all args test, `not(...)`=>False, nested any/all correct, `cfg_attr(test,...)`
  =>False (not excluded), `cfg(feature="test")` NOT excluded, whitespace/multiline
  cfg normalized.
- Leading inner `#![cfg(test)]` (incl. after leading `//!`/`#![allow]`) blanks the
  whole file; non-leading `#![cfg(test)]` does not; `#![cfg(any(test,...))]`
  retained.
- Masking: line comments, nested block comments, strings, byte/char/C strings,
  raw and raw-byte strings (`r#"}"#`, `br#"}"#`), lifetimes not treated as char
  literals, `#` inside strings not treated as attributes. Byte offsets/newlines
  preserved so ranges reapply to original source.
- Malformed input: unterminated cfg(test) item at EOF blanks to EOF without crash;
  unbalanced attributes fall through safely.

## Production panic / swallowed-error hiding challenge
No new logic can hide a production panic/swallowed match except via a genuine
`#[cfg(test)]`-gated direct item. The whole-tree OLD-vs-NEW diff confirms the only
newly-excluded production matches are the three test-only `oauth.rs` `.expect(`
calls. Counting uses `pattern.search(line)` (max one hit per line) in the gate
scripts; a raw `findall` scan diverges on dot_ok (1101 vs 1089) but both parsers
agree line-for-line, so this is expected and not a defect.

## Commands / evidence
- Tests: `python3 -m unittest discover -s tests -p 'test_rust_production_filter.py' -v`
  => 17 passed (Python 3.9.6). `py_compile` of all four Python files OK.
- Current tree (no --update): panic gate exit 1 (31->46, real drift),
  swallowed gate exit 1 (2987->3077, real drift). Correct fail-closed behavior.
- Baseline replay: `git archive f67e7b45 src crates` overlaid with HEAD
  scripts+JSON => panic "OK total=31 files=12" (exit 0), swallowed
  "OK total=2987 ... unwrap_or_default 781" (exit 0).
- `git diff --check` per commit and across range: clean (exit 0).
- Trailers: all three commits carry `Co-authored-by: agent <agent@rudnik.online>`;
  author/committer `John Rudnik`.
- YAML: `yq -e '.jobs'` valid for both workflows. Step order confirmed: "Run Rust
  production filter tests" precedes both budget gates in `ci.yml` job `quality`
  (steps 11 < 12,13) and in `fork-ci.yml` (step 11 < 12,13).
- Import: budget scripts do sibling `from rust_production_filter import ...`;
  works because `python3 scripts/x.py` puts `scripts/` on sys.path (comment
  documents this); tests inject `scripts/` explicitly. `test_..._use_the_shared_classifier`
  asserts identity binding.
- Working tree left clean (`git status --porcelain` empty); no tracked files
  modified, no commits, no coordinator worktree touched, no descendants spawned.

## Open questions
- Should the stale swallowed baseline refresh (IMPORTANT-1) be split out or the
  commit message corrected? The coordinator should decide whether bundling a
  non-parser baseline refresh into this "parser-only" series is acceptable.
- `todos_view.rs` baseline of 4 predates this branch (set at f67e7b45); confirm
  upstream source drift, not a prior parser error, is the intended explanation.

## Confidence
High on parser correctness, policy preservation, gate/baseline behavior, tests,
CI wiring, and no-production-hiding (whole-tree differential evidence).
High that IMPORTANT-1 is real (both parsers reproduce 781/3 at baseline).

## What I did not check
- Full end-to-end GitHub Actions run (validated YAML + step order statically only).
- Behavior on non-UTF-8 files (scripts use errors="ignore"; not exercised).
- Exhaustive fuzzing of raw-string hash-count edge cases beyond the tested set.
- `--update` write path was not executed (read-only review; would mutate JSON).
- Windows path handling of `is_test_rust_file` (posix-normalized; not run on Windows).
