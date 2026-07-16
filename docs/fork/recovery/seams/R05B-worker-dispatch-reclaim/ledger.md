# R05B Worker dispatch, spawn mode, liveness, reclaim, and failure backoff: authoritative ledger

| Field | Value |
|---|---|
| State | `blocked` (adjudicated; not approved for swarm widening) |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review head | `5baf343ba6da564afc3f6c58c5edca7a64d6e67f` |
| Review mode | `full` |
| Research budget | 8 decisive checkpoints, all consumed without extension |
| Authority today | `fork` for the reviewed behavior; upstream is comparison evidence only |
| Recommended disposition | `retain-fork` |
| Confidence | medium-high |
| Last updated | 2026-07-15 UTC |

## Preserved independent reviews

- `opus-review.md` is a byte-preserved copy of `/tmp/jcode-r05b-opus-review.md`, SHA-256 `e1afb8064071bdcf3ab0c672281661ce82d97a38b687a049c04fc3b35ae2ccb4`.
- `grok-review.md` is a byte-preserved copy of `/tmp/jcode-r05b-grok-review.md`, SHA-256 `20fd612212a9e03f62a2f06c27ae7ad3eb7c8e81349ff46a2321dbd5a53e78a5`.
- Terra reproduced both hashes using `shasum -a 256` and byte identity using `cmp -s` on 2026-07-15. These reviews remain independent evidence, including disagreements, rather than being rewritten here.

## Scope and invariants

- **Owns:** assignment dispatch, explicit and inherited spawn-mode policy, dead-worker detection as consumed policy, bounded assignment reclaim, retry limits, session-growth containment, and observable failure.
- **Excludes:** R05A DAG/control-log truth, and R04 process/session lifecycle, PID truth, and terminal-status authority. R05B consumes R05A's ready/assigned plan state and R04's member lifecycle state to choose dispatch and reclaim policy.
- **Must preserve:** cross-seam invariant 5 in `RESPONSIBILITIES.md:67`: one liveness authority per layer, no unbounded assignment or session-file growth after a dead process, and no reclaim that erases history. R09's no-`--update` rule and visible behavioral-debt attribution also bind.

### Exact R04/R05A/R05B boundary

| Layer | Owns | R05B interaction |
|---|---|---|
| R04 | Generic process/task life, dead-PID reconciliation, crash/stop/terminal status | `sweep_dead_pid_swarm_members` and lifecycle writers establish member status. R05B must not invent PID liveness. It reads `member_status_is_dead` for policy. |
| R05A | Dependency readiness, node transitions, control-log event/fold/replay, plan truth | `reclaim_stale_plan_assignments` synchronizes/reads the fold to find departed membership but does not define the fold. Dispatch only acts on R05A's runnable/blocked state. |
| R05B | Assignment choice, spawn authority, stale/dead reclaim, cap and churn policy, operator-visible failure | It maps R04's dead member plus R05A's eligible node to requeue, cap-fail, or bounded abort. It must preserve R05A evidence while doing so. |

## Eight decisive checkpoints

1. **Preservation and fixed refs.** Both review hashes above reproduce. `git rev-parse --verify` resolved the review head, fork, upstream, and merge base; `git merge-base fork upstream` returned `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.
2. **True fork contribution.** Static ref comparison found `RunPlanChurnGuard` occurrences base/upstream/fork = `0/0/3`; `reclaim_stale_plan_assignments` = `0/0/2`; `SwarmSpawnMode` had an `#[default]` in all three refs. The default was already `Inline` at the merge base and upstream. The fork contribution is churn containment, reclaim wiring, and fallback behavior, **not** changing the default to Inline.
3. **Dispatch and progress write.** `handle_comm_assign_task_with_mode` performs the direct assignment and, at `comm_control.rs:1724-1739`, changes `assigned_to`/`queued` then replaces `task_progress` with `SwarmTaskProgress { ..Default::default() }`. `handle_comm_assign_next` auto-selects or spawns; the latter calls `spawn_swarm_agent` with no explicit mode and therefore inherits configuration. Auto-pick claims have a 15-second TTL.
4. **Spawn-mode authority.** `spawn_swarm_agent` resolves one authority at `comm_session.rs:579`: explicit request or configured mode. `Headless|Inline` use in-process creation. `Visible|Auto` both attempt a visible session at `:619-648`, then `Ok(false)|Err(_)` both silently create headless at `:651-691`.
5. **Liveness, timers, and reclaim.** Canonical `member_status_is_dead` in `swarm.rs:372-377` is `failed|stopped|crashed`. The stale reaper, dead-PID sweep, member terminal GC, task heartbeat/stale/sweep/reap thresholds, and auto-pick claim TTL are timed mechanisms. `next_runnable_task_id_reclaiming_stranded` independently copies the dead triple at `comm_control.rs:732-738`; `swarm_verbs.rs:55` is a third copy.
6. **Bound and visibility.** `RunPlanChurnGuard` aborts after three assignment waves without completion. Its deterministic third-wave fixture asserts the `possible spawn churn` diagnostic and names all nodes/workers. Salvage persists/broadcasts state and sends a coordinator notification. This bounds the prior storm but live-member capacity alone does not bound cumulative session files.
7. **History preservation audit.** `jcode-plan::reclaim_stranded_assignment` preserves the tested heartbeat/detail fields but overwrites `checkpoint_summary` with its own reclaim reason. Direct stale `assign_task` resets the entire record, and cap-fail overwrites `checkpoint_summary` in `salvage_plan_assignments_of`; the cap-fail fixture only asserts status and assignment. Therefore the invariant is not fully met for checkpoint provenance on either automatic reclaim path, and is substantially violated by stale direct takeover.
8. **No-runtime validation and hygiene.** Source and fixtures were inspected offline. No cargo or live process/terminal spawn test was run in this adjudication because the task explicitly prohibits live process/terminal/network/credential use. Independent review evidence records warmed offline results, but they are not represented as a fresh Terra test run. The mandatory post-write scope/hash/hygiene checks are recorded below.

