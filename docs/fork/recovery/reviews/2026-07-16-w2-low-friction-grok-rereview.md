# W2/R05B final HEAD low-friction repair review

Verdict: **PASS**

Reviewed repository: `/Users/jrudnik/labs/jcode-w2-r05b`  
Branch: `recovery/fix-r05b-spawn-reclaim-2026-07-15`  
Base: `602709895be96a85a6090690c0b27d5681d17321`  
HEAD reviewed: `f8c5f8204056ff783d99769e4088e7bcceb56d73` (`f8c5f8204`)  
Mode: read-only incident/correctness review. I did not edit tracked repository files. I wrote only this review artifact under `/tmp`. Focused deterministic local tests were run under the requested offline environment.

## Executive conclusion

Final HEAD satisfies the RECOVERY_PLAN W2 / R05B acceptance bar for the six required offline fixtures and resolves the prior review blockers without reintroducing the R03A/wire-scope violation.

The decisive points are:

1. Explicit `Visible` spawn now fails closed for both visible-launch `Err` and `Ok(false)`, with no headless fallback path after the resolver returns `Err`.
2. `Auto` fallback remains in scope after the low-friction repair: the fallback is visible in member label/detail, event history, and live swarm event stream, and its detail survives initial-prompt status transitions. The response/replay metadata leg was intentionally removed per the Opus scope FAIL. I found no in-scope behavior regression from that removal.
3. Stale direct takeover, below-cap reclaim, and cap-fail preserve prior heartbeat/detail/checkpoint/count provenance and append new provenance instead of substituting it away.
4. Lazy reclaim, eager salvage, and verb/report classification share the same dead-status authority.
5. Full configured-concurrency churn-to-abort is now tested through the `run_plan` tool/server path, with the session creation bound and residue policy asserted.
6. The R04-to-R05B dead-PID chain is covered offline: persisted active dead PID -> crashed member -> exactly one requeue -> coordinator notification -> no duplicate assignment -> preserved history.

I found **no blocking findings**.

## Severity-ranked findings

### Blocking / High

None.

### Medium

None.

### Low / residual notes

1. **Response metadata is intentionally not present.** The original fixture text said Auto fallback should be visible in event/detail/response. The low-friction repair deliberately drops the response/replay metadata because it crossed R03A-owned wire/durable replay scope. In-scope observability is now event/detail/history, with any future response field deferred to R03A. This is acceptable because the operator-selected repair path and ledger record that decision, but it remains a product/governance tradeoff.
2. **The worktree currently has an untracked evidence directory:** `docs/fork/recovery/evidence/2026-07-16-w2-scope-repair/`. It contains README/log/hash evidence for the same final HEAD and reports tracked status clean inside `scope-state.txt`. It is not part of final HEAD. I did not remove or modify it. This is not an R05B correctness blocker, but parent integration should decide whether to commit or discard it.
3. **No live terminal/daemon/provider/credential path was exercised or needed.** The `communicate_run_plan...` fixture starts an in-process temporary test server/socket as a cargo test fixture. That is not a production daemon or external network/credential exercise. No real terminal launch, live swarm pilot, network provider, credential, reload, publication, or baseline update is required to validate this W2 package.

## Scope checked

### Recovery plan and ledger obligations

RECOVERY_PLAN W2 requires:

- entry reproduction of two R05A fixtures;
- fixture 1: explicit Visible fail-closed plus labeled Auto fallback;
- fixture 2: stale direct takeover preserves history;
- fixture 3: automatic reclaim and cap-fail preserve history;
- fixture 4: one liveness authority;
- fixture 5: bounded residual session count via full churn-to-abort at configured concurrency and explicit residue policy;
- fixture 6: R04 -> R05B dead-PID chain with exactly one requeue or cap-fail, notification, no duplicate assignment, preserved history;
- stop if live daemon/terminal/credentials are needed, R04 vocabulary changes, durable schema crosses R06A, or protocol/wire governance is needed.

I checked the changed paths from base..HEAD:

