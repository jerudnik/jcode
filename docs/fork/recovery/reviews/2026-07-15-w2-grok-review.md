# W2 R05B spawn/reclaim safety independent review

Verdict: **FAIL**

Reviewed worktree: `/Users/jrudnik/labs/jcode-w2-r05b`
Fixed base: `602709895be96a85a6090690c0b27d5681d17321`
Fixed HEAD: `2f4dfd7d2ff1e08cd18a6ea34f06f3be171719b1`
Branch observed: `recovery/fix-r05b-spawn-reclaim-2026-07-15`
Review mode: read-mostly/offline. I wrote no tracked repository files. Build artifacts were redirected with `CARGO_TARGET_DIR=/tmp/jcode-w2-grok-target` for the focused test run.

## Executive conclusion

The core source changes mostly implement the intended W2 behavior, and all focused tests I reran passed. I still return **FAIL** because the authoritative R05B fixture contract is stricter than the proof provided in two places:

1. **Auto fallback observability is not proven on the full response/event path and is not present in the spawn response.** The code labels/member-details the fallback, but `CommSpawnResponse` still carries only `new_session_id` and `initial_prompt_delivered`. The new test exercises only `resolve_swarm_spawn_creation`, not `handle_comm_spawn` or `handle_comm_assign_next` response/event observability. Also, for headless Auto fallback with an initial prompt, the fallback detail set on the member can be overwritten by the later `update_member_status(... detail = initial_msg ...)` task.
2. **The churn-to-abort fixture is helper-level, not a full configured-concurrency `run_plan` churn sequence with an asserted residue policy.** The guard formula is plausible and tested, but the required fixture said to drive a full sequence and assert total created sessions plus retention/cleanup expectation. The implementation aborts with an inspection message and does not explicitly cleanup or state a retention policy in the returned error.

If the acceptance bar is narrowed to "source behavior is plausibly safe and focused helper/unit tests pass," this would be close to PASS. Against RECOVERY_PLAN W2 and the R05B exact fixture language, it is not fully proven.

## Severity-ranked findings

### High: Fixture 1 Auto fallback response/event observability is incomplete

Evidence:

- Exact R05B fixture text requires Auto fallback to be visible in event/detail/response: `docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md:90-93`.
- The helper resolves Auto fallback into `Headless { fallback_detail: Some(...) }`: `crates/jcode-app-core/src/server/comm_session.rs:436-448`.
- The only Auto fallback test checks the helper and label construction, not full request/response/event behavior: `crates/jcode-app-core/src/server/comm_session_tests.rs:690-709`.
- Full spawn response maps successful spawn to `PersistedSwarmMutationResponse::Spawn { new_session_id, initial_prompt_delivered }` only: `crates/jcode-app-core/src/server/comm_session.rs:996-1003`. I found no response field carrying requested mode, resolved mode, fallback detail, or original visible failure.
- Member detail is set to the fallback detail at `crates/jcode-app-core/src/server/comm_session.rs:778-787`, but if a startup message exists on a headless fallback, a spawned task calls `update_member_status(... Some(truncate_detail(&initial_msg, 120)) ...)` at `crates/jcode-app-core/src/server/comm_session.rs:810-847`, which can replace that detail. The label still contains fallback text, so this is not a total observability loss, but it refutes the stronger detail/response proof.

Impact:

- Explicit `Visible` fail-closed behavior itself looks correct.
- Auto fallback is observable via label/member detail in at least the no-initial-message path, but the stronger ledger/signoff fixture is not satisfied by an end-to-end test or response surface.

### High: Fixture 5 churn-to-abort proof is not a full sequence and lacks explicit residue policy assertion

Evidence:

- Exact R05B fixture text requires a "full churn-to-abort sequence at configured concurrency" and an "explicit retention/cleanup expectation for pre-prompt failures": `docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md:96`.
- The implementation guard is real: `RunPlanChurnGuard::MAX_WAVES_WITHOUT_COMPLETION = 3`, bound helper `concurrency_limit * MAX_WAVES_WITHOUT_COMPLETION`, and diagnostic emission at `crates/jcode-app-core/src/tool/communicate.rs:560-625`.
- `run_plan` aborts on the guard after await and broadcasts an alert, but it returns the diagnostic directly with no cleanup and no explicit "retained for inspection" policy in the error text: `crates/jcode-app-core/src/tool/communicate.rs:1618-1631`.
- The test asserts only the helper formula and diagnostic strings: `crates/jcode-app-core/src/tool/communicate_tests.rs:181-224`. It does not drive `run_plan`, does not create sessions through the assignment loop, and does not assert actual created-session count or cleanup/retention state.