## Divergence at a glance

| Concern | Fork | Upstream / merge base | Consequence |
|---|---|---|---|
| Default spawn mode | `Inline` | Already `Inline` | Do not credit the fork with a default change. |
| Spawn-storm protection | Churn guard, run-plan single-driver guard, capacity recovery, dead-assignee reclaim wiring | No churn guard or assign-time stale reclaim | Retain the fork safety behavior. |
| Visible authority | Explicit `Visible` silently falls back headless exactly like `Auto` | Not a basis to adopt upstream | Fork policy violates explicit-mode semantics and failure visibility. |
| Assignment history | Primitive reclaim preserves fields, but stale direct assignment replaces progress | No upstream remedy demonstrated | Violates the no-history-erasure invariant for a reclaim/takeover route. |
| Cap-fail history | Cap-fail retains many fields but overwrites last checkpoint summary | No upstream remedy demonstrated | Bounded residual history loss must be fixed or explicitly accepted with test evidence. |

## Evidence ledger: every relevant writer, timer, authority, and observable

| Category | Writers / authority path | Evidence and consequence |
|---|---|---|
| Assignment selection and binding | `handle_comm_assign_task_with_mode`; `handle_comm_assign_task`; `handle_comm_assign_next`; `resolve_assignment_target_for_task`; auto-target filter/load/claim helpers | Direct assignment writes `assigned_to`, `queued`, and replaces progress at `comm_control.rs:1724-1739`. `assign_next` is the automatic driver and is the spawn caller. The 15-second claim prevents concurrent auto-picks. |
| Assignment run/status | `spawn_assigned_task_run`; turn-end disposition; `handle_comm_task_control` retry/reassign/replace/requeue paths | These write queued/running/done/failed/requeued state and member queued/running/completed/failed state. Task-control's explicit reassign/retry is distinct from ordinary automatic reclaim. |
| Lazy reclaim | `next_runnable_task_id_reclaiming_stranded` -> `jcode_plan::next_stranded_runnable_item_id` -> `reclaim_stranded_assignment` | One stranded eligible node is reclaimed per dispatch. It is capped by `MAX_DEAD_ASSIGNEE_RECLAIMS = 3`; the primitive preserves heartbeat/detail fields but overwrites the prior checkpoint summary, which needs an append-only repair. |
| Assign-time departed-assignee sweep | `reclaim_stale_plan_assignments`, called before direct assignment | Non-terminal, non-running work held by a departed control-log member has only `assigned_to` cleared and plan version bumped. It is a separate writer and currently does not use the canonical dead-status predicate. |
| Eager dead-member reclaim | `salvage_assignments_of_dead_member` -> `salvage_plan_assignments_of`, called from `refresh_swarm_task_staleness` and `remove_session_from_swarm` | Under cap it calls the primitive then queues. At cap it fails/unassigns and overwrites the terminal checkpoint summary. It persists, broadcasts, and notifies. |
| Staleness/reaper | `refresh_swarm_task_staleness` | Writes `running` -> `running_stale`, revives fresh heartbeats, and fails departed stale work after reap threshold. Its second phase salvages dead/departed assignees with a grace period. |
| R04-fed liveness writers | dead-PID sweep, status update/report, stop/remove lifecycle paths | `sweep_dead_pid_swarm_members` mirrors exited sessions as crashed; R05B consumes it. Member GC uses the broader terminal predicate, intentionally not the dead predicate. |
| Liveness authority | `member_status_is_dead`; copied triple in `next_runnable_task_id_reclaiming_stranded`; copied triple in `swarm_verbs::is_failed` | The authority is canonical only at `swarm.rs:372-377`; copies can diverge when statuses evolve. |
| Timers / bounded mechanisms | auto-claim TTL; heartbeat interval; stale threshold; task-sweep interval; reap threshold; dead-PID sweep interval/atomic claim; terminal retention/GC interval; broadcast debounce; churn three-wave counter; live-member cap | All timing policy is bounded/configured. The member cap limits live members, not residual on-disk session files. The churn guard is the true assignment-wave brake. |
| Spawn-mode authority | parser -> client request -> `handle_comm_spawn` / `handle_comm_assign_next` -> `spawn_swarm_agent` -> visible launcher or headless creation | Explicit request overrides config. However, `Visible` and `Auto` share the same fallback branch, so only a failure of both visible and headless reaches the user-facing error path. |
| Failure visibility | active-assignment error; churn abort/broadcast; `DeadMemberSalvage::describe`/coordinator notification; orphan-reap warning; post-both-paths spawn error | These are observably useful except for visible-launch failure, which is swallowed by headless fallback. |
| History preservation | `reclaim_stranded_assignment`; stale direct `assign_task`; cap-fail salvage; task-control requeue fixture | The primitive retains most fields but replaces checkpoint summary. Direct stale takeover replaces all progress. Cap-fail likewise destroys the prior checkpoint text. |

## Adjudication

