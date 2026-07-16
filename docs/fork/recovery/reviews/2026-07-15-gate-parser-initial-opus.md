# Independent Review: Phase 0 Quality-Gate Parser Fix

**Reviewer:** verify subagent (independent)
**Workspace:** `/private/tmp/jcode-recovery-gate-parser`
**Commit under review:** `c3c3dd76050a7dfa310fd91b5989dde96ea5bb1f` ("scripts: share Rust production filter")
**Parent:** `d756d6a2c`
**Branch:** `recovery/fix-gate-parser-2026-07-15`

## Verdict: APPROVED

The fix is correct, strengthens (does not weaken) the quality-gate policy, and is well-covered by tests. No CRITICAL or IMPORTANT findings. Two SUGGESTIONs and several documented out-of-scope observations below.

---

## What the change does

- Extracts the shared Rust production/test classifier into new `scripts/rust_production_filter.py`.
- Both callers (`check_panic_budget.py`, `check_swallowed_error_budget.py`) now `from rust_production_filter import production_lines, production_rust_files` and delete their duplicated copies.
- Adds `tests/test_rust_production_filter.py` (12 tests).
- Replaces the old naive line-oriented `brace_delta`/`CFG_TEST_RE` classifier with a masking-based classifier that: masks comments (incl. nested block comments), string/char/raw/byte literals to spaces (preserving byte offsets and newlines), then finds `#[cfg(...)]` outer attributes whose cfg-expression *requires* `test`, and blanks the entire attached `mod`/`fn` item by real brace/semicolon matching.

Diff stat: +531/-138 across 4 files.

---

## Evidence and validation commands

### 1. Existing test suite (12 tests) - PASS
```
$ python3 -m unittest tests.test_rust_production_filter -v
Ran 12 tests in 0.003s
OK
$ python3 -m unittest discover -s tests -p "test_rust_production_filter.py"
Ran 12 tests -> OK
```
Both the module path and unittest discovery work. `sys.path.insert(0, str(SCRIPTS_DIR))` in the test file makes `import check_panic_budget`, `import check_swallowed_error_budget`, and `from rust_production_filter import ...` resolve. The identity test `assertIs(check_panic_budget.production_lines, production_lines)` confirms both callers use the shared symbol.

### 2. Import behavior from direct script execution - PASS
```
$ python3 scripts/check_panic_budget.py         # from repo root: runs, imports OK
$ cd /tmp && python3 /private/tmp/.../scripts/check_swallowed_error_budget.py  # from foreign CWD: runs, imports OK
```
Python auto-inserts the script's own directory as `sys.path[0]`, so `from rust_production_filter import ...` resolves regardless of CWD. No `sys.path` manipulation needed in the scripts. Verified from two different working directories.

### 3. Both gate scripts re-run without --update - exit 1 (pre-existing baseline drift, out of scope)
```
$ python3 scripts/check_panic_budget.py           -> exit 1
$ python3 scripts/check_swallowed_error_budget.py  -> exit 1
```
The non-zero exit is **source drift in the recovery workspace**, not caused by the parser. Panic baseline (`scripts/panic_budget.json` "total") = 34. The **new** classifier counts 46 production panics; the **old** classifier counted 62. Both 46 and 62 exceed the 34 baseline, so the gate fails under either classifier. The parser change does not introduce the failure. This is out of scope for a parser fix.

### 4. Old vs new classifier comparison on the real repo (feasible, done)
Reconstructed the parent's classifier verbatim from `git show d756d6a2c:scripts/check_panic_budget.py` and ran both over `production_rust_files()`:
```
PANIC total  old=62  new=46  (delta -16)
  DIFF crates/jcode-base/src/auth/oauth.rs:        old=3 new=0
  DIFF crates/jcode-base/src/mcp/manager.rs:       old=9 new=0
  DIFF crates/jcode-base/src/terminal_launch.rs:   old=4 new=0
SWALLOW let_underscore     old=1124 new=1124  (0)
SWALLOW dot_ok             old=1139 new=1139  (0)
SWALLOW unwrap_or_default  old=814  new=814   (0)
```
All 16 fewer panic hits are in genuine test-only code that the **old** classifier failed to exclude. In each of the 3 files the affected items are `#[cfg(test)] fn build_claude_exchange_request(...)`, `#[cfg(all(test, unix))] mod tests { ... }`, etc., where the `fn(`/`mod` signature spans multiple lines so the brace does not appear on the item-start line. The old `brace_delta`-on-the-item-line logic saw delta 0, dropped only the signature line, and then **counted the test body panics as production**. The new classifier correctly excludes the whole item. This is a **correctness improvement that strengthens the gate's precision** (fewer false-positive production panics), not a weakening. The swallowed-error patterns are unchanged (0 delta), confirming no collateral movement.

### 5. Adversarial probes (ephemeral, /tmp only) - ALL PASS
Wrote four ephemeral probe scripts. Every category the task requested plus extras:

- **Nested block comments** with embedded `}`: `/* outer /* inner } */ still } */` inside `#[cfg(test)] mod` -> test dropped, production kept. PASS
- **Escaped strings/chars**: `"... \" ... \\ ... }"`, `'\''`, `'\\'` -> braces inside not counted, lifetimes not misparsed. PASS
- **Raw strings with hashes**: `r##"contains "}"# and more }"##` -> terminator matched correctly, brace not counted. PASS. Offset length preserved.
- **Lifetimes** `<'a, 'b: 'a>`, `&'a str` -> not treated as char literals; production kept. PASS
- **Multiline / nested cfg**: `#[cfg(all(\n test,\n unix\n))]` -> dropped. PASS
- **cfg(all(test, ...))** -> dropped. **cfg(any(test, ...))** production path -> **kept** (correct: any(test, feature) can compile outside test). **cfg(any(test))** (test-only) -> dropped. PASS
- **cfg(not(test))** -> **kept** (production). **cfg(all(not(test), unix))** -> **kept**. PASS
- **cfg_attr(test, ...)** and **cfg_attr(all(test, unix), ...)** -> **kept** (correct: cfg_attr conditionally applies an attribute, item still compiles in production). PASS
- **cfg_expr logic unit checks** (9 cases): `test`->T, `not(test)`->F, `all(test,unix)`->T, `any(test,x)`->F, `any(test)`->T, `all(not(test),unix)`->F, `all(unix,any(test))`->T, `any(all(test,unix),test)`->T, `any(all(test),foo)`->F. All correct.
- **cfg_attr file-level test path** `#[cfg(test)] #[path="foo_tests/mod.rs"] mod tests;` -> whole item (semicolon-terminated) dropped, production kept. PASS
- **Item declarations with brace on following line** (`mod tests\n{`, `fn helper()\n{`) -> dropped. PASS
- **Malformed/unbalanced**: unterminated block comment, unterminated string, `mod tests { fn t() {` with missing closes -> **no crash**, graceful. PASS
- **Extras**: `pub(crate) async fn`, `const unsafe fn`, `extern "C" fn`, byte string `b"}"`, byte char `b'}'`, `c"..."`, char literal `'}'`, closures `|x| { }`, generics `Vec<Box<dyn Fn()>>`, doc-comment containing `#[cfg(test)]` text, whitespace-heavy `#  [  cfg  (  test  )  ]`, `#[cfg(test)] impl` block, `#[cfg(test)] struct`, `#[cfg(test)] pub use`, two items on one line. All behave correctly/conservatively.
- **Byte-offset preservation** verified: `len(mask(s)) == len(s)` for raw/escaped/nested-comment/byte/char inputs (critical because masked ranges are applied back onto the original source in `production_lines_from_text`).

### 6. Policy preservation
`test_test_rust_file_policy_is_preserved` plus the `is_test_rust_file` code is byte-identical in behavior to the parent (same `tests/` dir, `_tests`/`_test`/`tests_` suffix/prefix rules, `tests.rs`). Confirmed `production_rust_files()` still enumerates `src` + `crates` roots. File-classification policy is unchanged.

---

## Findings

### CRITICAL: none
### IMPORTANT: none

### SUGGESTION 1 (non-blocking): `#[cfg(test)] impl` / `struct` / macro-item bodies are not excluded
`ITEM_START_RE` matches only `mod`/`fn`. A `#[cfg(test)] impl Foo { fn helper() { panic!() } }` block's inner panics would still be counted as production. This is **conservative (over-counts, never under-counts)** and **matches the old classifier's mod/fn-only scope**, so it is not a regression and cannot weaken the gate. Worth a code comment or a future extension to `impl`/`trait` if test-only impls with panics appear. Not blocking.

### SUGGESTION 2 (non-blocking): shared-module coupling to CWD-independent import
The scripts rely on Python's implicit `sys.path[0] = script dir`. This works for `python3 scripts/foo.py` (how CI invokes them, verified in `.github/workflows/{ci,fork-ci}.yml`) and for the test's explicit path insert. It would break only if a caller did `python3 -m scripts.check_panic_budget` from repo root without a package `__init__`. No such invocation exists in the repo. A one-line comment noting the sibling-import assumption would help future maintainers. Not blocking.

---

## Out-of-scope / pre-existing (assessed, not fixed)

- **Non-executable script mode**: `scripts/*.py` are `100644` (no +x) despite shebangs. Confirmed via `git ls-tree` that the parent `d756d6a2c` scripts were **also `100644`**, and the new file follows suit. CI invokes them as `python3 scripts/...` (`.github/workflows/ci.yml:71,75`, `fork-ci.yml:205,208`), so the missing bit is irrelevant. **Pre-existing, out of scope.**
- **Gate exit-1 baseline drift**: baselines (total 34) are stale relative to the recovery workspace's Rust source (46 real production panics). Not a parser concern. **Out of scope.**

---

## What I did NOT check
- Did not build or run the Rust crates; the gate scripts only lex `.rs` text, so Rust compilation is irrelevant to the classifier's behavior.
- Did not exhaustively enumerate every `.rs` file's per-item classification beyond the aggregate old-vs-new diff (which surfaced exactly the 3 changed files and I inspected all 3).
- Did not test on non-UTF8 files (reader uses `errors="ignore"`, matching parent behavior).
- Did not run the coordinator worktree or read its files (per instructions).
- Did not assess `production_lines` performance on the full tree beyond the ~38s comparison run (acceptable for a gate).

---

## Confidence
**High.** The fix is a clear, well-tested improvement. The 12 shipped tests pass under both direct and discovery invocation; 30+ independent adversarial probes (including every requested category and malformed input) pass; the old-vs-new comparison proves the only behavioral delta is the correct exclusion of previously-miscounted test-only items in 3 files, with swallowed-error patterns unchanged. Policy is preserved and made stricter (more precise), never weakened. Production-capable cfg items (`not(test)`, `any(test, other)`, `cfg_attr`, `all(not(test),...)`) remain counted.

**Recommendation: APPROVED.**
