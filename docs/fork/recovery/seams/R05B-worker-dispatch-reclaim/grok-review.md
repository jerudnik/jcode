# R05B adversarial Grok-style full review

Date: 2026-07-15  
Worktree: `/Users/jrudnik/labs/jcode-seam-r05b`  
Reviewed commit: `5baf343ba6da564afc3f6c58c5edca7a64d6e67f`  
Fixed refs: fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`  
Constraint honored: I did not read `/tmp/jcode-r05b-opus-review.md`. I also did not mutate the repo; outputs were written only under `/tmp`.

## Disposition

**BLOCK R05B approval as a completed full seam.** The fork contains meaningful worker-dispatch hardening, but R05B cannot be accepted as a reviewed/approved seam at `5baf343ba` because:

1. **The required R05B seam ledger is absent.** `docs/fork/recovery/README.md:29` says a full review seam contains `opus-review.md`, `grok-review.md`, and `ledger.md`; `docs/fork/recovery/PROGRESS.md:12`, `:31`, and `:35` state R05B remains pending. `find docs/fork/recovery/seams -maxdepth 1 -type d -name 'R05*' -print` produced no R05 directories.
2. **Explicit visible spawn authority is not honored.** `spawn_swarm_agent` treats `Visible` and `Auto` identically for fallback: any visible launch `Err` or `Ok(false)` falls through to `create_headless_session` (`comm_session.rs:619-653`, `:659-691`). This directly violates R05B's ownership of "headless/visible authority" (`RESPONSIBILITIES.md:28`) and the incident's missing spawn-mode authority (`PRESCREEN.md:117`).
3. **A direct stale `assign_task` takeover can erase task progress history.** The active-assignment guard deliberately allows stale assigned work (`comm_control.rs:194-199`, `:1668-1683`), but the successful assignment path replaces the entire `SwarmTaskProgress` record with a defaulted struct (`comm_control.rs:1724-1739`). That is inconsistent with the cross-seam invariant that reclaim cannot erase history (`RESPONSIBILITIES.md:67`) and with the plan-level reclaim helper that explicitly preserves prior heartbeat history (`jcode-plan/src/lib.rs:618-640`, tested at `:1104-1150`).

**Pilot relevance:** R05B is **not a prerequisite for the smallest no-swarm pilot** unless the chosen pilot exercises swarm-driven worker dispatch. `RESPONSIBILITIES.md:86` says R05B is not a prerequisite unless the stack exercises it. If the pilot uses swarm/run_plan/spawned workers, this review blocks pilot entry until the visible-spawn fallback and history-preservation fixtures are fixed or explicitly scoped out.

## Eight decisive checkpoints

### 1. Responsibility and boundary checkpoint

Supported.

- R05B owns assignment dispatch, headless/visible authority, dead-worker detection, bounded reclaim, retry limits, session-growth containment, and observable failure (`RESPONSIBILITIES.md:28`).
- R04 owns process/task lifecycle and terminal state, excluding worker assignment/reclaim policy (`RESPONSIBILITIES.md:26`).
- R05A owns DAG/control-log semantics and excludes process spawn, worker health, and render state (`RESPONSIBILITIES.md:27`).
- The liveness invariant explicitly splits authority: R04 owns process/task life and terminal state; R05B owns assignment, reclaim, and retry policy; dead process must not cause unbounded assignment/session-file growth, and reclaim cannot erase history (`RESPONSIBILITIES.md:67`).
- Mapper evidence matches this split: R05B owns run-plan assignment dispatch, worker spawn mode, dead-worker detection, failure scoreboard, reclaims, retry limits, and bounded backoff (`reviews/2026-07-15-responsibility-mapper-luna.md:173-178`).

### 2. Incident and maintenance evidence checkpoint

Supported and severe.

- The recorded R05 incident: a six-node run-plan completed zero nodes, emitted about 76 assignments in two minutes, and created 190 session files; terminal-backed workers died before prompts, while explicit headless spawns succeeded (`PRESCREEN.md:117`).
- The incident implicated missing failure backoff and spawn-mode authority (`PRESCREEN.md:117`).
- Separate preservation evidence shows stale legacy worker stopping reclaimed three clean orchestrator worktrees and branch refs, repaired non-destructively (`BASELINES.md:226-243`). This is not R05B behavior directly, but it raises the cost of unsafe session cleanup and supports R11/R00 caution.

### 3. Assignment dispatch and double-assignment checkpoint

Mostly supported.

- Automatic candidate filtering excludes the requester, non-agent roles, wrong swarms, non-ready/completed statuses, and non-drivable workers (`comm_control.rs:52-67`).
- Drivable auto workers are headless or report back to the requester (`comm_control.rs:69-73`), which is tested in `auto_worker_filter.rs:17-57`.
- Active assignment load is counted from non-terminal items (`jcode-plan/src/lib.rs:556-566`) and auto-pick skips busy workers (`comm_control.rs:160-177`).
- Race claims prevent concurrent auto-picks from choosing the same worker during the pre-plan-write window (`comm_control.rs:86-127`), with a 15s claim TTL (`comm_control.rs:86-89`, `:119`).
- Direct `assign_task` rejects a named task when it is already assigned and active within the stale window (`comm_control.rs:179-236`, `:1660-1687`).
- Existing fixtures cover pure conflict logic and handler-level rejection (`assign_double.rs:1-5`, `:188-263`), but I did not complete app-core execution because the cold offline app-core build timed out.

**Residual risk:** stale assigned items are allowed for takeover, and that takeover path can erase progress history. See finding F2.

### 4. Spawn mode authority checkpoint

**Failed. Blocker F1.**

- The parser accepts only visible/headless/inline/auto and errors on invalid strings (`client_lightweight_control.rs:31-53`).
- The handler passes parsed `spawn_mode` into `handle_comm_spawn` (`client_lifecycle.rs:2271-2287`, `client_lightweight_control.rs:315-331`).
- But `spawn_swarm_agent` resolves `spawn_mode.unwrap_or(config.agents.swarm_spawn_mode)` and then treats `Visible` and `Auto` the same for fallback (`comm_session.rs:579`, `:619-625`).
- If visible launch returns `Ok(false)` or `Err(_)`, the code creates a headless session (`comm_session.rs:651-691`) without preserving the visible error in the user-facing response.
- The explicit `handle_comm_spawn` error path only sees errors after both visible and headless fail (`comm_session.rs:837-944`). Thus an explicit visible request can silently become headless.

This is not a theoretical style issue. The R05 incident specifically says terminal-backed workers died before prompts and explicit headless spawns succeeded (`PRESCREEN.md:117`). If `visible` is requested for operator-visible terminals, silent fallback changes semantics and hides the failure class R05B must expose.

### 5. Health/liveness, dead-worker reclaim, and retry/backoff checkpoint

Partly supported, with one gap.

Supported mechanisms:

- `member_status_is_dead` treats `failed`, `stopped`, and `crashed` as dead (`swarm.rs:369-374`).
- Dead-member salvage requeues non-terminal assignments or marks them failed after `MAX_DEAD_ASSIGNEE_RECLAIMS` (`swarm.rs:416-472`).
- Salvage persists state, broadcasts the plan, and notifies the coordinator (`swarm.rs:475-531`, `:534-570`).
- `MAX_DEAD_ASSIGNEE_RECLAIMS` is 3 (`jcode-plan/src/lib.rs:581-584`); `next_stranded_runnable_item_id` excludes items at or above the cap (`jcode-plan/src/lib.rs:586-615`).
- `reclaim_stranded_assignment` clears only the assignment binding, increments reclaim count, sets a checkpoint summary, and preserves prior progress fields (`jcode-plan/src/lib.rs:618-640`).
- `refresh_swarm_task_staleness` marks old running tasks `running_stale`, revives them on fresh heartbeat, fails orphaned stale tasks past reap deadline, then salvages assigned work whose assignee is dead or gone (`swarm.rs:647-730`, `:755-806`).
- Dead PID sweep opportunistically reconciles Active sessions and mirrors crashed status into swarm member state (`swarm.rs:242-322`), and is triggered from `broadcast_swarm_status` at most once per interval (`swarm.rs:877-890`).

Gap:

- I did not see a deterministic fixture covering the full visible-process-death path from persisted dead PID to assignment salvage and coordinator notification in one test. There is a unit test that marks a dead visible member crashed (`swarm.rs:2029-2053`) and separate salvage tests (`swarm.rs:3054-3140`), but not the whole chain.

### 6. Reuse, zombie reuse, and session growth checkpoint

Mostly supported, but depends on the spawn-mode blocker.

- Foreign unowned sessions, even with no attachment, are excluded from auto assignment as possible zombies (`auto_worker_filter.rs:46-57`).
- Owned workers and headless workers are reusable (`auto_worker_filter.rs:17-34`, `:59-99`).
- Busy workers are skipped, and the stable no-target error is intentionally used by `spawn_if_needed`/`run_plan` to spawn a fresh worker instead of stacking (`comm_control.rs:136-177`; `assign_busy_skip.rs:1-10`, `:42-56`, `:101-193`).
- Total live member capacity is capped by `MAX_SWARM_MEMBERS` (`swarm.rs:26-32`) and enforced before spawn (`comm_session.rs:1434-1502`). Terminal members do not consume capacity (`swarm.rs:343-355`).
- Event history is capped at `MAX_EVENT_HISTORY = 5000` and old events are popped (`state.rs:381`, `swarm.rs:1418-1441`).

Residual risk:

- Explicit visible spawn fallback can still create headless sessions when the requested visible path is failing, so session-growth containment is not decisive for the incident class until F1 is fixed and tested.

### 7. Terminal failure visibility checkpoint

Partly supported.

- Spawn failure after both visible and headless fail is user-visible as `Failed to spawn agent: ...` (`comm_session.rs:944`).
- `assign_next` spawn failure is user-visible as `Failed to spawn preferred worker: ...` (`comm_control.rs:2072-2078`).
- Dead-member salvage notification names requeued and failed task ids and tells the coordinator to check plan status (`swarm.rs:390-413`, `:557-570`).
- Orphaned stale task reaping logs a warning and marks the item failed (`swarm.rs:692-725`).

But explicit visible-launch failure is swallowed by headless fallback. Therefore terminal failure visibility fails for that mode.

### 8. Ref comparison checkpoint

Compared without treating upstream as authority.

- `git diff --stat upstream..fork` for scoped code shows fork adds substantial R05B code relative to upstream: `comm_control.rs` +120 lines, `comm_session.rs` +327 lines, `swarm.rs` +467 lines.
- `git diff --stat merge-base..fork` shows similar fork-side growth: `comm_control.rs` +120, `comm_session.rs` +335, `swarm.rs` +608.
- `HEAD` vs fixed fork changes are docs only for the scoped code: `docs/fork/recovery/RESPONSIBILITIES.md` 125 changed lines; no scoped source changes since fork in this worktree.
- Fork-specific diff hunks include stale assignment reclaim in `comm_control.rs`, spawn selection/fallback/model-route changes in `comm_session.rs`, and dead PID sweep/reaper/salvage tests in `swarm.rs`.

## Writers and timers for assigned/running/ready/dead/reclaimed state

### Assignment and task-status writers

- Plan item `assigned_to` and `queued`: `handle_comm_assign_task_with_mode` writes `assigned_to = Some(target_session)` and `status = "queued"`, then inserts a fresh `SwarmTaskProgress` (`comm_control.rs:1724-1739`).
- Task run starts: `spawn_assigned_task_run` marks the item `running`, records `started_at_unix_ms`, and assigned session id (`comm_control.rs:860-884`).
- Turn end: task may be marked `done`, requeued, or failed for missing artifacts (`comm_control.rs:1029-1092`) and worker status updated completed/failed (`comm_control.rs:1131-1181`).
- Task control requeue/replace/retry paths update statuses around `comm_control.rs:2147-2640`; task-control requeue has a fixture preserving prior progress (`task_control.rs:381-472`).
- Plan-level reclaim clears assignment and increments reclaim count (`jcode-plan/src/lib.rs:623-640`).
- Dead-member salvage requeues or fails items (`swarm.rs:427-472`).
- Staleness sweep transitions `running` to `running_stale`, revives `running_stale` to `running`, or fails orphaned stale work (`swarm.rs:647-730`).

### Member lifecycle/status writers

- Headless fallback startup marks member `running`, then `ready` or `failed` after `process_message_streaming_mpsc` (`comm_session.rs:753-824`).
- Assignment dispatch marks target member `queued` before sending prompt (`comm_control.rs:1841-1848`).
- Task run marks member `running` and later `completed`/`failed` (`comm_control.rs:914-936`, `:1179-1181`).
- Dead PID sweep marks member `crashed` with detail `client process exited` (`swarm.rs:302-318`).
- Stop/remove paths mark/remove members and salvage assignments (`swarm.rs:1163-1385`).

### Timers and periodic mechanisms

- Auto-pick claim TTL: 15s (`comm_control.rs:86-89`, `:119`).
- Assignment heartbeat interval: `swarm_task_heartbeat_interval` used in spawned heartbeat loop (`comm_control.rs:935-941`), configured in `swarm.rs:221-226`.
- Task stale threshold: `swarm_task_stale_after` (`swarm.rs:228-233`).
- Task sweep interval: `swarm_task_sweep_interval` (`swarm.rs:235-240`).
- Dead PID sweep interval and claim: `swarm_dead_pid_sweep_interval`, `claim_dead_pid_sweep` (`swarm.rs:242-263`), invoked by status broadcast (`swarm.rs:877-890`).
- Terminal member retention and GC interval: `swarm_terminal_member_retention` and `swarm_terminal_member_gc_interval` (`swarm.rs:324-340`).
- Orphaned stale reap threshold: `swarm_task_reap_after` (`swarm.rs:573-581`).
- Broadcast debounce uses a sleep (`swarm.rs:947-949`).

## Findings

### F1. BLOCKER: explicit `visible` spawn falls back to headless and swallows the visible failure

Evidence:

- R05B explicitly owns headless/visible authority and observable failure (`RESPONSIBILITIES.md:28`).
- Incident evidence says terminal-backed workers died before prompts while explicit headless spawns succeeded (`PRESCREEN.md:117`).
- `SwarmSpawnMode::Visible | SwarmSpawnMode::Auto` both call `prepare_visible_spawn_session` (`comm_session.rs:619-648`).
- `Ok((_, false)) | Err(_)` both proceed to `create_headless_session` (`comm_session.rs:651-691`).
- The user sees a spawn error only if `create_headless_session` also fails (`comm_session.rs:837-944`).

Impact:

- `visible` does not mean visible. A broken terminal spawn can silently become headless.
- The failure mode implicated by the incident is hidden rather than surfaced.
- Mode fallback ambiguity makes run-plan incident reproduction misleading.

Cheapest fixture:

- Unit-test `spawn_swarm_agent` with `spawn_mode = Visible` and an injected visible launcher returning `Err("terminal failed")` or `Ok(false)`. Assert no headless session is created and the response contains the visible failure.
- Separate test for `Auto` may allow fallback, but must record mode `auto -> headless fallback` visibly in the member detail/event/response.

### F2. IMPORTANT: stale direct `assign_task` can erase task progress history

Evidence:

- Direct assignment rejects only fresh active assignments; stale assigned work is allowed (`comm_control.rs:194-199`, `:1668-1683`).
- Successful assignment writes `plan.task_progress.insert(... SwarmTaskProgress { assigned_session_id, assignment_summary, assigned_at_unix_ms, ..Default::default() })` (`comm_control.rs:1727-1738`), replacing any prior progress record.
- Plan-level reclaim, by contrast, explicitly preserves prior history and only clears assignment binding (`jcode-plan/src/lib.rs:618-640`), with tests asserting prior heartbeat preservation (`jcode-plan/src/lib.rs:1104-1150`).
- The cross-seam invariant says reclaim cannot erase history (`RESPONSIBILITIES.md:67`), and R05B owns reclaim/retry policy.

Impact:

- A stale takeover can lose `last_heartbeat_unix_ms`, `last_detail`, checkpoint counts, `dead_assignee_reclaims`, and prior summaries.
- This makes post-incident diagnosis and failure backoff less reliable.

Cheapest fixture:

- Seed a plan item `running_stale` assigned to `old`, with progress containing `last_heartbeat_unix_ms`, `last_detail`, and `dead_assignee_reclaims`.
- Call direct `assign_task(task_id, target_session = new)` through the handler.
- Assert assignment changes to `new` but prior progress fields are preserved or intentionally appended, not defaulted away.

### F3. PROCESS BLOCKER: R05B has no committed seam ledger/review set

Evidence:

- Full seams require two independent seam reviews and adjudication (`RESPONSIBILITIES.md:15`) and the README says a full seam directory contains `opus-review.md`, `grok-review.md`, and `ledger.md` (`README.md:29`).
- `PROGRESS.md:12`, `:31`, and `:35` say R05B remains pending.
- `find docs/fork/recovery/seams -maxdepth 1 -type d -name 'R05*' -print` returned no directories.

Impact:

- Even if code were perfect, R05B is not yet an approved full seam in the repository.

Cheapest fixture:

- Land R05B ledger plus two independent reviews and adjudication, including the red-debt assignment required by R09.

### F4. GAP: full process-truth to assignment-salvage chain is not covered by one deterministic fixture

Evidence:

- Dead PID sweep marks crashed visible members (`swarm.rs:265-322`) and has a test (`swarm.rs:2029-2053`).
- Salvage requeues/fails tasks and notifies coordinator (`swarm.rs:416-531`) and has direct tests (`swarm.rs:3054-3140`).
- The chain from dead process -> crashed member -> stale/dead assignment salvage -> notification is split across mechanisms and was not run end-to-end here.

Impact:

- Control-log/member-map truth and OS process truth can drift long enough to affect assignment/reuse decisions, especially around visible worker death.

Cheapest fixture:

- Use a persisted session with a dead PID, a swarm member assigned to a running task, and a plan with progress. Invoke the daemon-side sweep/status path plus task staleness refresh. Assert member `crashed`, task requeued or failed by cap, coordinator notification, and no duplicate assignment.

## Negative findings that survived adversarial checks

These are not blockers from the source evidence I saw:

- **Unbounded automatic dead-worker retries:** bounded by `MAX_DEAD_ASSIGNEE_RECLAIMS = 3` and excluded at cap (`jcode-plan/src/lib.rs:581-615`). Plan tests passed for this.
- **Concurrent auto-pick double assignment:** in-process claims plus plan loads address the pre-plan-write window (`comm_control.rs:86-177`). Existing tests target this (`assign_busy_skip.rs:58-83`). I did not complete app-core execution due timeout.
- **Foreign zombie reuse:** auto-filter excludes unowned sessions even if they have no attachment (`auto_worker_filter.rs:46-57`).
- **Unbounded member/event growth:** `MAX_SWARM_MEMBERS` caps live capacity (`swarm.rs:26-32`, `comm_session.rs:1488-1502`), terminal members are capacity-free (`swarm.rs:343-355`), and event history is capped (`state.rs:381`, `swarm.rs:1437-1441`).
- **Silent dead-member salvage:** salvage emits lifecycle logs, persists state, broadcasts the plan, and notifies the coordinator (`swarm.rs:498-531`, `:557-570`).

## R09 debt

R09 remains binding. It says red debt stays visible and assigned to owning behavior seams, including worker dispatch to R05B (`R09-quality-gates/ledger.md:28`), and that per-file assignment of production/test-size violations to R02/R04/R05B/R12 has not yet been enumerated (`R09-quality-gates/ledger.md:49`).

R05B therefore owes before implementation gate:

- its share of production-size/test-size violations for `comm_control.rs`, `comm_session.rs`, `swarm.rs`, and related tests;
- any panic/swallowed-error debt introduced in worker dispatch/spawn/reclaim paths;
- no `--update` baseline move. R09 forbids hiding current red debt (`R09-quality-gates/ledger.md:27-30`).

## Exact commands and results

| Command | Result |
|---|---|
| `pwd; git rev-parse HEAD; git status --short; git rev-parse 5baf343ba 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` | Worktree `/Users/jrudnik/labs/jcode-seam-r05b`; HEAD `5baf343ba6da564afc3f6c58c5edca7a64d6e67f`; all fixed refs resolved. |
| `find docs/fork/recovery -maxdepth 5 -type f ...` | Found `RESPONSIBILITIES.md`, `PROGRESS.md`, R00/R09/R11 ledgers, incident docs, no R05B seam directory. |
| `grep -R "R05B\|spawn storm\|run_plan..." -n docs/fork/recovery docs/architecture` | Located R05 incident at `PRESCREEN.md:117`, R05B responsibility at `RESPONSIBILITIES.md:28`, invariant at `:67`, pending status at `PROGRESS.md:12/:31/:35`. |
| `git diff --stat 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4..5baf343ba -- scoped files` | Scoped source unchanged since fork; docs changed. |
| `git diff --stat 802f6909825809e882d9c2d575b7e478dce57d3b..7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 -- comm_control.rs comm_session.rs swarm.rs jcode-plan/src/lib.rs` | Fork differs materially from upstream: `comm_control.rs` 120 lines, `comm_session.rs` 327 lines, `swarm.rs` 467 lines. |
| `git diff --stat 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d..7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 -- scoped files` | Fork differs materially from merge base: `comm_control.rs` 120 lines, `comm_session.rs` 335 lines, `swarm.rs` 608 lines. |
| Initial test command with multiple cargo test names | Failed immediately with cargo syntax error: `unexpected argument ...`; no test assertion failure. Output saved in `/tmp/jcode-r05b-test-plan.txt`, `/tmp/jcode-r05b-test-busy.txt`, `/tmp/jcode-r05b-test-double.txt` and background output `247007715m`. |
| Corrected command: `CARGO_TARGET_DIR=/tmp/jcode-r05b-target CARGO_NET_OFFLINE=true JCODE_SCCACHE=off scripts/dev_cargo.sh test --offline -p jcode-plan stranded -- --nocapture` | Passed: `2 passed; 0 failed; 77 filtered out`, including `reclaim_stranded_assignment_releases_owner_and_counts_reclaims` and `stranded_runnable_item_requires_dead_assignee_and_respects_reclaim_cap`. |
| Corrected app-core command: `scripts/dev_cargo.sh test --offline -p jcode-app-core busy_worker ...` followed by `double_assignment ...` in same script | Timed out after 600s compiling `jcode-app-core` dependencies under `/tmp` target. No app-core test assertion result. This is a validation gap, not a code failure. |
| `git status --short` after commands | Empty output; no repo mutations detected. |

Notes on test constraints: I used `/tmp/jcode-r05b-target`, `CARGO_NET_OFFLINE=true`, `--offline`, and no live server/process tests. The `scripts/dev_cargo.sh` wrapper re-entered the Nix dev shell and printed hook messages; this did not dirty the repository.

## Confidence and gaps

Confidence: **medium-high** for the two concrete blockers and the overall `block R05B approval` disposition. The source evidence is direct and line-cited.

Gaps:

- I did not complete app-core targeted tests because the cold offline compile timed out at 600s.
- I did not run live process/terminal spawn tests by instruction.
- I did not inspect `/tmp/jcode-r05b-opus-review.md`.
- I did not exhaustively enumerate all quality-gate red entries by file; R09 says R05B owes that before implementation gate.

## Final recommendation

- **Do not mark R05B approved.**
- **Do not use R05B as a swarm-driven pilot prerequisite until F1 and F2 have fixtures and fixes.**
- **For a no-swarm single-turn pilot, R05B can remain pending** as long as the pilot avoids `run_plan`, swarm spawn/reuse, and dead-worker reclaim paths.