| Disagreement / finding | Opus position | Grok position | Terra resolution | Deciding evidence |
|---|---|---|---|---|
| Default Inline as the incident fix | Default was already Inline; fork value is churn/reclaim/fallback | Agrees source has meaningful hardening, not a default claim | **Accepted.** The ledger must not attribute an Inline default change to the fork. | Static base/upstream/fork counts: default marker `1/1/1`; churn `0/0/3`; stale-reclaim `0/0/2`. |
| Fork retention | Conditional `retain-fork` because it carries the only storm remediation | Blocks approval of the seam, not the usefulness of hardening | **`retain-fork`, but state remains blocked.** Retention preserves unique safety behavior; approval/swarm widening waits on blockers below. | No upstream churn/reclaim counterpart; deterministic churn fixture exists. |
| Copied liveness predicate | IMPORTANT structural divergence | Notes health mechanisms but does not reject the same behavior | **IMPORTANT.** Refactor to one callable authority and cover assign/reclaim paths. | `member_status_is_dead` at `swarm.rs:372-377`; copies at `comm_control.rs:734` and `swarm_verbs.rs:55`. |
| Explicit `Visible` fallback | Treats fallback as incident fail-safe | BLOCKER: user-visible request silently becomes headless | **BLOCKER.** Explicit `Visible` is an authority contract, not permission to degrade silently. Only `Auto` may fallback, and it must surface the fallback. | `comm_session.rs:619-648` groups modes; `:651-691` groups their fallback; error only follows later headless failure. |
| Stale direct `assign_task` takeover | Not identified as a blocker in Opus's conclusion | IMPORTANT history loss | **IMPORTANT.** It is a reclaim/takeover policy route and replacing progress violates invariant 5. | Stale work bypasses active conflict at `comm_control.rs:1668-1683`; `task_progress.insert(..Default::default())` at `:1727-1738`. |
| Cap-fail checkpoint overwrite | IMPORTANT bounded residual: not full history loss but prior checkpoint is destroyed and untested | Did not identify separately | **IMPORTANT.** It violates “reclaim cannot erase history” in the checkpoint field, as does the lower-cap primitive. Preserve old checkpoint provenance and append terminal reason, rather than substitute it. | `salvage_plan_assignments_of` overwrites `checkpoint_summary`; `salvage_fails_task_once_reclaim_cap_is_reached` asserts only outcome/status/assignment. |
| Residual session files | Bounded but nonzero residual risk | Visible fallback makes containment non-decisive until F1 | **Residual risk, widening blocker.** Bound creation across churn-to-abort and decide cleanup/retention policy. | Three-wave churn bound plus live-member-only cap; no full session-count fixture. |
| Full R04 dead-PID to salvage chain | Partial unit coverage | Gap: no deterministic whole chain | **Required swarm-widening fixture.** Do not broaden to visible/dead-process scenarios without it. | Separate dead-PID and salvage fixtures, no composed process-to-notification fixture. |

### Terra reproduction

`git show <base|upstream|fork>:crates/jcode-config-types/src/lib.rs | grep -A12 'enum SwarmSpawnMode' | grep -c '#[default]'` returned one default marker at every fixed ref. The companion fixed-ref counts were churn `0/0/3` and stale-reclaim `0/0/2`; the focused upstream diff had no churn/reclaim/backoff remediation. This decides both the corrected provenance claim and the sole disposition: retain the fork's safety behavior, without inventing a default-mode credit.

## Required exact fixtures before approval or swarm widening

1. **Visible authority:** Inject visible launch `Err("terminal failed")` and `Ok(false)` into `spawn_swarm_agent` with explicit `Visible`. Assert no headless session is created and the response reports the visible error. Separate `Auto` fixture may create headless only if event/detail/response records `auto -> headless fallback` and original error.
2. **Stale direct takeover preserves history:** Seed `running_stale`, `assigned_to=old`, with `last_heartbeat_unix_ms`, `last_detail`, checkpoint summary/count, and `dead_assignee_reclaims`. Invoke the direct `assign_task(task_id, target=new)` handler. Assert assignment becomes `new` and old provenance survives in an append-only or explicit prior-history field.
3. **Automatic reclaim and cap-fail preserve history:** Seed one below-cap and one at-cap assignment with a worker-authored checkpoint, heartbeat/detail/counts, then reclaim/salvage. Assert the binding/status changes and every pre-fail provenance value remains available while each automatic reason is appended, not substituted.
4. **One liveness authority:** Change the status set through one authority and prove lazy stranded reclaim, staleness salvage, and verb/report classification agree. Remove hand-written dead triples.
5. **Bounded residual session count:** Drive a full churn-to-abort sequence at configured concurrency and assert total created sessions is bounded by the declared formula, with an explicit retention/cleanup expectation for pre-prompt failures.
6. **R04 -> R05B chain:** Persist a dead PID session and assigned running item, invoke dead-PID/status sweep plus staleness refresh, and assert crashed status, exactly one requeue or cap-fail, coordinator notification, no duplicate assignment, and preserved history.

## Recommendation and pilot relevance

- **Disposition: `retain-fork`.** Deleting or adopting upstream would discard the fork-only churn/reclaim protection. There is no upstream behavior to compose or adopt.
- **No-swarm pilot relevance:** R05B is not a prerequisite for the current smallest no-swarm pilot. `RESPONSIBILITIES.md:86` excludes R05B unless the chosen stack exercises swarm behavior. Do not block the no-swarm, no-tool one-turn pilot on this ledger.
- **Swarm-widening blockers:** Do not exercise `run_plan`, automatic worker spawning, explicit visible spawning, or dead-worker reclaim in a pilot until fixtures 1-6 pass and the corresponding fixes land in separate slices. The explicit-visible and stale-history defects are current behavioral blockers, not documentation debt.
- **Upstream opportunity:** none now. First repair/verify fork semantics; only then assess a small upstream patch for explicit-mode authority and history-safe reclaim.
- **Quality-of-life ideas:** no implementation in this seam record.

## Bounded implementation slices

| Slice | Class | Change | Acceptance | Rollback or stop condition |
|---|---|---|---|---|
| 1 | `fix` | Make explicit `Visible` fail closed; permit and visibly label fallback only for `Auto`. | Visible/Auto injected-launch fixtures pass; no silent semantic downgrade. | Stop if API consumers cannot distinguish requested from resolved mode without protocol change; isolate a compatibility proposal. |
| 2 | `fix` | Preserve prior progress in stale direct assignment and in both below-cap/cap-fail automatic reclaim, appending terminal/reassignment provenance. | Fixtures 2 and 3 pass; existing primitive reclaim fixture remains green. | Stop if durable schema/replay ownership crosses R06A; hand off schema design rather than silently change evidence semantics. |
| 3 | `refactor` | Route all R05B dead-status decisions through one authority. | Fixture 4 proves lazy/eager/report paths agree. | Stop if R04 status vocabulary needs a contract decision; R04 owns that vocabulary. |
| 4 | `fix` | Add churn-to-abort session bound and R04-to-R05B dead-PID salvage chain fixture. | Fixtures 5 and 6 pass offline and prove bounded creation/no duplicate dispatch. | Stop if proof requires a live daemon, terminal, credentials, or platform UI behavior; remain no-swarm and record the gap. |
| 5 | `docs` | Append fixture outcomes and R09 per-file debt attribution after source fixes. | No baseline update; every relevant red entry assigned or explicitly non-R05B. | Stop if it mixes behavior changes with ratchet updates or overwrites preserved evidence. |