- `crates/jcode-app-core/src/server/comm_control.rs`
- `crates/jcode-app-core/src/server/comm_control_tests.rs`
- `crates/jcode-app-core/src/server/comm_control_tests/assign_task.rs`
- `crates/jcode-app-core/src/server/comm_session.rs`
- `crates/jcode-app-core/src/server/comm_session_tests.rs`
- `crates/jcode-app-core/src/server/swarm.rs`
- `crates/jcode-app-core/src/swarm_verbs.rs`
- `crates/jcode-app-core/src/tool/communicate.rs`
- `crates/jcode-app-core/src/tool/communicate_tests.rs`
- `crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs`
- `crates/jcode-plan/src/lib.rs`
- W2 review/ledger docs under `docs/fork/recovery/`

The final changed-path set does **not** include `crates/jcode-protocol/src/wire.rs` or `server/swarm_mutation_state{,_tests}.rs`.

### Prior history preservation

The append-only history remains visible:

- Initial W2 implementation and fixture closure are preserved in `docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md:137-231`.
- Grok's HIGH-gap FAIL is preserved at `ledger.md:236-242` and byte-preserved as `docs/fork/recovery/reviews/2026-07-15-w2-grok-review.md` with SHA-256 `6b3df2d04cc0ca7e7756def3643836e8590e1a51a25de5d054a6ecd8131413ae`.
- HIGH-gap remediation commits `2a5beea61` and `6115daa39` are preserved at `ledger.md:244-332`.
- Opus scope/governance FAIL is preserved at `ledger.md:334-340` and byte-preserved as `docs/fork/recovery/reviews/2026-07-16-w2-scope-adjudication.md` with SHA-256 `b44b7acd0324a4fe76bf1696f4d44792b56832396b7c00884ef3a3b1e3be9a2b`.
- Low-friction repair commits `6dfe2cdb6` and `f13620596` plus final ledger commit `f8c5f8204` are preserved at `ledger.md:342-407`.

## Obligation-by-obligation evidence

### 1. Explicit `Visible` fail-closed and `Auto` fallback detail

**Code evidence**

- `crates/jcode-app-core/src/server/comm_session.rs:434-467`: `resolve_swarm_spawn_creation` distinguishes `Visible` from `Auto`.
  - `Visible` returns `Err` for `Ok((session_id, false))` at lines 445-447.
  - `Visible` returns `Err` for launch errors at line 448.
  - `Auto` alone falls back to `Headless` with fallback detail preserving `requested Auto -> resolved Headless` and the original launch error/false result at lines 451-463.
- `comm_session.rs:712-753`: `spawn_swarm_agent` calls the resolver with `?`, so an explicit `Visible` resolver error exits before headless creation.
- `comm_session.rs:793-804`: Auto fallback labels the member and sets `member.detail` to the fallback detail.
- `comm_session.rs:827-931`: for a headless fallback with an initial prompt, running/ready/failed status updates wrap the new detail with `auto_fallback_status_detail`, preserving the original fallback detail instead of overwriting it.

**Test evidence**

- `crates/jcode-app-core/src/server/comm_session_tests.rs:664-688`: helper-level tests cover explicit Visible false and error fail-closed.
- `comm_session_tests.rs:690-711`: helper-level Auto fallback detail and label include `requested Auto -> resolved Headless` and original error.
- `comm_session_tests.rs:713-840`: handler-level Auto fallback fixture drives `handle_comm_spawn` with `JCODE_TEST_VISIBLE_SPAWN_ERROR`, an initial prompt, and asserts response success, headless member, fallback detail survives status updates, joined event history, and live event stream join.

**Review result**

PASS. Explicit `Visible` cannot silently downgrade to headless. Auto fallback detail survives prompt status changes through in-scope member/event observability. Response metadata is not present by design after low-friction scope repair.

### 2. Stale direct takeover preserves history

**Code evidence**

- `crates/jcode-app-core/src/server/comm_control.rs:1723-1750`: direct assignment records previous assignee, updates assignment fields, and appends takeover provenance through `jcode_plan::append_progress_provenance` instead of replacing progress with `Default`.
- `crates/jcode-plan/src/lib.rs:70-79`: `append_progress_provenance` appends new text to an existing checkpoint summary with ` | `, preserving prior checkpoint text.

**Test evidence**

- `crates/jcode-app-core/src/server/comm_control_tests/assign_task.rs:200-322`: seeds stale/running assignment with heartbeat, detail, checkpoint summary/count, and reclaim count, then asserts those survive and takeover provenance is appended.

**Review result**

PASS.

