# R05B full-seam review: Worker dispatch, spawn mode, liveness, reclaim, and failure backoff

| Field | Value |
|---|---|
| Seam | R05B (worker dispatch, headless/visible authority, dead-worker detection, bounded reclaim, retry limits, session-growth containment, observable failure) |
| Reviewer | Opus (independent verify agent), swarm subagent type `verify` |
| Review head | fork `5baf343ba6da564afc3f6c58c5edca7a64d6e67f` (worktree `/Users/jrudnik/labs/jcode-seam-r05b`, branch `recovery/seam-r05b-20260715`) |
| Fixed refs | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `full` (RESPONSIBILITIES.md rank 4, score 15/16) |
| Recommended disposition | **`retain-fork`** (single supported disposition; conditions below) |
| Confidence | high for dispatch/reclaim/liveness/churn behavior and test coverage; medium for the spawn-mode incident causal chain (the incident's *default* mode was already `Inline` at base/upstream, so the fork fix is churn/reclaim, not the default) |
| Working-tree state | clean, zero source/doc/ref edits; `vendor/upstream` still pinned `631935dd1...` |

R05B owns worker assignment dispatch, spawn-mode authority, dead-worker detection, bounded reclaim, retry/backoff limits, session-growth containment, and observable failure. It excludes graph truth (R05A) and generic child-process lifecycle (R04). Per cross-seam invariant 5: R04 owns process/task life and terminal state; R05B owns assignment, reclaim, and retry policy; a dead process must not cause unbounded assignment or session-file growth, and a reclaim must not erase history. This review validates that invariant against reproduced evidence.

---

## 1. Evidence ledger

All commands run in the review worktree on 2026-07-15. `dev_cargo.sh` runs `cargo test` inside the repo Nix dev shell; no network.

### 1.1 Governance and incident anchoring

| Item | Command / location | Result |
|---|---|---|
| Incident note hash reproduces | `shasum -a 256 /Users/jrudnik/notes/projects/jcode/maintenance/bug-run-plan-spawn-storm.md` | `7fdd90404a8cfb7e729686621df072ad13a48d48e0e88e5310926552d75eb992` — matches `PRESCREEN.md` R05 row and R11 ledger |
| Fixed refs resolve | `git rev-parse --verify` on the three refs | all succeed |
| `vendor/upstream` still merge-base | `git rev-parse vendor/upstream` | `631935dd1d3b...` (not current upstream; R00 obligation honored) |
| Working tree unmutated | `git status --porcelain` | empty (no source/doc/ref/stash/worktree edits) |

Incident facts (source-backed, note SHA above): 2026-07-07, jcode v0.36.0, a 6-node run_plan (4 roots, concurrency 4) completed **zero** nodes, emitted **~76 assignments in ~2 minutes**, and created **190 session files** (91 crashed/0-message SIGHUP workers in the plan dir, 93 closed/1-message in `/labs/jcode`). Manual `spawn_mode: headless` + explicit prompt worked every time. Suspected causes: (1) run_plan default spawn mode `visible`/`auto` producing SIGHUP window deaths; (2) no failure detection or backoff, so a pre-prompt death is treated as a free slot and respawned; (3) a second spawn path firing per loop.

### 1.2 Semantic fork/base/upstream comparison

| Behavior | base `631935` | upstream `802f69` | fork `7ff4fc` | Finding |
|---|---|---|---|---|
| `SwarmSpawnMode` enum + `#[default] Inline` | present | present | present | **Default is NOT the fork fix.** `git show <base>:crates/jcode-config-types/src/lib.rs` and the upstream copy both already default to `Inline` (in-process, no terminal window). The incident ran v0.36.0, older than all three refs. |
| `RunPlanChurnGuard` (3-wave abort) | absent (grep count 0) | absent (0) | present (5 markers) | **Fork-only remediation** of incident cause 2. |
| `reclaim_stale_plan_assignments` (assign-time) | absent (0) | absent (0) | present (2) | Fork-only. |
| `next_stranded_runnable_item_id` + `MAX_DEAD_ASSIGNEE_RECLAIMS` | present (6/4) | present (6/4) | present | Pre-existing at base; the *cap* is inherited, the *churn/salvage wiring around it* is fork. |
| `communicate.rs` upstream commits | — | 3 commits (`9f201ca1a` seeding collision, `5eb8e09c9` async member waits, `7cb5211fa` require labels) | — | **Upstream shipped no spawn-storm remediation.** `git diff <base> <up> -- communicate.rs \| grep -iE 'churn\|reclaim\|backoff\|spawn_mode\|stranded\|wave'` returns empty. |

Commands: `git show <ref>:<file> | grep -c <marker>`; `git log --oneline <base>..<up> -- <file>`; `git diff <base> <up> -- <file>`. Patch-ID pre-screen (R00) already proved no exact per-commit equivalence; this is symbol/semantic level as R00 requires. Conclusion: the load-bearing R05B safety machinery (churn breaker, assign-time reclaim, dead-member salvage) is **fork-only with no upstream counterpart**, so the only coherent dispositions are `retain-fork` or `delete`, and deleting the sole spawn-storm fix is self-defeating.

### 1.3 Behavior enumeration — every liveness / reclaim / assignment writer and timer

**Liveness authority (who decides a worker is dead):**
- `member_status_is_dead(status)` — `crates/jcode-app-core/src/server/swarm.rs:372` — canonical predicate `matches!(status, "failed" | "stopped" | "crashed")`.
- `assignee_is_dead` closure at `swarm.rs:781` — calls `member_status_is_dead` **plus** a salvage grace window (correct, uses the authority).
- `assignee_is_dead` closure at `comm_control.rs:732-738` — **inlines** the literal `"failed" | "stopped" | "crashed"` instead of calling `member_status_is_dead`, plus `None => true` (departed member). **Finding R05B-1 (IMPORTANT):** a second, hand-copied liveness definition. It agrees today, but invariant 5 ("one liveness authority per layer") is only textually, not structurally, satisfied. A future status added to `member_status_is_dead` would silently diverge here.
- `member_status_is_terminal` (`swarm.rs:346`) — superset (`+ closed, disconnected`) used for member GC, distinct and intentionally broader.
- `swarm_verbs.rs:55` also re-inlines the same triple — third copy, doc/verb layer.

**Reclaim / salvage writers (who releases a dead worker's assignment):**
1. `reclaim_stranded_assignment` (`jcode-plan/src/lib.rs:623`) — the primitive: clears `assigned_to`, bumps `dead_assignee_reclaims`, `plan.version += 1`, **preserves** heartbeats/checkpoints/details (only the binding is released). History-preserving by construction.
2. `next_runnable_task_id_reclaiming_stranded` (`comm_control.rs:712`) — assign-time (lazy) reclaim, gated by `next_stranded_runnable_item_id` which excludes nodes at/over `MAX_DEAD_ASSIGNEE_RECLAIMS`.
3. `reclaim_stale_plan_assignments` (`comm_control.rs:641`, called `:1651`) — pre-assignment sweep of non-terminal items with departed assignees.
4. `salvage_plan_assignments_of` / `salvage_assignments_of_dead_member` (`swarm.rs:427/479`) — eager, at the moment a member dies: requeue under the cap, or mark `failed` at the cap. Callers: `refresh_swarm_task_staleness` sweep (`swarm.rs:796`) and `remove_session_from_swarm` (`swarm.rs:1193`).
5. `refresh_swarm_task_staleness` reaper (`swarm.rs:647`, W3) — a `running_stale` item past `swarm_task_reap_after` whose assignee departed is **failed** (staleness is not a dead end).

All five route the cap through the single constant `MAX_DEAD_ASSIGNEE_RECLAIMS = 3` (`jcode-plan/src/lib.rs:584`). Two enforcement sites (`salvage_plan_assignments_of:445`, `next_stranded_runnable_item_id:614`) read it; both preserve prior progress. **No reclaim path deletes history** — verified by reading each writer.

**Timers (all env-overridable, `Duration::from_secs(configured_positive_u64(...))`):**
`swarm_task_heartbeat_interval`, `swarm_task_stale_after`, `swarm_task_sweep_interval`, `swarm_dead_pid_sweep_interval`, `swarm_terminal_member_gc_interval`, `swarm_terminal_member_retention`, `swarm_task_reap_after` (all `swarm.rs` 221-577). Dead-PID sweep is throttled by an atomic `claim_dead_pid_sweep` (once per interval).

**Dispatch / assignment writers:**
- `handle_comm_assign_next` (`comm_control.rs:1951`) — the auto-driver entry; spawns via `spawn_swarm_agent` with `spawn_mode = None` (line 2028) → inherits config default (`Inline`).
- `handle_comm_assign_task` — explicit assignment, guarded by `active_assignment_conflict` (double-assign guard).
- `auto_assign_claims` (`comm_control.rs:102`) + `AUTO_ASSIGN_CLAIM_TTL = 15s` — in-process claim so concurrent picks within the plan-write window cannot stack on one worker (observed live: 3 picks in ~100ms).
- `filter_swarm_agent_candidates` / `is_drivable_auto_worker` (`comm_control.rs:52/71`) — auto-pick only lands on `is_headless` or requester-owned `ready|completed` members; foreign humans and zombies are excluded (they can only be targeted explicitly). This is the fix for incident cause 3 (stray spawn/assign onto wrong sessions).

**Spawn-mode authority (visible/headless/inline/auto):**
- Parsed once by `parse_swarm_spawn_mode` → `SwarmSpawnMode::parse` (`client_lightweight_control.rs:31`), error names the four legal values.
- Resolved once in `spawn_swarm_agent` (`comm_session.rs:579`): `spawn_mode.unwrap_or(agents_config.swarm_spawn_mode)`. `Headless | Inline` → in-process (no window); `Visible | Auto` → `prepare_visible_spawn_session`.
- **Fail-safe fallback (`comm_session.rs:651-691`):** any visible-spawn `Err(_)` **or** a `launched == false` result falls back to `create_headless_session`. So even an explicit `Visible`/`Auto` on a display-less host degrades to a working in-process worker instead of a SIGHUP corpse. This is the structural cure for incident cause 1, independent of the default.

**Session/member-growth containment:**
- `RunPlanChurnGuard::MAX_WAVES_WITHOUT_COMPLETION = 3` (`communicate.rs:567`): after 3 consecutive assignment waves with zero completed-node progress the driver **aborts** with a diagnostic naming churned nodes and lost workers, and broadcasts an alert (`communicate.rs:1613-1626`). Empty idle waves do not reset the counter; only completion progress does (`record_wave:575-586`). This is the direct bound on the "76 assignments" storm.
- `run_plan_driver_claims` single-driver guard (`communicate.rs:790`): check-and-insert under one mutex; a second `run_plan` for the same session gets `AlreadyRunning`. Stale claims (finished/pre-reload task) are replaceable via `is_live_task`. Prevents N concurrent drivers each spawning a wave.
- `MAX_SWARM_MEMBERS = 1000` hard cap (`jcode-swarm-core/src/lib.rs:64`, enforced `comm_session.rs:1492`): refuses spawns past 1000 live members. **Caveat:** this counts *live* members; reaped dead members free slots, so the 1000 cap alone does not bound cumulative *session files*. The churn guard is the true storm bound; the member cap is a breadth ceiling.
- Member-cap recovery ladder in run_plan (`communicate.rs:1494-1529`): on cap refusal, first free finished owned workers, then fall to reuse-only (no spawn), then continue with in-flight work — never an unbounded respawn.
- Terminal-member GC + retention (`swarm.rs:336/324`, server loop `server.rs:1291`): terminal members pruned after a retention window so historical records do not grow forever.
- Session picker filters 0-message/empty and crashed sessions (`session_picker/loading.rs:916` `is_empty_session_file`, `render.rs:723` `omitted_crashed_count`) so a residual storm is not shown as live work (incident's "80 empty sessions in the picker").

### 1.4 Observable failure

- Double-assign rejection names task, current assignee, activity age, and takeover path (`active_assignment_error`, tested `assign_double.rs:43-50`).
- Churn abort returns `Err` with "possible spawn churn" + churned nodes + lost workers, and broadcasts to the swarm (`communicate.rs:618`, tested).
- Dead-member salvage notifies the coordinator/owner with a `⚠ Worker … died …` message distinguishing requeued vs cap-failed tasks (`DeadMemberSalvage::describe`, `swarm.rs:391`).
- Reap marks the item `failed` with a checkpoint summary (visible, not silent).

### 1.5 Reproduction of the spawn-storm evidence (deterministic)

The live 190-session storm is not reproducible without a display-less spawn host, but its **causal mechanism is deterministically reproduced** by the unit fixtures:

- `run_plan_churn_guard_aborts_after_three_assignment_waves_without_completion` (`communicate_tests.rs:181`): three `record_wave(assignments, completed=0, completed=0)` calls; the third returns the abort diagnostic. This is exactly "assign, worker dies, respawn, 0 completions" — the incident loop — now bounded at 3 waves instead of 76 assignments.
- `run_plan_churn_guard_trips_across_empty_idle_waves` (`:224`): slow churn (assign, idle, assign, idle, assign) still trips — closes the "reset every quiet loop" evasion.
- `run_plan_churn_guard_resets_on_completion_progress` (`:202`): genuine progress clears the counter (no false abort).
- `f1_assign_next_reclaims_task_from_departed_assignee` (`failure_scoreboard.rs:19`): a task assigned to a ghost (non-member) is reclaimed to a live worker instead of stalling — the "runnable task(s) could not be assigned" fast-fail from the incident.

I ran these; results in §1.6.

### 1.6 Narrow no-network test results (commands + outcomes)

```
bash scripts/dev_cargo.sh test -p jcode-plan --lib
  -> test result: ok. 79 passed; 0 failed  (incl. reclaim_stranded_assignment_releases_owner_and_counts_reclaims,
     stranded_runnable_item_requires_dead_assignee_and_respects_reclaim_cap)

bash scripts/dev_cargo.sh test -p jcode-app-core --lib server::comm_control::tests
  -> test result: ok. 80 passed; 0 failed  (incl. f1..f4 failure scoreboard,
     assign_task_rejects_double_assignment_of_actively_worked_task,
     assign_task_allows_taking_over_stale_assignment,
     task_control_reassign_tells_displaced_worker_to_stand_down)

bash scripts/dev_cargo.sh test -p jcode-app-core --lib server::swarm::tests
  -> test result: ok. 29 passed; 0 failed  (incl. salvage_fails_task_once_reclaim_cap_is_reached,
     salvage_requeues_dead_members_tasks_and_notifies_coordinator,
     refresh_swarm_task_staleness_reaps_orphaned_tasks_past_deadline,
     staleness_sweep_salvages_tasks_of_vanished_assignee,
     dead_pid_sweep_marks_swarm_member_crashed_without_picker)

bash scripts/dev_cargo.sh test -p jcode-app-core --lib churn
  -> test result: ok. 5 passed; 0 failed  (the three RunPlanChurnGuard tests + format/e2e)
```

`jcode-app-core --lib` compiled clean (one pre-existing dead-code warning in `control_log_sync.rs:263`, not R05B). No network used; the dev shell provides the toolchain.

---

## 2. Supported disposition

**`retain-fork`**, conditional on the fixtures/observables in §5.

Justification: the spawn-storm remediation (churn breaker, assign-time and eager dead-member reclaim under a shared cap, drivable-worker auto-pick filter, single-driver guard, headless fallback) is entirely fork-authored and has **no upstream counterpart** (§1.2). Upstream's only `communicate.rs` changes since the merge base are unrelated (collision-safe seeding, async waits, label requirement). The behavior is well-factored, history-preserving, and independently tested with red-first fixtures. Adopting upstream would *lose* the fix; deleting it re-opens the incident. `compose` is not available because there is nothing upstream to compose with. `upstream-patch` is not applicable for the same reason.

---

## 3. Pilot relevance

R05B is **not a pilot prerequisite** (RESPONSIBILITIES.md: pilot is `no, unless swarm-driven`; the bounded pilot is one no-tool agent turn). The R00/R09/R11 overlays still bind. R05B does not block the Phase 3 pilot and the pilot does not exercise worker dispatch. This ledger is Phase 2 completion work, not a pilot gate.

---

## 4. Blockers and observables (findings)

| ID | Severity | Finding | Evidence | Required observable |
|---|---|---|---|---|
| R05B-1 | IMPORTANT | Three copies of the dead-status predicate: `member_status_is_dead` (authority), inlined literal in `comm_control.rs:734`, inlined literal in `swarm_verbs.rs:55`. Invariant 5's "one liveness authority" is textual, not structural. | grep of `"failed" | "stopped" | "crashed"` across `crates/jcode-app-core/src` | A single `assignee_is_dead`/`member_status_is_dead` call at every site, with a test proving all reclaim/assign paths change together when the status set changes. |
| R05B-2 | MINOR | `MAX_SWARM_MEMBERS = 1000` bounds *live* members, not cumulative session files. Under a pathological pre-churn-abort burst (≤3 waves × concurrency) a bounded but nonzero pile of 0-message sessions can still be created; the picker hides them but they persist on disk. The incident's own F4 asked for GC of pre-prompt crashed sessions; that GC is presentation-only (`is_empty_session_file` filter), not disk reclamation. | `comm_session.rs:1492`; `session_picker/loading.rs:916`; incident note "Session GC" fix. | A fixture bounding total sessions created across a full churn-to-abort cycle, or an explicit decision that ≤(3×concurrency) residual sessions is acceptable and picker-hidden. |
| R05B-3 | MINOR | The incident's *default-mode* hypothesis is stale: default was already `Inline` at base/upstream, so on current code the storm's proximate cause (visible-window SIGHUP) is cured by both the default and the headless fallback. The remaining real risk is churn on *any* pre-prompt worker death, which the churn guard bounds. The ledger must not credit the fork for changing the default. | `git show <base>:crates/jcode-config-types/src/lib.rs` shows `#[default] Inline`. | Ledger states the true fork contribution is churn/reclaim/fallback, not the default mode. |
| R05B-4 | MINOR | `next_runnable_task_id_reclaiming_stranded` reclaims one stranded node per call; a plan with many dead-assignee nodes drains them across successive assign_next calls. Correct, but worth a note that reclaim throughput is one-per-dispatch. | `comm_control.rs:742` (single `?` on first stranded id). | None; document the drain cadence. |
| R05B-5 | IMPORTANT | The cap-fail terminal transition **overwrites** `progress.checkpoint_summary` with `"failed: assigned worker … died and the automatic reclaim cap was reached"` and sets `completed_at_unix_ms`/`stale_since_unix_ms`, discarding the prior checkpoint text. Heartbeats/`started_at`/`heartbeat_count` survive, so this is not full history loss, but the last worker-authored checkpoint is lost. Crucially, **no fixture asserts history survival on the cap-fail path** — `salvage_fails_task_once_reclaim_cap_is_reached` only asserts `status=="failed"` and `assigned_to==None` (verified: its assertions are status/assigned_to/outcome only). The "no reclaim erases history" invariant is proven only for the *requeue* path (`requeue_existing_assignment_preserves_prior_progress_history`, `reclaim_stranded_assignment_releases_owner_and_counts_reclaims`), not the *fail* path. | `swarm.rs:445-462` (checkpoint overwrite); `swarm.rs` cap-fail test asserts only status/assigned_to. | A fixture asserting that after cap-fail the node retains its pre-fail heartbeat/checkpoint provenance (append a terminal reason rather than overwrite), or an explicit decision that overwriting the checkpoint on terminal-fail is acceptable and why. |

No CRITICAL findings. No double-assignment, unbounded-reclaim, unbounded-retry, or history-erasing reclaim path was found; each is affirmatively tested.

---

## 5. Fixtures proving the invariants (what a passing R05B must show)

These already substantially exist; the seam's implementation gate should confirm all pass and add the two gaps (R05B-1, R05B-2).

1. **One liveness authority:** a test that changes the dead-status set in one place and asserts every reclaim/auto-pick path observes it (closes R05B-1). Today: `member_status_is_dead_matches_terminal_non_success_states` (`swarm.rs:3011`) tests the predicate but not the call-site unification.
2. **Bounded retries:** `stranded_runnable_item_requires_dead_assignee_and_respects_reclaim_cap` + `salvage_fails_task_once_reclaim_cap_is_reached` (cap = 3, then `failed`). PASS.
3. **No double assignment:** `assign_task_rejects_double_assignment_of_actively_worked_task` (rejects, keeps original assignee) + `active_assignment_conflict_detects_only_assigned_and_fresh_items` (pure guard) + `released_claim_makes_member_pickable_again` (claim TTL). PASS.
4. **No unbounded session/log growth:** `run_plan_churn_guard_aborts_after_three_assignment_waves_without_completion` + `_trips_across_empty_idle_waves` (storm bounded at 3 waves) + single-driver claim tests. PASS. Gap: total-sessions-created bound (R05B-2).
5. **Failure visibility:** churn abort diagnostic (broadcast), `salvage_requeues_dead_members_tasks_and_notifies_coordinator`, double-assign error text. PASS.
6. **History preserved on reclaim:** `reclaim_stranded_assignment_releases_owner_and_counts_reclaims` + `requeue_existing_assignment_preserves_prior_progress_history` cover the **requeue** path. PASS for requeue. **Gap (R05B-5):** the **cap-fail** terminal path overwrites `checkpoint_summary` and has no history-survival assertion — add one or record the accepted overwrite.
7. **R05A/R04 separation:** the W1 dual-write assertion `assert_control_log_matches_maps` in F1/F3/F4 proves the control-log fold (R05A truth) stays consistent with the in-memory maps after R05B mutations, without R05B owning the fold. PASS.

---

## 6. R05A DAG/control-log truth vs R04 process lifecycle (separation)

- **R05A** owns `jcode-swarm-core/src/control_log.rs` `fold`/`replay`/`ControlLogWriter` (dependency readiness, node transitions, event fold). R05B *consumes* fold-consistency (dual-write assertions) but does not define it.
- **R04** owns session process status: `member_status_is_dead` reads a member `status` string that R04's process/session lifecycle (crash → `Crashed`, `sweep_dead_pid_swarm_members` mirroring `reconcile_active_sessions`) populates. R05B *reads* that truth to decide reclaim; it does not own PID liveness or terminal state.
- **R05B** is the policy layer between them: given R04's "this member is dead" and R05A's "this node is ready/assigned", it decides assignment, reclaim-under-cap, retry-limit, and churn-abort. Invariant 5 is honored in behavior; R05B-1 is the one structural crack (a copied predicate) that should be closed so the R04→R05B liveness contract has a single definition.

---

## 7. R09 debt attribution for R05B

Per R09 overlay obligation 3 (debt follows behavioral ownership). R05B-owned production files:
- `crates/jcode-app-core/src/tool/communicate.rs`: 3 panic-prone markers, all `unreachable!`/`expect` inside the seed-collision retry loop (`:2695,:2704,:2718`), provably exhausted, not in dispatch/reclaim. Base had 0 (file evolved on both sides). These belong to the seeding path (R05A-adjacent), not dispatch.
- `crates/jcode-app-core/src/server/comm_control.rs`, `comm_session.rs`: **0** production panic markers.
- `crates/jcode-plan/src/lib.rs`: 3 markers, all inside `#[cfg(test)]` (mod at `:791`); **0** production.
- `python3 scripts/check_panic_budget.py` red items are in `jcode-telemetry-core`, `jcode-tui`, `memory_recall_bench` — **not R05B files**.
- Test-size: `communicate.rs` (3422 lines), `comm_control.rs` (2727), `swarm.rs` (3432) are large but the bulk is `#[cfg(test)]` modules (comm_control_tests, communicate_tests, swarm tests). R05B owes R09 a per-file enumeration of any of the 60 production-size / 31 test-size violations that land in these files before its implementation gate; I did not enumerate the ratchet's per-file list here (that is the seam's implementation-gate task).
- **No `--update`** was or should be used. Gate baselines unchanged; working tree clean.

Net: R05B introduces **no new production panic/swallowed-error debt in its dispatch/reclaim/liveness paths**. The only production `unreachable!`s are in the (R05A-owned) seed-collision loop.

---

## 8. Negative findings (what I looked for and did NOT find)

- **No double-assignment path:** every explicit assign routes through `active_assignment_conflict`; every auto-pick routes through `auto_assign_claims` + drivable filter. No unguarded `assigned_to =` write to an already-active node was found.
- **No unbounded reclaim:** all five reclaim writers share `MAX_DEAD_ASSIGNEE_RECLAIMS`; past the cap the node is `failed`, not re-dispatched.
- **No unbounded retry/respawn:** churn guard aborts at 3 waves; member-cap ladder never loops into unbounded spawn; single-driver guard prevents N drivers.
- **No history-erasing reclaim:** every reclaim preserves `task_progress` heartbeats/`started_at`/`heartbeat_count`; the requeue path preserves the checkpoint too. **Caveat (R05B-5):** the cap-fail terminal path overwrites `checkpoint_summary` with the failure reason (last worker checkpoint lost, heartbeats survive) and is untested for history survival.
- **No competing upstream fix silently ignored:** `git diff <base> <up> -- communicate.rs` shows upstream did not touch churn/spawn/reclaim; nothing was overlooked.
- **No liveness authority owned by R05A/R04 that R05B re-derives incorrectly** other than the copied literal predicate (R05B-1).
- **No stash/worktree/ref mutation, no source or doc edit** (git status clean).
- **The default-spawn-mode incident hypothesis did not reproduce as a fork fix** (R05B-3): it was already `Inline` upstream; the fork's contribution is churn/reclaim/fallback.

---

## 9. Confidence and gaps

- **High confidence:** dispatch, reclaim-under-cap, churn abort, double-assign guard, single-driver guard, headless fallback, and history preservation — all read at source level and confirmed by 193 passing tests across four crates/modules I ran.
- **Medium confidence:** (a) the exact residual-session count under a real display-less burst (R05B-2 is a reasoned bound, not measured live — the live 190-session repro needs a headed-spawn host I must not use); (b) the full R09 per-file ratchet attribution for the three large files (deferred to the seam's implementation gate, as R09's own ledger permits).
- **Explicit gap (R00-required):** I did not attempt to prove semantic equivalence of anything absorbed into curated sync `b3ed82a6b`; the churn/reclaim code is demonstrably fork-only so no equivalence claim is needed.
- **Did not read** any future Grok R05B artifact (none consulted; only R00/R04-scope-via-RESPONSIBILITIES/R05A-scope-via-RESPONSIBILITIES/R09/R11 ledgers, PRESCREEN, PROGRESS, the incident note, and source/tests).

---

## 10. Bounded implementation slices (each with rollback/stop per R00)

1. **Unify liveness (R05B-1).** Replace the inlined literals at `comm_control.rs:734` and `swarm_verbs.rs:55` with `member_status_is_dead`, add a call-site-unification test. Rollback: revert the two-line change; behavior is identical today so the slice is safe. Stop if it forces a visibility/module change larger than re-exporting the predicate.
2. **Bound cumulative sessions (R05B-2).** Add a fixture asserting total sessions created across a churn-to-abort cycle ≤ (MAX_WAVES × concurrency), or record an explicit accepted-residual decision. Rollback: drop the test. Stop rather than add disk-GC of crashed sessions (that is R04/R06A territory, not R05B).
3. **Ledger correction (R05B-3).** State the true fork contribution (churn/reclaim/fallback, not default mode). Docs-only, append-only per R11. Rollback: none needed.
4. **Cap-fail history (R05B-5).** Change `salvage_plan_assignments_of` to *append* a terminal reason rather than overwrite `checkpoint_summary`, and add a fixture asserting the pre-fail checkpoint/heartbeat provenance survives the cap-fail transition — or record an explicit accepted-overwrite decision. Rollback: revert the append and drop the test. Stop if preserving the checkpoint requires a schema change to `SwarmTaskProgress` beyond one field append (that touches R06A evidence schema).

Each slice is test-first, no baseline `--update`, no ref/stash mutation, and stops rather than broadening into R04 process GC or R05A fold semantics.