## R09 debt, validation, and sign-off

- **R09 debt:** `R09-quality-gates/ledger.md` makes dispatch-reclaim debt R05B-owned but the per-file production/test-size violations still require enumeration before an implementation gate. Attribute only `comm_control.rs`, `comm_session.rs`, `swarm.rs`, `jcode-plan`, and dispatch portions of `tool/communicate.rs`; do not use `--update`. Current records suggest no R05B production panic marker in `comm_control.rs`/`comm_session.rs`, but that is not a substitute for the required per-file ratchet enumeration.
- **Source checks run by Terra:** fixed refs, merge base, source-level fork/upstream marker comparison, symbol/fixture inspection, preservation hashes, and clean pre-write worktree. No live service, terminal spawn, network, credential, or cargo execution was used.
- **Independent test evidence, not a fresh Terra run:** Opus records offline `jcode-plan` 79/0, app-core comm-control 80/0, swarm 29/0, and churn 5/0. Grok records two `jcode-plan` stranded tests passing and app-core compile timeout. These conflicting test scopes are preserved, not collapsed; rerun the exact six required fixtures after source fixes.
- **Failure modes checked:** wrong default provenance; auto-versus-explicit spawn authority; duplicate assignment guard; dead/departed reassignment; reclaim cap; churn waves; stale takeover history reset; cap-fail checkpoint overwrite; liveness-predicate drift; residual session growth; R04-to-R05B chain gap.
- **Remaining risks:** bounded residual session files, copied liveness predicate, no complete dead-PID-to-salvage fixture, and no fresh command execution under the no-live-process constraint.
- **Opus review:** conditional retain-fork, with IMPORTANT liveness/cap-fail and residual-risk findings.
- **Grok review:** fails approval on explicit-visible fallback and stale-takeover history loss, and requires the process-chain fixture.
- **Terra adjudication:** `blocked` for swarm widening, `retain-fork` as the only disposition, not a blocker for the current no-swarm pilot.
- **Sol sign-off:** pending.
- **Fable sign-off:** pending.

## Validation and sign-off amendment (2026-07-15)

- **Sol sign-off:** **PASS** for this authoritative blocked/`retain-fork` ledger. The byte-preserved record is [`2026-07-15-r05b-sol-signoff.md`](../../reviews/2026-07-15-r05b-sol-signoff.md), copied from `/tmp/jcode-r05b-sol-signoff.md`, SHA-256 `2871d7913f855b79df2e4539f9206cdb29f66a1b249d02990f492e41d97bbecc`.
- **Fable sign-off:** **PASS** for this authoritative blocked/`retain-fork` ledger. The byte-preserved record is [`2026-07-15-r05b-fable-signoff.md`](../../reviews/2026-07-15-r05b-fable-signoff.md), copied from `/tmp/jcode-r05b-fable-signoff.md`, SHA-256 `9c15ba39c72ce367f818cd289a7716df8c4bdb7fca21f75e8a23b7f2bf9ae6bc`.
- Terra reproduced both sign-off hashes and byte identity with `shasum -a 256` and `cmp -s` at preservation time. Both reviewers pass the ledger specifically because it preserves the evidence and the blocked state. Neither pass approves the seam implementation or swarm widening.
- **Current state unchanged:** R05B remains `blocked` for swarm-driven use and widening until its required fixes and fixtures land. The current no-swarm pilot remains outside R05B's prerequisite set.

## 2026-07-15 W2 recovery implementation amendment

**State amendment:** W2 recovery fixtures and fixes are implemented on isolated branch
`recovery/fix-r05b-spawn-reclaim-2026-07-15` from base `602709895`.
This closes the six required offline fixture obligations for this workstream, but it
is **not** swarm pilot authorization and does not widen to live daemon, terminal,
network, credential, MCP, reload, publication, baseline update, or parent-branch
integration.

### Commit sequence

All commits include `Co-authored-by: agent <agent@rudnik.online>`.

| Order | Commit | Purpose | Changed paths |
|---:|---|---|---|
| 1 | `2d36b9f49` | `fix: fail closed explicit visible swarm spawn` | `crates/jcode-app-core/src/server/comm_session.rs`; `crates/jcode-app-core/src/server/comm_session_tests.rs` |
| 2 | `a87f81f9d` | `fix: preserve swarm reclaim progress history` | `crates/jcode-plan/src/lib.rs`; `crates/jcode-app-core/src/server/comm_control.rs`; `crates/jcode-app-core/src/server/comm_control_tests.rs`; `crates/jcode-app-core/src/server/comm_control_tests/assign_task.rs`; `crates/jcode-app-core/src/server/swarm.rs` |
| 3 | `282bad941` | `fix: bound swarm churn after dead workers` | `crates/jcode-app-core/src/tool/communicate.rs`; `crates/jcode-app-core/src/tool/communicate_tests.rs`; `crates/jcode-app-core/src/server/swarm.rs` |
| 4 | `5ae37a297` | `refactor: centralize swarm liveness authority` | `crates/jcode-app-core/src/server/comm_control.rs`; `crates/jcode-app-core/src/server/swarm.rs`; `crates/jcode-app-core/src/swarm_verbs.rs` |
| 5 | `c82de8b3f` | `fix: keep churn bound helper test-only` | `crates/jcode-app-core/src/tool/communicate.rs` |
| 6 | `da8fb9e01` | `test: format W2 swarm fixtures` | `crates/jcode-app-core/src/server/comm_control_tests/assign_task.rs`; `crates/jcode-app-core/src/server/swarm.rs` |