### 3. Automatic reclaim and cap-fail preserve history

**Code evidence**

- `crates/jcode-plan/src/lib.rs:634-653`: `reclaim_stranded_assignment` clears only the binding, preserves progress, increments `dead_assignee_reclaims`, and appends reclaim provenance.
- `crates/jcode-app-core/src/server/swarm.rs:427-472`: `salvage_plan_assignments_of` requeues below-cap tasks using the plan reclaim primitive and cap-fails tasks by clearing assignment and appending cap-fail provenance.

**Test evidence**

- `crates/jcode-plan/src/lib.rs:1118-1172`: primitive reclaim test preserves old checkpoint and appends `assignment reclaimed`.
- `crates/jcode-app-core/src/server/swarm.rs:3175-3240`: below-cap salvage preserves heartbeat/detail/checkpoint count/old checkpoint and notifies coordinator.
- `swarm.rs:3243-3290`: cap-fail salvage preserves heartbeat/detail/checkpoint count/old checkpoint and appends cap-fail reason.

**Review result**

PASS.

### 4. One liveness authority

**Code evidence**

- `crates/jcode-app-core/src/swarm_verbs.rs:54-55`: central dead-status predicate is `failed | stopped | crashed`.
- `crates/jcode-app-core/src/server/swarm.rs:372-373`: server-side `member_status_is_dead` delegates to `swarm_verbs`.
- `crates/jcode-app-core/src/server/comm_control.rs:732-735`: lazy stranded reclaim uses `super::swarm::member_status_is_dead`.
- `crates/jcode-app-core/src/server/swarm.rs:781-785`: staleness salvage uses the same server predicate.
- `crates/jcode-app-core/src/swarm_verbs.rs:86-87`: verb/report retry classification uses the central predicate.

**Test evidence**

- `crates/jcode-app-core/src/server/swarm.rs:3124-3135`: server and verb predicates agree for dead and live statuses.
- Existing focused tests cover lazy reclaim and verb retry classification.

**Review result**

PASS. I did not find remaining hand-written R05B dead triples in production decision paths.

### 5. Full configured-concurrency churn-to-abort and residue policy

**Code evidence**

- `crates/jcode-app-core/src/tool/communicate.rs:560-627`: `RunPlanChurnGuard` aborts after `MAX_WAVES_WITHOUT_COMPLETION = 3`, records churned nodes/lost workers, and emits a diagnostic with the explicit residue policy: default run_plan error cleanup cleans pre-prompt failed sessions, `retain_agents=true` retains them for inspection.
- `communicate.rs:1290-1327`: `run_swarm_plan_to_terminal` runs finished-worker cleanup on `run_plan` errors unless `retain_agents=true`.
- `communicate.rs:1337-1354`: `run_plan` derives configured deep concurrency and initializes the churn guard.
- `communicate.rs:1618-1631`: after await, the guard aborts and broadcasts the diagnostic when three assignment waves complete no nodes.

**Test evidence**

- `crates/jcode-app-core/src/tool/communicate_tests.rs:180-239`: helper-level formula and diagnostic test asserts `initial_sessions + concurrency_limit * MAX_WAVES_WITHOUT_COMPLETION` and residue wording.
- `crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs:288-404`: full offline tool/server fixture uses configured concurrency `2`, six independent plan nodes, a deterministic failing provider, asserts exactly six provider calls/sessions (`2 * 3`), asserts residue wording, asserts cleanup message, and asserts zero retained failed workers after default cleanup.

**Review result**

PASS. This closes the prior Grok HIGH gap: the proof is no longer helper-only and includes the default cleanup/retention policy.

### 6. R04 -> R05B dead-PID chain

**Code evidence**

- `crates/jcode-app-core/src/server/swarm.rs:269-318`: `sweep_dead_pid_swarm_members` reconciles active sessions, loads non-dead members, marks crashed sessions `crashed`, and records `client process exited` detail.
- `swarm.rs:475-...` / `salvage_assignments_of_dead_member`: salvage persists/broadcasts plan changes and notifies coordinator when dead-member assignments are requeued or failed.

**Test evidence**

- `crates/jcode-app-core/src/server/swarm.rs:2063-2173`: persists an active session with a fake dead PID, runs dead-PID sweep, asserts crashed status/detail, salvages, asserts exactly one requeue, no failed tasks, no duplicate assignment, preserved heartbeat/detail/checkpoint, appended reclaim provenance, and coordinator notification.