Impact:

- Static source reading suggests total fresh assignments are bounded by at most `concurrency_limit * 3` before abort because the guard records each assignment wave.
- The exact fixture proof is still weaker than declared. This is a test-realism/false-pass gap, not a demonstrated unbounded-session bug.

### Medium: Ledger states "six required offline fixture obligations" are closed, but two closures are overclaimed

Evidence:

- W2 ledger amendment says fixture obligations are closed at `docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md:137-175`.
- The command evidence for Auto and churn exactly names the helper-level tests noted above: `auto_visible_failure_allows_labeled_headless_fallback` and `run_plan_churn_guard_aborts_after_three_assignment_waves_without_completion`.
- Those commands passed locally, but they do not cover the stronger full-path claims.

Impact:

- This is an R09/R11 honesty issue in the validation wording, not a hidden baseline update or source-schema violation.

## Claims I could prove

### R05A entry tests reproduced

Commands run with `CARGO_NET_OFFLINE=true`, `CARGO_TARGET_DIR=/tmp/jcode-w2-grok-target`, `CARGO_INCREMENTAL=0`:

- `bash scripts/dev_cargo.sh test -p jcode-app-core control_log_fold_tracks_maps_through_handler_sequence -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.
- `bash scripts/dev_cargo.sh test -p jcode-app-core scan_from_tail_offset_finds_artifact_once -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.

This satisfies the W2 entry criterion in RECOVERY_PLAN lines 99-103.

### Explicit Visible Err/Ok(false) no longer falls back to headless

Evidence:

- `Visible` branch returns `Err` on `Ok((session_id, false))` and on launch error: `crates/jcode-app-core/src/server/comm_session.rs:427-434`.
- Headless creation is only reached after `resolve_swarm_spawn_creation(...)?` succeeds and returns `SwarmSpawnCreation::Headless`: `crates/jcode-app-core/src/server/comm_session.rs:697-738`.
- Focused command: `bash scripts/dev_cargo.sh test -p jcode-app-core visible_launch -- --nocapture` -> exit 0, `2 passed`, `0 failed`, `1098 filtered out`.

Limit: helper-level injection proves resolver behavior and static control flow proves no headless branch after `Err`; it does not launch a real terminal, by instruction.

### Stale direct takeover preserves prior progress/checkpoint/heartbeat/detail/reclaim provenance

Evidence:

- Direct assignment preserves existing progress and appends takeover provenance rather than replacing with `Default`: `crates/jcode-app-core/src/server/comm_control.rs:1723-1750`.
- Test seeds heartbeat/detail/checkpoint/count/reclaim state and asserts it survives: `crates/jcode-app-core/src/server/comm_control_tests/assign_task.rs:200-322`.
- Focused command: `bash scripts/dev_cargo.sh test -p jcode-app-core assign_task_stale_direct_takeover_preserves_progress_history -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.

### Below-cap and cap-fail reclaim preserve provenance while appending reason/outcome

Evidence:

- Shared append helper appends into `checkpoint_summary` without adding a schema field: `crates/jcode-plan/src/lib.rs:70-79`.
- `reclaim_stranded_assignment` preserves prior fields and appends "assignment reclaimed": `crates/jcode-plan/src/lib.rs:634-653`.
- Cap-fail uses `append_progress_provenance` instead of overwriting checkpoint text: `crates/jcode-app-core/src/server/swarm.rs:445-460`.
- Focused commands:
  - `bash scripts/dev_cargo.sh test -p jcode-plan reclaim_stranded_assignment_releases_owner_and_counts_reclaims -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `78 filtered out`.
  - `bash scripts/dev_cargo.sh test -p jcode-app-core salvage_requeues_dead_members_tasks_and_notifies_coordinator -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.
  - `bash scripts/dev_cargo.sh test -p jcode-app-core salvage_fails_task_once_reclaim_cap_is_reached -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.