Commits 5 and 6 are narrow validation cleanups after the main approved behavior
and refactor commits: commit 5 restores the warning budget by making the
fixture-facing churn-bound helper `#[cfg(test)]`; commit 6 is exact-path rustfmt
cleanup for W2-owned fixture files after full-tree rustfmt proved there is
pre-existing unrelated formatting debt outside this workstream.

### Fixture closure against the six required obligations

| Obligation | Evidence |
|---|---|
| 1. Visible authority | `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core visible_launch -- --nocapture` passed with `2 passed`, exit `0`; `auto_visible_failure_allows_labeled_headless_fallback` passed with `1 passed`, exit `0`. Explicit `Visible` `Err` and `Ok(false)` fail closed without headless fallback; `Auto` fallback is labeled with the original visible failure. |
| 2. Stale direct takeover preserves history | `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core assign_task_stale_direct_takeover_preserves_progress_history -- --nocapture` passed with `1 passed`, exit `0`. The takeover preserves heartbeat, detail, checkpoint count, reclaim count, old checkpoint text, and appends takeover provenance. |
| 3. Automatic reclaim and cap-fail preserve history | `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-plan reclaim_stranded_assignment_releases_owner_and_counts_reclaims -- --nocapture` passed with `1 passed`, exit `0`; `salvage_requeues_dead_members_tasks_and_notifies_coordinator` passed with `1 passed`, exit `0`; `salvage_fails_task_once_reclaim_cap_is_reached` passed with `1 passed`, exit `0`. Reclaim/cap-fail append provenance instead of substituting away prior checkpoint history. |
| 4. One liveness authority | `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core member_status_is_dead_matches_terminal_non_success_states -- --nocapture` passed with `1 passed`, exit `0`; `f1_assign_next_reclaims_task_from_departed_assignee` passed with `1 passed`, exit `0`; `failed_instance_needs_retry` passed with `1 passed`, exit `0`. Dead vocabulary remains `failed`, `stopped`, `crashed`, with `swarm_verbs` as the central callable and server/swarm delegating through it. |
| 5. Bounded residual session count | `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core run_plan_churn_guard_aborts_after_three_assignment_waves_without_completion -- --nocapture` passed with `1 passed`, exit `0`. The pinned bound is `initial_sessions + concurrency_limit * MAX_WAVES_WITHOUT_COMPLETION`; fixture values assert `1 + 2 * 3 = 7`. |
| 6. R04 -> R05B dead-PID chain | `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core dead_pid_sweep_then_salvage_requeues_once_without_duplicate_assignment -- --nocapture` passed with `1 passed`, exit `0`. The chain persists a dead PID session, marks the member `crashed`, requeues exactly once, notifies the coordinator, leaves no duplicate assignment, and preserves/appends history. |

### Broader validation record

Commands were run offline and without `--update`.

| Command | Result |
|---|---|
| `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-plan --lib` | exit `0`; `79 passed`, `0 failed`. |
| `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core server::comm_control::tests` | exit `0`; `81 passed`, `0 failed`. |
| `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core server::swarm::tests` | exit `0`; `30 passed`, `0 failed`. |
| `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core swarm_verbs::tests` | exit `0`; `12 passed`, `0 failed`. |
| `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh test -p jcode-app-core tool::communicate::tests` | exit `0`; `87 passed`, `0 failed`. |
| `CARGO_NET_OFFLINE=true bash scripts/dev_cargo.sh check -p jcode-app-core -p jcode-plan` | exit `0`; warning budget issue fixed by `c82de8b3f`. |
| `nix develop -c bash -c 'rustfmt --edition 2024 --check <10 W2 touched files>'` | exit `0`; exact scoped rustfmt check passed. Full `cargo fmt --all -- --check` remains expected-red from unrelated current-tree files such as `jcode-base/src/subscription_api.rs` and `jcode-build-support/src/tests.rs`; no out-of-scope rustfmt changes were kept. |
| `bash scripts/git-hooks/pre-commit` | exit `0`. |
| `git diff --check` | exit `0`. |
| `git status --short` before ledger edit | empty. |

Validation command-shape notes retained for audit: an early two-filter cargo test
attempt failed because of command shape and was rerun individually; an early
app-core filter for `reclaim_stranded_assignment_releases_owner_and_counts_reclaims`
matched zero tests because the fixture belongs to `jcode-plan`; an early
`nix develop -c python3 scripts/check_dependency_boundaries.py` failed because the
flake shell does not expose `python3` by that name. Each was corrected and rerun
with the passing commands above.

### R09 matrix and per-file attribution

No R09 command used `--update`.

| Gate | Result |
|---|---|
| Classifier unit tests | `python3 -m unittest discover -s tests -p test_rust_production_filter.py` passed `17` tests, exit `0`. |
| Classifier compile | `python3 -m py_compile scripts/rust_production_filter.py scripts/check_panic_budget.py scripts/check_swallowed_error_budget.py tests/test_rust_production_filter.py` exit `0`. |
| Dependency boundaries | `nix develop -c bash -c '/Library/Developer/CommandLineTools/usr/bin/python3 scripts/check_dependency_boundaries.py'` passed, exit `0`. |
| Wildcard re-export budget | `python3 scripts/check_wildcard_reexport_budget.py` passed at total `16`, exit `0`. |
| Warning budget | `nix develop -c bash scripts/check_warning_budget.sh` passed at `current=0 baseline=0`, exit `0`. |
| Panic budget | Expected red: `31 -> 48`, exit `1`. W2 attribution: new production panic-prone usage in `crates/jcode-app-core/src/server/comm_session.rs` (`2`) and `crates/jcode-app-core/src/tool/communicate.rs` (`1`). |
| Swallowed-error budget | Expected red: `2987 -> 3074`, exit `1`. W2 attribution: `crates/jcode-app-core/src/server/comm_session.rs` grew `22 -> 24`; `crates/jcode-app-core/src/server/swarm.rs` grew `20 -> 21`. |
| Production size budget | Expected red, exit `1`. W2-owned production-size growth: `comm_control.rs` `2729 -> 2739`, `comm_session.rs` `1341 -> 1615`, `swarm.rs` `3138 -> 3584`, `tool/communicate.rs` `3178 -> 3427`. |
| Test size budget | Expected red, exit `1`. W2-owned test-size growth: new oversized `comm_session_tests.rs` `1264`; `tool/communicate_tests.rs` `1639 -> 2063`. The red `comm_control_tests/dag_e2e.rs` entry is pre-existing/current-tree debt not modified by this W2 branch. |