**Review result**

PASS.

## Removal of response/replay metadata did not regress in-scope behavior

I specifically checked the Opus scope FAIL remediation:

- `git diff --name-only base..HEAD -- crates/jcode-protocol/src/wire.rs crates/jcode-app-core/src/server/swarm_mutation_state.rs crates/jcode-app-core/src/server/swarm_mutation_state_tests.rs` returned no paths.
- `rg -n "requested_spawn_mode|spawn_fallback_detail|SwarmSpawnOutcome" crates` returned no matches.
- `crates/jcode-protocol/src/wire.rs:1490-1498` shows `CommSpawnResponse` has only `id`, `session_id`, `new_session_id`, and `initial_prompt_delivered`.
- `crates/jcode-app-core/src/server/swarm_mutation_state.rs:27-32` shows persisted spawn response has only `new_session_id` and `initial_prompt_delivered`.
- The handler-level Auto fallback test still passes and proves detail/history/live-event observability after the response/replay fields were removed.

Conclusion: the protocol/replay widening was removed, and the in-scope R05B observability behavior remains covered.

## Commands and results

### Static state / scope commands

```text
pwd
# /Users/jrudnik/labs/jcode-w2-r05b

git rev-parse HEAD
# f8c5f8204056ff783d99769e4088e7bcceb56d73

git rev-parse 602709895be96a85a6090690c0b27d5681d17321
# 602709895be96a85a6090690c0b27d5681d17321

git log --oneline --decorate --reverse 602709895be96a85a6090690c0b27d5681d17321..HEAD
# 2d36b9f49 fix: fail closed explicit visible swarm spawn
# a87f81f9d fix: preserve swarm reclaim progress history
# 282bad941 fix: bound swarm churn after dead workers
# 5ae37a297 refactor: centralize swarm liveness authority
# c82de8b3f fix: keep churn bound helper test-only
# da8fb9e01 test: format W2 swarm fixtures
# 2f4dfd7d2 docs: record R05B W2 recovery validation
# 47c31fccc docs: Preserve W2 review failure.
# 2a5beea61 fix: expose auto spawn fallback path
# 6115daa39 test: prove run_plan churn abort bound
# a342cd5fb docs: record R05B proof gap remediation
# 66cc39541 docs: Preserve W2 scope adjudication.
# 6dfe2cdb6 fix: remove W2 spawn response widening
# f13620596 test: prove fallback without protocol widening
# f8c5f8204 docs: record W2 low-friction scope repair

git diff --name-status 602709895be96a85a6090690c0b27d5681d17321..HEAD
# 14 changed paths, all listed in Scope checked above; no protocol or swarm_mutation_state files in final diff.

git diff --stat 602709895be96a85a6090690c0b27d5681d17321..HEAD
# 14 files changed, 1413 insertions(+), 48 deletions(-)

shasum -a 256 docs/fork/recovery/reviews/2026-07-15-w2-grok-review.md docs/fork/recovery/reviews/2026-07-16-w2-scope-adjudication.md docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md
# 6b3df2d04cc0ca7e7756def3643836e8590e1a51a25de5d054a6ecd8131413ae  docs/fork/recovery/reviews/2026-07-15-w2-grok-review.md
# b44b7acd0324a4fe76bf1696f4d44792b56832396b7c00884ef3a3b1e3be9a2b  docs/fork/recovery/reviews/2026-07-16-w2-scope-adjudication.md
# 838c01a770e3eb814c3f95226a6a6724860dfc860b99901f71ee705f9ba91e8b  docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md

git diff --name-only 602709895be96a85a6090690c0b27d5681d17321..HEAD -- crates/jcode-protocol/src/wire.rs crates/jcode-app-core/src/server/swarm_mutation_state.rs crates/jcode-app-core/src/server/swarm_mutation_state_tests.rs
# no output

rg -n "requested_spawn_mode|spawn_fallback_detail|SwarmSpawnOutcome" crates || true
# no output

git diff --check 602709895be96a85a6090690c0b27d5681d17321..HEAD
# exit 0, no output
```

### Focused offline validation matrix

All commands were run inside:

```text
FORK_NUDGE_MAX_AGE=2147483647
FORK_NUDGE_AUTOSYNC=0
CARGO_NET_OFFLINE=true
CARGO_INCREMENTAL=0
CARGO_TARGET_DIR=/tmp/jcode-w2-low-friction-review-target
nix develop --offline -c bash -lc '<commands>'
```

Results:

```text
bash scripts/dev_cargo.sh test -p jcode-app-core handle_comm_spawn_auto_fallback_preserves_history_and_detail_with_prompt -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.24s

bash scripts/dev_cargo.sh test -p jcode-app-core communicate_run_plan_churns_to_abort_at_configured_concurrency_and_cleans_failed_workers -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 3.99s

bash scripts/dev_cargo.sh test -p jcode-app-core visible_launch -- --nocapture --test-threads=1
# test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1100 filtered out; finished in 0.00s

bash scripts/dev_cargo.sh test -p jcode-app-core assign_task_stale_direct_takeover_preserves_progress_history -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.54s

bash scripts/dev_cargo.sh test -p jcode-plan reclaim_stranded_assignment_releases_owner_and_counts_reclaims -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 78 filtered out; finished in 0.00s

bash scripts/dev_cargo.sh test -p jcode-app-core salvage_requeues_dead_members_tasks_and_notifies_coordinator -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.01s

bash scripts/dev_cargo.sh test -p jcode-app-core salvage_fails_task_once_reclaim_cap_is_reached -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.00s

bash scripts/dev_cargo.sh test -p jcode-app-core member_status_is_dead_matches_terminal_non_success_states -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.00s

bash scripts/dev_cargo.sh test -p jcode-app-core f1_assign_next_reclaims_task_from_departed_assignee -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.60s

bash scripts/dev_cargo.sh test -p jcode-app-core failed_instance_needs_retry -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.00s

bash scripts/dev_cargo.sh test -p jcode-app-core dead_pid_sweep_then_salvage_requeues_once_without_duplicate_assignment -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.01s

bash scripts/dev_cargo.sh test -p jcode-app-core control_log_fold_tracks_maps_through_handler_sequence -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.59s

bash scripts/dev_cargo.sh test -p jcode-app-core scan_from_tail_offset_finds_artifact_once -- --nocapture --test-threads=1
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1101 filtered out; finished in 0.00s

bash scripts/dev_cargo.sh check -p jcode-app-core -p jcode-protocol -p jcode-plan
# Finished `dev` profile [unoptimized] target(s) in 1m 04s
# background task exit code: 0
```

The full background task was `746724nxn2`, duration `451.10s`, exit code `0`.

## Residual risks

- The response-leg Auto fallback metadata is deferred, not implemented. If a future API consumer needs requested/resolved mode on `CommSpawnResponse`, it must go through R03A protocol governance instead of W2.
- The run_plan fixture proves a deterministic pre-prompt failure churn scenario with configured concurrency `2`. It does not exhaust every possible provider failure timing, but it covers the incident-shaped full churn-to-abort path and residue policy required by W2.
- The dead-PID chain fixture proves the requeue branch. Cap-fail is separately tested, but there is no combined dead-PID-to-cap-fail fixture. W2 wording allowed exactly one requeue or cap-fail, and separate cap-fail history coverage exists.
- I did not run the broader R09 expected-red matrix or full workspace tests in this review. The final W2 ledger records broader prior validation. I only reran the focused W2/R05A matrix and affected crate check under the user's allowed environment.
- Untracked evidence directory noted above remains outside HEAD.

## What I did not check

- No live terminal spawn, real daemon, real provider, network API, credential, MCP/tool, reload, publication, release, install, updater, or baseline mutation.
- No parent-branch integration, merge/rebase, stash/ref/worktree movement, or commit creation.
- No full UI/TUI rendering inspection.
- No exhaustive fuzz/state-space search of all swarm statuses beyond the targeted fixture matrix and static source review.
- No full `cargo test --workspace`; this was intentionally limited to deterministic focused offline checks.

## Confidence

**High.** The final code paths are narrow, the prior failure history is preserved, the protocol/replay widening is removed from final HEAD, and every W2/R05B acceptance fixture plus the R05A entry fixtures passed under the requested offline environment. My remaining uncertainty is limited to broader live-system behavior that this workstream explicitly does not authorize or require.