### Lazy/eager/report liveness decisions now share one authority

Evidence:

- Central predicate lives in `crates/jcode-app-core/src/swarm_verbs.rs:54-55`.
- Server `swarm` delegates to that predicate: `crates/jcode-app-core/src/server/swarm.rs:372-373`.
- Lazy reclaim uses `super::swarm::member_status_is_dead`: `crates/jcode-app-core/src/server/comm_control.rs:732-735`.
- Eager salvage uses the same server predicate: `crates/jcode-app-core/src/server/swarm.rs:781-785`.
- Report/verb path uses `crate::swarm_verbs::member_status_is_dead`: `crates/jcode-app-core/src/swarm_verbs.rs:84-87`.
- Literal search found the only live dead-status triple under changed R05B decision code in `swarm_verbs.rs:55`; other matches were comments, terminal-status predicates, or tests.
- Focused commands:
  - `bash scripts/dev_cargo.sh test -p jcode-app-core member_status_is_dead_matches_terminal_non_success_states -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.
  - `bash scripts/dev_cargo.sh test -p jcode-app-core f1_assign_next_reclaims_task_from_departed_assignee -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.
  - `bash scripts/dev_cargo.sh test -p jcode-app-core failed_instance_needs_retry -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.

### Offline R04 dead-PID chain works for the requeue path

Evidence:

- Test persists an active session with fake dead PID, sweeps it to `crashed`, salvages assignment, asserts exactly one requeue, no duplicate assignment, notification, and preserved history: `crates/jcode-app-core/src/server/swarm.rs:2063-2173`.
- Focused command: `bash scripts/dev_cargo.sh test -p jcode-app-core dead_pid_sweep_then_salvage_requeues_once_without_duplicate_assignment -- --nocapture` -> exit 0, `1 passed`, `0 failed`, `1099 filtered out`.

Limit: This proves the requeue branch. The cap-fail branch is separately unit-tested, but the dead-PID chain fixture does not drive a dead-PID-to-cap-fail variant.

## Commit sequencing and changed surfaces

Observed commit order, base..HEAD:

1. `2d36b9f49 fix: fail closed explicit visible swarm spawn`
2. `a87f81f9d fix: preserve swarm reclaim progress history`
3. `282bad941 fix: bound swarm churn after dead workers`
4. `5ae37a297 refactor: centralize swarm liveness authority`
5. `c82de8b3f fix: keep churn bound helper test-only`
6. `da8fb9e01 test: format W2 swarm fixtures`
7. `2f4dfd7d2 docs: record R05B W2 recovery validation`

Static checks:

- `git diff --name-status base..HEAD` -> 11 changed files, all within RECOVERY_PLAN W2 surface or tests/docs: `comm_control.rs`, `comm_session.rs`, `swarm.rs`, `swarm_verbs.rs`, dispatch portions of `tool/communicate.rs`, `jcode-plan/src/lib.rs`, related tests, and R05B ledger.
- `git diff --shortstat base..HEAD` -> `11 files changed, 599 insertions(+), 42 deletions(-)`.
- `git diff --check base..HEAD` -> exit 0.
- Commit trailer check over all seven commits -> all include `Co-authored-by: agent <agent@rudnik.online>`.
- Extra commits 5 and 6 match the ledger explanation as narrow cleanup/test formatting commits. They are not in the original ideal commit plan, but they are small and after the main behavior/refactor commits.

## Durable schema/R06A and R04 vocabulary boundaries

- No new `SwarmTaskProgress` field was added in W2. `git diff --unified=0 base..HEAD -- crates/jcode-plan/src/lib.rs` shows a new helper and use-site changes, not schema widening.
- History is preserved by appending into existing `checkpoint_summary`: `crates/jcode-plan/src/lib.rs:70-79`.
- R04 status vocabulary remains the existing dead triple `failed | stopped | crashed` via `swarm_verbs::member_status_is_dead`: `crates/jcode-app-core/src/swarm_verbs.rs:54-55`.
- I found no new R04 lifecycle status string introduced by W2.

## R09 honesty and ledger append-only checks

Commands:

- `python3 -m unittest discover -s tests -p test_rust_production_filter.py` -> exit 0, `Ran 17 tests`, `OK`.
- `python3 -m py_compile scripts/rust_production_filter.py scripts/check_panic_budget.py scripts/check_swallowed_error_budget.py tests/test_rust_production_filter.py` -> exit 0.
- `python3 scripts/check_wildcard_reexport_budget.py` -> exit 0, `wildcard re-export budget check passed (total=16)`.
- `python3 scripts/check_panic_budget.py` -> expected red exit 1, `31 -> 48`; W2-attributed entries include `comm_session.rs (2)` and `tool/communicate.rs (1)`, matching the ledger.
- `python3 scripts/check_swallowed_error_budget.py` -> expected red exit 1, `2987 -> 3074`; W2-attributed entries include `comm_session.rs 22 -> 24` and `swarm.rs 20 -> 21`, matching the ledger.
- `python3 scripts/check_code_size_budget.py` -> expected red exit 1; W2-owned growth includes `comm_control.rs 2729 -> 2739`, `comm_session.rs 1341 -> 1615`, `swarm.rs 3138 -> 3584`, and `tool/communicate.rs 3178 -> 3427`, matching the ledger.
- `python3 scripts/check_test_size_budget.py` -> expected red exit 1; W2-owned entries include new oversized `comm_session_tests.rs 1264`, `tool/communicate_tests.rs 1639 -> 2063`, and pre-existing/current-tree `comm_control_tests/dag_e2e.rs` appears as expected.
- Static `git diff --name-only base..HEAD | rg 'baseline|ratchet|budget|QUALITY_GATES|R09|Cargo.lock'` -> no changed baseline/ratchet/budget paths.
- Static `git diff base..HEAD | rg -- '--update'` -> only documentation lines saying no `--update` was used.
- R05B signoff hashes reproduced:
  - `2871d7913f855b79df2e4539f9206cdb29f66a1b249d02990f492e41d97bbecc  docs/fork/recovery/reviews/2026-07-15-r05b-sol-signoff.md`
  - `9c15ba39c72ce367f818cd289a7716df8c4bdb7fca21f75e8a23b7f2bf9ae6bc  docs/fork/recovery/reviews/2026-07-15-r05b-fable-signoff.md`
- Ledger append-only check for docs commit: `git show --numstat 2f4dfd7d2 -- docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md` -> `99  0`.

## Non-W2 behavior

No changed file outside the W2 declared source/test/docs surface was observed. Headless/Inline spawn still resolve directly to headless creation with no visible attempt: `crates/jcode-app-core/src/server/comm_session.rs:423-426`, `667-695`. Existing terminal-status predicates in `jcode-plan` and `server/swarm` are broader than R05B dead-assignee policy and appear unchanged in purpose.

I did not find a concrete non-W2 behavior regression beyond the intended explicit-Visible fail-closed semantic change.

## Command environment caveat

The focused Cargo commands were invoked with `CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/jcode-w2-grok-target CARGO_INCREMENTAL=0`. Because `cargo` was not initially on PATH, `scripts/dev_cargo.sh` printed that it re-entered the repo Nix dev shell, printed cached substituter settings, imported rerere cache, and installed git hooks. I did not intentionally launch a terminal, live daemon/swarm, network operation, credentials, MCP, reload, publication, baseline update, or `--update`. `git status --short`, `git diff --name-only`, and `git diff --cached --name-only` after tests showed no tracked or staged repository changes.

## Scope limits and not checked

- No live daemon, real swarm, real terminal launch, network, credentials, MCP/tools, reload, publication, parent integration, or baseline update was exercised.
- I did not run the full app-core or workspace test suite.
- I did not rerun Nix-based dependency-boundary or warning-budget gates after observing the dev-shell side effects in the focused test run; I did run the Python classifier, wildcard, and expected-red R09 gates listed above.
- I did not independently prove real OS process liveness beyond the offline fake-dead-PID fixture.
- I did not inspect every non-W2 file in the repository, only changed surfaces and directly relevant ledgers/signoffs.

## Confidence

**Medium-high**. The source and focused offline tests support most W2 behavior. The FAIL rests on exact fixture/proof language and observable-response/residue-policy gaps, not on a discovered unbounded loop or data-erasing source path.