Changed W2 paths not listed by a specific panic/swallowed/size gate remain within
that gate's current budget output for this branch: `comm_control_tests.rs`,
`comm_control_tests/assign_task.rs`, `swarm_verbs.rs`, and `jcode-plan/src/lib.rs`.

### Invariant evidence and residual scope

- R04 status vocabulary was not changed. Dead status remains the existing triple
  `failed | stopped | crashed`; R05B now consumes one central predicate rather
  than copying the vocabulary into assign/retry paths.
- Durable schema was not widened. History preservation is implemented by appending
  checkpoint provenance via `jcode_plan::append_progress_provenance` and by
  preserving existing progress fields instead of adding a new field.
- Spawn behavior is offline-fixtured only. No live daemon, terminal spawn, network,
  credentials, MCP tools, reload, publication, parent integration, stash/ref move,
  or baseline update was performed.
- Session churn bound is explicit and tested: `initial_sessions + concurrency_limit * MAX_WAVES_WITHOUT_COMPLETION`.
- Remaining red R09 debt is visible by design and not hidden by baseline movement;
  the W2-owned growth above is the owning-seam attribution for this slice.

## 2026-07-16 W2 independent review failure amendment

Independent incident-focused Grok review of fixed W2 HEAD `2f4dfd7d2ff1e08cd18a6ea34f06f3be171719b1` returned **FAIL**. The byte-preserved review is [`../../reviews/2026-07-15-w2-grok-review.md`](../../reviews/2026-07-15-w2-grok-review.md), SHA-256 `6b3df2d04cc0ca7e7756def3643836e8590e1a51a25de5d054a6ecd8131413ae`.

The reviewer found two HIGH proof gaps while confirming the other focused fixes and tests: Auto fallback was not proven across the full handler response/event/detail path and its detail could be overwritten by initial-prompt status; the churn fixture tested the bound helper rather than a full configured-concurrency churn-to-abort sequence with an explicit retained-or-cleaned residue policy. The earlier W2 closure claim is superseded: R05B remains blocked pending separate remediation commits, deterministic full-path fixtures, and fresh independent review.

The first Fable refresh attempt failed at the Anthropic API and produced no artifact or verdict. That infrastructure failure is recorded here and will be retried only after the two Grok findings are remediated.

## 2026-07-16 W2 Grok HIGH-gap remediation amendment

This append-only amendment preserves the 2026-07-16 independent Grok **FAIL** above. It records the separate remediation commits for exactly the two HIGH proof gaps identified in that review. It is not live swarm, terminal, daemon, network, credential, MCP, reload, publication, parent-integration, or baseline-update authorization.

### Remediation commits

All commits include `Co-authored-by: agent <agent@rudnik.online>`.

| Order | Commit | Purpose | Changed paths |
|---:|---|---|---|
| 1 | `2a5beea61` | `fix: expose auto spawn fallback path` | `crates/jcode-app-core/src/server/comm_control.rs`; `crates/jcode-app-core/src/server/comm_session.rs`; `crates/jcode-app-core/src/server/comm_session_tests.rs`; `crates/jcode-app-core/src/server/swarm_mutation_state.rs`; `crates/jcode-app-core/src/server/swarm_mutation_state_tests.rs`; `crates/jcode-app-core/src/tool/communicate.rs`; `crates/jcode-protocol/src/wire.rs` |
| 2 | `6115daa39` | `test: prove run_plan churn abort bound` | `crates/jcode-app-core/src/tool/communicate.rs`; `crates/jcode-app-core/src/tool/communicate_tests.rs`; `crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs` |

### HIGH gap 1 closure: Auto fallback full-path observability

`2a5beea61` adds response metadata for Auto fallback without changing required fields or removing compatibility defaults: `requested_spawn_mode`, `resolved_spawn_mode`, and `spawn_fallback_detail` are optional `CommSpawnResponse` fields. The handler-level fixture `handle_comm_spawn_auto_fallback_preserves_response_event_and_detail_with_prompt` injects deterministic visible failure through `JCODE_TEST_VISIBLE_SPAWN_ERROR`, drives `handle_comm_spawn`, and asserts:

- response/event path carries `requested_spawn_mode = Auto`, `resolved_spawn_mode = Headless`, and fallback detail containing the original visible error;
- persisted mutation replay carries the same optional response fields;
- member detail preserves `requested Auto -> resolved Headless` even when an initial prompt runs and status updates would otherwise overwrite detail;
- the swarm debug event stream records the fallback-created member join.

Focused validation after the fix:

| Command | Result |
|---|---|
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core handle_comm_spawn_auto_fallback_preserves_response_event_and_detail_with_prompt -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |

### HIGH gap 2 closure: full configured-concurrency churn-to-abort and residue policy

`6115daa39` adds explicit residue-policy text to the churn abort diagnostic and a full offline `run_plan` fixture, `communicate_run_plan_churns_to_abort_at_configured_concurrency_and_cleans_failed_workers`. The fixture runs through the tool/server path with configured concurrency `2`, six independent plan nodes, and a deterministic test provider that fails before prompt completion. It asserts:

- provider call count, used as actual created-session count, is exactly `configured_concurrency * 3 = 6`;
- the abort diagnostic states `run_plan aborted after 3 consecutive assignment wave(s)` and includes the residue policy;
- default `run_plan` error cleanup leaves zero retained failed workers; `retain_agents=true` is documented as the inspection-retention mode.

