# F17 global-state test-pollution cleanup — design + worker spec

## Objective
Make `cargo test -p jcode-tui` pass in a **real parallel run** (default
thread count), not just in per-test isolation. Today: 38 target tests pass
singly, but ~5-7 fail under parallelism due to process-global state races.
This is the prerequisite to making the jcode-tui rail BLOCKING in
`.github/workflows/fork-ci.yml` (F17's actual gate). No half measures:
fix the root cause, do not paper over with `--test-threads=1` or ignores.

## Verified root cause (coordinator bisect)
- `tui::ui::tests::basic::` runs clean in parallel (77/0).
- `tui::app::tests::` fails in parallel (899 pass / 7 fail), same tests
  pass singly. So pollution is concentrated in `app::tests` and is
  **parallelism-driven** (thread interleaving), not mere sequential
  accumulation.
- Parallel-failing set (representative): test_agents_review_picker_saves_config_override,
  test_login_completed_surfaces_new_provider_models_in_local_model_picker,
  test_model_picker_includes_copilot_models_in_remote_mode,
  test_model_picker_remote_falls_back_to_current_model_when_catalog_empty,
  test_queued_file_activity_repaint_does_not_leave_trailing_digit_artifact.
- Mechanism: process-global **env vars** (`JCODE_HOME`, Azure/OpenAI-compat
  keys) mutated via `crate::env::set_var` + a shared auth/config cache.
  A partial serialization primitive exists: `crate::storage::lock_test_env()`
  (a global Mutex). But it is applied **inconsistently**:
  - Multiple `with_temp_jcode_home` definitions exist; some acquire
    `lock_test_env()` (auth_tests.rs) and some may not
    (support_failover/part_01.rs:356, remote_tests.rs).
  - Multiple `create_test_app` definitions (remote_tests.rs:40,
    support_failover/part_01.rs:178, ui_header.rs:1024) used ~732 times.
  - **Deep issue:** `lock_test_env()` only serializes *writers* against
    each other. A test that merely READS `JCODE_HOME`-derived config or
    the auth cache while NOT holding the lock still races with a
    concurrent writer. So readers must lock too.

## The fix (single-chokepoint, honest)
1. **Unify env scoping.** Make ONE canonical `with_temp_jcode_home` (and
   any `create_test_app` that reads config/auth) acquire
   `crate::storage::lock_test_env()` for the whole closure/app lifetime.
   De-duplicate the divergent copies to one shared test-support helper so
   there is a single source of truth. Prefer a `crates/jcode-tui/src/tui/
   app/test_support.rs` (or existing shared module) exporting:
   - `with_temp_jcode_home<T>(f) -> T` (locks, sets temp HOME, saves/
     restores the full env allowlist, clears auth/config caches on entry).
   - `create_test_app()` that internally calls the guarded path OR
     documents that callers must already hold the env lock.
2. **Readers lock too.** Any test that constructs an App and asserts on
   config/auth/model-picker contents must hold the env lock for the read,
   because a concurrent writer mutating global env corrupts the read.
   The cleanest way: route ALL such tests through the guarded
   `create_test_app`/`with_temp_jcode_home` so the lock is automatic.
3. **Reset caches on scope entry.** Where a global cache (auth cache,
   `LAST_USER_PROMPT_POSITIONS`, body/full-prep cache, frame metrics,
   render profile, theme_detect `DETECTED`) can leak across tests, call
   the existing `clear_*_for_tests()` helpers at the start of the guarded
   constructor. Consolidate the ~20 scattered clear helpers into one
   `reset_tui_test_globals()` called from the guarded entrypoint.
4. Do NOT use `--test-threads=1`, `#[serial]` crate, or `#[ignore]` as the
   fix. The lock + cache-reset discipline is the real fix and keeps the
   suite parallel/fast.

## Verification (mandatory, every worker)
Rebuild then run the FULL module in PARALLEL (default threads) repeatedly:
```
cargo test -p jcode-tui --no-run
B=$(ls -t target/debug/deps/jcode_tui-* | grep -v '\.d$' | head -1)
for i in 1 2 3; do T=$(mktemp -d); HOME=$T JCODE_HOME=$T $B "tui::app::tests::" 2>&1 | grep "test result"; done
```
Must be `ok` all 3 runs (proving the race is gone, not just lucky). Then
the whole crate in parallel must be green:
```
T=$(mktemp -d); HOME=$T JCODE_HOME=$T $B 2>&1 | grep "test result"
```

## Constraints
- Test-support / test code only where possible. If a product-code change
  is needed (e.g. exposing a `reset` on a cache), keep it minimal, gated
  `#[cfg(test)]` or `pub(crate)`, and FLAG it in the report.
- Do NOT commit. Leave changes in the worktree for coordinator review.
- Report: files changed, the unification you did, and paste the 3x
  parallel `test result: ok` proof.