Focused validation after the fix and rustfmt amend:

| Command | Result |
|---|---|
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core communicate_run_plan_churns_to_abort_at_configured_concurrency_and_cleans_failed_workers -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |

### Re-run W2/R05A validation matrix

Commands were run offline and without `--update` after the two source/test fixes. The churn test was rerun once more after the rustfmt-only amend to commit `6115daa39` and remained green as recorded above.

| Command | Result |
|---|---|
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core handle_comm_spawn_auto_fallback_preserves_response_event_and_detail_with_prompt -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core communicate_run_plan_churns_to_abort_at_configured_concurrency_and_cleans_failed_workers -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core visible_launch -- --nocapture --test-threads=1` | exit `0`; `2 passed`, `0 failed`, `1100 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core assign_task_stale_direct_takeover_preserves_progress_history -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-plan reclaim_stranded_assignment_releases_owner_and_counts_reclaims -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `78 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core salvage_requeues_dead_members_tasks_and_notifies_coordinator -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core salvage_fails_task_once_reclaim_cap_is_reached -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core member_status_is_dead_matches_terminal_non_success_states -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core f1_assign_next_reclaims_task_from_departed_assignee -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core failed_instance_needs_retry -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core dead_pid_sweep_then_salvage_requeues_once_without_duplicate_assignment -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core control_log_fold_tracks_maps_through_handler_sequence -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh test -p jcode-app-core scan_from_tail_offset_finds_artifact_once -- --nocapture --test-threads=1` | exit `0`; `1 passed`, `0 failed`, `1101 filtered out`. |
| `CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/jcode-w2-r05b-target bash scripts/dev_cargo.sh check -p jcode-app-core -p jcode-protocol -p jcode-plan` | exit `0`; `Finished dev profile` in `46.02s`. |

### R09, format, hook, and diff checks

No R09 baseline command used `--update`.

| Gate | Result |
|---|---|
| `python3 -m unittest discover -s tests -p test_rust_production_filter.py` | exit `0`; `Ran 17 tests`, `OK`. |
| `python3 -m py_compile scripts/rust_production_filter.py scripts/check_panic_budget.py scripts/check_swallowed_error_budget.py tests/test_rust_production_filter.py` | exit `0`. |
| `python3 scripts/check_wildcard_reexport_budget.py` | exit `0`; `wildcard re-export budget check passed (total=16)`. |
| `python3 scripts/check_panic_budget.py` | expected red exit `1`; total `31 -> 48`; W2-attributed entries still include `crates/jcode-app-core/src/server/comm_session.rs (2)` and `crates/jcode-app-core/src/tool/communicate.rs (1)`. |
| `python3 scripts/check_swallowed_error_budget.py` | expected red exit `1`; total `2987 -> 3074`; W2-attributed entries still include `crates/jcode-app-core/src/server/comm_session.rs 22 -> 24` and prior W2 `crates/jcode-app-core/src/server/swarm.rs 20 -> 21`. |
| `python3 scripts/check_code_size_budget.py` | expected red exit `1`; W2-owned growth still includes `comm_control.rs 2729 -> 2739`, `comm_session.rs 1341 -> 1694`, `swarm.rs 3138 -> 3584`, and `tool/communicate.rs 3178 -> 3436`. |
| `python3 scripts/check_test_size_budget.py` | expected red exit `1`; W2-owned growth still includes new oversized `comm_session_tests.rs 1403` and `tool/communicate_tests.rs 1639 -> 2096`; pre-existing/current-tree `comm_control_tests/dag_e2e.rs` remains expected red and was not modified by these two remediation commits. |
| `nix develop -c bash -c '/Library/Developer/CommandLineTools/usr/bin/python3 scripts/check_dependency_boundaries.py'` | exit `0`; `dependency boundary check passed`. |
| `nix develop -c bash scripts/check_warning_budget.sh` | exit `0`; `Warning budget OK: current=0 baseline=0`. |
| `nix develop -c bash -c 'rustfmt --edition 2024 --check <9 remediation-touched Rust files>'` | exit `0` after amending `6115daa39` with exact-file rustfmt cleanup. |
| `bash scripts/git-hooks/pre-commit` | exit `0`. |
| `git diff --check` | exit `0`. |

### Scope, authority, and remaining limits

- The earlier Grok FAIL remains preserved above and is not rewritten.
- Optional response fields were added only to `CommSpawnResponse` with serde defaults/skip-if-none for wire compatibility; existing required fields remain unchanged.
- No durable task-progress schema widening was introduced.
- No live terminal, live daemon, live swarm, network, credentials, MCP/tool invocation, reload, publication, stash/ref/worktree move, parent integration, or baseline update was performed.
- Independent review after this remediation is still unverified in this amendment.

## 2026-07-16 W2 protocol-scope adjudication amendment

Independent Opus scope adjudication of remediation HEAD `a342cd5fbe6c0185b486577e59996acc94770b8e` returned **FAIL (scope/governance)** with high confidence. The byte-preserved report is [`../../reviews/2026-07-16-w2-scope-adjudication.md`](../../reviews/2026-07-16-w2-scope-adjudication.md), actual file SHA-256 `b44b7acd0324a4fe76bf1696f4d44792b56832396b7c00884ef3a3b1e3be9a2b`.

The optional response fields are statically serde-backward-compatible and their constructors, persistence/replay path, and consumers were found technically coherent. They are nevertheless a wire change in `crates/jcode-protocol` and a durable mutation-replay schema widening outside W2's declared surface. This triggers the preserved slice-1 stop condition and R03A protocol-bump governance. The earlier remediation-closure claim is superseded: current W2 HEAD must not proceed to behavioral rereview or integration as an in-scope R05B package.

W2 is paused at an explicit decision boundary. Either obtain R03A/user authorization for the isolated compatibility proposal and targeted wire surface, or remove the protocol/replay response-field widening, retain only in-scope event/detail observability, and leave the response leg deferred under R03A. The churn-to-abort remediation is not rejected by this scope review, but remains unintegrated with the rest of W2.

## 2026-07-16 low-friction scope-repair amendment

The operator selected the second path above: remove response/replay widening
now, preserve R05B-owned safety and internal observability, and defer any future
response metadata to R03A-governed work after the independent basics are
working.

### Repair commits

| Commit | Class | Result |
|---|---|---|
| `6dfe2cdb6` | `fix` | Removes `requested_spawn_mode`, `resolved_spawn_mode`, and `spawn_fallback_detail` from `CommSpawnResponse` and `PersistedSwarmMutationResponse::Spawn`; restores the spawn result to the pre-widening session-ID shape; retains explicit `Visible` failure, `Auto` fallback, member detail, status preservation, join history, and churn/reclaim behavior. |
| `f13620596` | `test` | Removes assertions that required the widened response/replay fields and keeps handler-level proof that the fallback reason survives initial-prompt status changes in member detail while the fallback-created join is recorded in history and the swarm event stream. |

`PROTOCOL_VERSION` remains `1`. A repository search after both commits finds no
`requested_spawn_mode`, `spawn_fallback_detail`, or `SwarmSpawnOutcome` symbol
under `crates/`. The final changed-path set versus W2 base no longer includes
`crates/jcode-protocol/src/wire.rs` or
`server/swarm_mutation_state{,_tests}.rs`; those files are restored to their
pre-W2 content.

### Preserved failed validation attempt

The first post-repair fixture attempted to prove delivery of an automatic
`SwarmStatus` snapshot to the coordinator helper's unregistered attachment
receiver. It timed out after five seconds: `0 passed; 1 failed; 1101 filtered
out`. That expectation exceeded the bounded low-friction contract and did not
identify a spawn/reclaim source regression. The failed attempt remains part of
the recovery history rather than being represented as a pass.

The fixture was narrowed only by removing that new delivery assertion. It
continues to prove the existing R05B-owned observables: fallback text in member
detail survives running/failed status updates, the member is headless, the
initial prompt is marked delivered, join history is present, and the live swarm
event stream records the join.

### Offline validation rerun

All commands set `FORK_NUDGE_MAX_AGE=2147483647`,
`FORK_NUDGE_AUTOSYNC=0`, `CARGO_NET_OFFLINE=true`, and
`CARGO_INCREMENTAL=0`, entered `nix develop --offline`, and used no
`--update`.

| Check | Result |
|---|---|
| Auto fallback handler history/detail | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| Configured-concurrency churn-to-abort | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| Explicit Visible fail-closed | `2 passed`, `0 failed`, `1100 filtered`; exit `0` |
| Stale takeover history | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| Primitive reclaim history | `1 passed`, `0 failed`, `78 filtered`; exit `0` |
| Salvage requeue history | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| Salvage cap history | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| Central liveness authority | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| Departed-assignee reclaim | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| Dead-instance retry | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| R04-to-R05B dead-PID chain | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| R05A control-log entry fixture | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| R05A tail-scan entry fixture | `1 passed`, `0 failed`, `1101 filtered`; exit `0` |
| `check -p jcode-app-core -p jcode-protocol -p jcode-plan` | exit `0`; dev profile finished |

Targeted Rust formatting, `git diff --check`, and both pre-commit checks passed.
The filtered app-core test builds continue to print the pre-existing
`drop_control_log_handle` test-only dead-code warning; package `check` is clean.

This amendment authorizes no live swarm, terminal, daemon, network, credential,
MCP/tool, reload, publication, release, baseline update, or parent integration.
Fresh independent correctness and scope reviews are still required.

## 2026-07-16 W2 final independent review and evidence closure amendment

The low-friction repair at fixed review HEAD `f8c5f8204056ff783d99769e4088e7bcceb56d73` received both required fresh independent reviews:

- correctness/behavioral review: **PASS**, preserved byte-for-byte as [`../../reviews/2026-07-16-w2-low-friction-grok-rereview.md`](../../reviews/2026-07-16-w2-low-friction-grok-rereview.md), SHA-256 `53f53949901ff7d91e3eaafe10bb2e6553f506fc5fe6da1e17fcd0030f81b384`;
- scope re-adjudication: **PASS (HIGH scope confidence)**, preserved byte-for-byte as [`../../reviews/2026-07-16-w2-low-friction-fable-scope-rereview.md`](../../reviews/2026-07-16-w2-low-friction-fable-scope-rereview.md), SHA-256 `5c775609d17f2e851810001aa9ab1fd747f1ba2fd928252ef3a9fc11d33b2607`.

The correctness review reran the 13 focused W2/R05B and R05A fixture commands plus the three-package offline check; all passed. The scope review verified blob identity to base for `wire.rs` and `swarm_mutation_state{,_tests}.rs`, `PROTOCOL_VERSION = 1`, zero removed-symbol hits under `crates/`, zero residual protocol/durable-schema crossing, append-only ledger history, the preserved failed attempt, and the successful 14-check log. Its MEDIUM durability finding is resolved by committing [`../../evidence/2026-07-16-w2-scope-repair/`](../../evidence/2026-07-16-w2-scope-repair/) with its SHA-256 manifest.

`crates/jcode-app-core/src/swarm_verbs.rs` is outside the recovery plan's literal `server/swarm*.rs` path shorthand but is explicitly R05B-owned for fixture 4: it contained the third copied dead-status predicate identified by this ledger, and the semantics-preserving centralization is required to prove verb/report and server paths use one liveness authority. This is a ledger-sanctioned surface clarification, not a new cross-seam expansion.

The fresh Grok review satisfies the behavioral-review requirement that the parallel Fable scope report still described as pending. W2/R05B is therefore closed for offline source integration. This does not authorize swarm widening or a live swarm pilot. Response-leg requested/resolved/fallback metadata remains deferred to R03A/future fork-owned API governance.
