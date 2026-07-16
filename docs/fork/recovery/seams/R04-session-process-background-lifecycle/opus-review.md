# R04 Session, child-process, and background-task lifecycle: independent Opus full-seam review

| Field | Value |
|---|---|
| Reviewer | Opus (independent), `verify` posture |
| Seam | R04 (session/child-process/background-task lifecycle), `full` review, pilot `conditional` |
| Review head | `5baf343ba6da564afc3f6c58c5edca7a64d6e67f` (branch `recovery/seam-r04-20260715`) |
| Fixed refs | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Recommended disposition | **`retain-fork`** for the fork-owned incident defenses; **shared base code** for detached-task/reload-recovery mechanics (no adopt/compose available) |
| Pilot entry verdict | **not a pilot prerequisite** at the approved bounded pilot; **blocked** for any pilot variant that adds reload, resume, cancellation, or a detached/background task |
| Confidence | high for topology, fork/base/upstream provenance, terminal-writer census, and marker-race guard; medium-high for reload-handoff ordering; medium for untested exceptional multi-daemon interleavings |
| Constraints honored | read-only source; no source/docs/ref/stash/worktree edits; no live user daemon; no network/credentials; no destructive action; no future Grok R04 artifact read; only `/tmp/jcode-r04-opus-review.md` written |

This review adjudicates behavioral ownership, not files. It was performed independently of any other R04 artifact. Upstream provenance is comparison evidence only and is never authority by itself (R00 overlay).

---

## 1. Scope, exclusions, and authority separation

Per `RESPONSIBILITIES.md:26`, R04 **owns**: create, attach, resume, cancel, shutdown, reload handoff, interruption, detached task adoption, orphan reconciliation, process markers, backoff, liveness, and terminal state. It **excludes**: binary selection (R01), DAG truth (R05A), and worker assignment/reclaim policy (R05B).

Authority boundaries confirmed against the approved ledgers, kept explicitly separate below:

- **R01 target authority** (`seams/R01.../ledger.md:26-31,87`): R01 defines the canonical build/executable/channel identity tuple and reload-target selection. R04 consumes that identity in restart/session snapshots and must **not** re-derive competing truth. R04's restart fields (`jcode_version`, optional `jcode_git_hash`, optional dirty) are an incomplete projection of the R01 tuple (R01 ledger:41,62). This is an R01-owned blocker, not an R04 authority claim.
- **R03A reconnect verdict** (`seams/R03A.../ledger.md:82-83`): R03A owns the compatibility verdict and the terminal incompatible action after reconnect. R04 owns the interrupt-and-handoff step that precedes reconnect (cross-seam invariant 6). R04 must not certify compatibility; it stages recovery intents and lets R03A evaluate on reconnect.
- **R05B assignment/reclaim policy** (`RESPONSIBILITIES.md:28,94`): R05B owns dead-worker assignment reclaim, retry caps, and spawn-mode authority. R04 owns dead-**member detection**, the transition to a terminal member/session state, and triggering salvage. The reclaim counter and cap (`MAX_DEAD_ASSIGNEE_RECLAIMS`, `reclaim_stranded_assignment`) are R05B policy invoked from R04's detection site (evidence 8).
- **R13 invalidation** (`seams/R13.../ledger.md:24-48`): R13 owns post-compaction `provider_session_id` invalidation. R04 owns the **new/child/recovered-session** reset sites that must not inherit a stale provider session (R13 census classifies these to R04). R04 does not own compaction policy.

Confirmed authority-boundary cross-check: R13's writer/reset census (`seams/R13.../ledger.md:46`) and R12's census (`seams/R12.../ledger.md:144-159`) both assign the crash/child/restore reset sites to R04; those are the same sites I census below (§5).

---

## 2. Fixed-ref and provenance verification (checkpoint 1)

```
git rev-parse --verify <fork>^{commit} <upstream>^{commit} <base>^{commit}  # all resolve
git merge-base 7ff4fc6be... 802f69098...   -> 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d   (== recorded base)
git merge-base --is-ancestor 631935dd1... HEAD   -> base is ancestor of review head
git merge-base --is-ancestor 7ff4fc6be... HEAD   -> fork is ancestor of review head
git diff --stat 7ff4fc6be... HEAD -- crates src  -> EMPTY
```

**Decisive result:** the review head has **zero source diff** from the fork baseline; the head is docs-only (`5baf343ba docs: Record R03A and R12 integration`). Therefore R04 runtime behavior at the review head is byte-identical to the fork baseline, and every fork-vs-base and fork-vs-upstream comparison below is valid at head. `vendor/upstream` is pinned to `631935dd1...` (the base, not current upstream), consistent with R00's provenance warning; I cite `802f69098...` as upstream truth.

---

## 3. Semantic divergence at a glance (checkpoints 2-3)

Numstat deltas from base, and fork-vs-upstream identity, over R04 behavioral surfaces:

| Surface | base→fork | base→upstream | fork vs upstream | Ownership reading |
|---|---:|---:|---|---|
| `server/lifecycle.rs` (idle monitor / safety exit) | 61/1 | 0/0 | 1/61 | **Fork-only** incident defense. Retain-fork. |
| `session/crash.rs` (crash marker consume) | 30/12 | 0/0 | 12/30 | **Fork-only** stale-marker safety. Retain-fork. |
| `jcode-storage/active_pids.rs` (PID markers) | 410/31 | 15/2 | 29/395 | **Fork-dominant** marker lifecycle rework. Retain-fork. |
| `server/headless.rs` (session cap) | 86/17 | 53/17 | 14/47 | Fork adds `MAX_TOTAL_SESSIONS` backstop over a partly-shared base. |
| `server/client_lifecycle.rs` | 234/198 | 80/9 | 189/154 | Fork-dominant; shared with R03B transport, R04 interrupt/handoff slice. |
| `server/client_session.rs` | 139/93 | 101/49 | 44/38 | Two-sided; reload-gating rules fork-owned (see R01 ledger:63). |
| `server/reload.rs` (handoff) | 0/0 | 0/0 | IDENTICAL | **Shared base code** across all three refs. No adopt target. |
| `server/reload_recovery.rs` (recovery intents) | 232/4 | 232/4 | IDENTICAL | **Shared**: fork and upstream carry the same post-base addition. |
| `jcode-base/background.rs` (orphan/detached) | 2/2 | 2/2 | IDENTICAL (cosmetic `_graceful_timeout` rename only) | **Shared base code.** No adopt/compose divergence. |
| `jcode-base/background/model.rs` | 0/0 | 0/0 | IDENTICAL | **Shared base code.** |
| `overnight.rs` (coordinator child) | 1/0 | 1/0 | IDENTICAL | **Shared**; only the provider-session reset line is R04-relevant. |

**Two-population finding.** R04's behavioral surface splits cleanly:

1. **Fork-owned incident defenses** (lifecycle idle monitor, crash-marker consume-order, PID-marker compare-and-delete lifecycle, session/tester caps). These are the material forkbomb-incident repairs and have **no upstream counterpart to adopt or compose**. Disposition: `retain-fork`.
2. **Shared base mechanics** (reload.rs handoff, reload_recovery.rs intents, background.rs/model.rs detached-task adoption and orphan reconciliation). These are byte-identical fork/upstream, so there is **no divergence to decide**; retention is trivial and no upstream patch is available.

This is important: R04's *detached-task adoption* and *reload-recovery intent* logic is **not** a fork-vs-upstream contest. Only the *marker/idle/cap incident hardening* is fork-authored. Any future review that claims "R04 is fork-dominant" without this split would overstate the contested surface.

---

## 4. Enumerated terminal-state writers and reload/interruption paths (checkpoints 4-6)

R04's core invariant (cross-seam invariant 5, `RESPONSIBILITIES.md:67`): "R04 owns process/task life and terminal state. A dead process cannot cause unbounded assignment or session-file growth, and a reclaim cannot erase history." I enumerate every terminal writer and reconciliation path.

### 4a. Session terminal state

| Writer | Site | Terminal transition | Evidence |
|---|---|---|---|
| `Session::detect_crash` | `jcode-base/src/session.rs:1070-1095` | `Active -> Crashed` when `last_pid` not running, or stale-age fallback (>120s) when no PID | read |
| `Session::reconcile_dead_owner` | `session.rs:1098-1104` | Persists the crash transition (`save()` after `detect_crash`) | read |
| `reconcile_active_sessions` | `session.rs:39-54` | Iterates active markers, reconciles dead owners, **then** sweeps stale markers (ordering is load-bearing) | read |
| `find_crashed_via_pid_files` | `session/crash.rs:331-390` | `set_status(Crashed)`, **saves before consuming the marker**; on save failure leaves the marker for a later pass | diff read |
| `recover_loaded_crashed_sessions` | `session/crash.rs:120-142` | Creates `session_recovery_*`, sets `provider_session_id = None` (R13-classified R04 reset) | read |

### 4b. Swarm member terminal state

| Writer | Site | Transition | Evidence |
|---|---|---|---|
| `sweep_dead_pid_swarm_members` | `server/swarm.rs:269-323` | Member `-> "crashed"` for sessions whose persisted status is `Crashed`; **filters dead members BEFORE `Session::load`** so per-sweep disk I/O is O(live), not O(all) | read |
| terminal-member GC | `swarm.rs:324-372` (`swarm_terminal_member_retention`, `expired_terminal_member_ids`, `member_status_is_terminal`) | Bounded retention then removal of terminal members; keeps completion reports without unbounded growth | read |

### 4c. Background-task terminal state (detached-task adoption / orphan reconciliation)

| Writer | Site | Transition | Evidence |
|---|---|---|---|
| `finalize_detached_status_if_needed` | `jcode-base/background.rs:139-210` | `Running(detached) -> Completed/Failed` by reaping/`is_process_running(pid)`; publishes `BackgroundTaskCompleted` | read |
| `status_is_reconcilable_orphan` + `finalize_orphaned_status_if_needed` | `background.rs:231-265` | `Running(non-detached) -> Failed` when the owning process image is gone; distinguishes **exec-reload (same PID, different `owner_instance` token) from live bootstrapping** | read |
| terminal-event record | `background/model.rs:130-136,249-265` | `Completed/Failed/Superseded/Cancelled` event kinds | read |
| `cancel_with_grace` | `background.rs:1119-1190` | SIGTERM, bounded grace, then SIGKILL to the detached process **group** | read |

### 4d. Provider-session reset writers R04 owns (feeds R13 invariant 3)

`overnight.rs:188` (coordinator child), `server/client_actions.rs:698` (transfer child), `session/crash.rs:139` (crash recovery), `tui_lifecycle_runtime.rs:324,331` (restore declines stale id), `conversation_state.rs:835` (`recover_session_without_tools`). R13 ledger:46-48 confirms every R04 reset either clears both `provider_session_id` copies or immediately replaces the whole session object; the one single-copy R04 site (`conversation_state.rs:835`) is benign because line 836 rebuilds `self.session`.

### 4e. Reload handoff / interruption path (cross-seam invariant 6 ordering)

`server/reload.rs`: on a reload signal the daemon snapshots running members (`reload.rs:217-234`, `candidate_snapshot` trace), `persist_reload_recovery_intents` writes a per-session directive keyed by role (`Initiator` / `InterruptedPeer` / `Headless`, `reload.rs:280-300`), then execs into the R01-selected binary and writes `ReloadPhase` state; on failure it records `ReloadPhase::Failed` and `exit(42)` (`reload.rs:180-206`). On restart, `reload_recovery.rs` peeks/consumes the directive idempotently (`peek_for_session:217`, `mark_delivered_if_matching_continuation:286`) with path-traversal-safe session IDs (`sanitize_session_id:54`) and TTL GC (`collect_garbage_at:121`). **R04 stages the handoff; R03A evaluates compatibility on reconnect; R01 authorizes the target.** No layer claims success before the next layer's observable, consistent with invariant 6.

### 4f. Daemon terminal exit / bounded shutdown (the incident's "why it never stopped")

| Guard | Site | Property |
|---|---|---|
| Always-on idle monitor | `server/lifecycle.rs:156-185` (`spawn_persistent_lifecycle_monitor`) | Persistent server exits after `idle_timeout_secs` with 0 clients; comment and code assert debug-control **must never** disable it (`persistent_should_exit:92-95`) |
| SIGTERM watchdog | `server.rs:1193-1211` | OS-thread `sleep(3s)` scheduled **outside Tokio**, then `exit(0)`; `unregister_server_bounded` for registry cleanup |
| Bounded registry cleanup | `SERVER_LIFECYCLE_INVARIANTS.md:27-42` + `unregister_server_bounded` | 2s timeout on registry I/O; no unbounded await between signal and exit |
| Dead-PID sweep O(live) | `swarm.rs:274-282` | Skips terminal members before disk load (the incident's O(N) `Session::load` amplifier) |
| Session cap | `headless.rs:19-26` (`MAX_TOTAL_SESSIONS = 1500`) | Backstops runaway headless/`mcp-serve` session creation |
| Tester caps | `debug_testers.rs:5-8,67-84` (`MAX_TESTERS=8`, `MAX_TESTER_DEPTH=1`) | Bounds tester fan-out and spawn depth |
| Owned-MCP child cap | `SERVER_LIFECYCLE_INVARIANTS.md:61` (`MAX_OWNED_MCP_CHILDREN=64`) | Bounds per-session owned MCP children |

These are exactly the guardrails the forkbomb incident (`MCP_SERVE_FORKBOMB_INCIDENT.md:92-101`) says were shipped, and they are the material R04 authority for the incident class.

---

## 5. Orphan / double-owner / stale-marker risk analysis (checkpoint 7)

The highest-risk R04 concern is a **stale PID marker deleting a live owner's marker**, or a dead owner's marker living forever. I traced the compare-and-delete design in `jcode-storage/active_pids.rs`:

- `remove_active_pid_marker_if_stale_and_matches` (`active_pids.rs:167-178`) re-acquires the shared `PidMarkerLock` and only unlinks when the on-disk bytes still **exactly match** the caller's earlier observation (`remove_marker_if_stale_and_matches:251-262`) **and** the marker is not live (`marker_contents_are_live:279-284` via `process::is_running`). This closes the read-to-delete race: a live owner that rewrote the marker between the caller's read and the delete is preserved.
- `sweep_stale_pid_markers` (`active_pids.rs:190-209`) holds the lock through final unlink and no-ops entirely if the lock cannot be acquired (`:191-193`), so an uncoordinated writer cannot be clobbered.
- Crash consume-order is safe: `crash.rs:355-388` saves the `Crashed` transition **before** consuming the marker, and on save failure leaves the marker for a later pass. `reconcile_active_sessions:49-52` documents and enforces the ordering (markers become `Crashed` before storage sweeps them), so a session marker is never orphaned before its terminal transition is durable.
- Exec-reload double-owner: `status_is_reconcilable_orphan` (`background.rs:231-246`) uses `owner_instance` (`process_instance_token`, `model.rs:89-93`) to distinguish a same-PID exec-reload (owning future is gone) from a still-bootstrapping task written by this exact image. This is the correct signal precisely because `getppid()==1` is unreliable after normal quit (`SERVER_LIFECYCLE_INVARIANTS.md:46-56`).

**Residual risks (bounded, not blockers for the approved pilot):**

1. **mtime is ordering, not cryptographic identity** (inherited from R01 ledger:114). PID reuse across a very fast crash+respawn with an identical marker byte string is theoretically possible; `marker_contents_are_live` mitigates by liveness check, but a reused PID that is coincidentally alive would read as live and be preserved (fail-safe direction: it does not delete a live-looking marker).
2. **No spawner heartbeat.** Faster-than-300s abandoned-daemon cleanup is explicitly deferred (`SERVER_LIFECYCLE_INVARIANTS.md:53-56`). An orphaned daemon with no clients still burns up to 300s before the idle exit. Acceptable and documented; not an R04 correctness defect.
3. **Reload-recovery under partial write.** `reload_recovery.rs` uses TTL GC for malformed/orphaned records (`collect_garbage_at:121`, tested at `:609`), but a directive persisted then never consumed relies on GC; no evidence of a leak, but no multi-daemon interleaving test exists.
4. **Age-heuristic crash fallback** (`session.rs:1085-1092`, 120s) can mark a legitimately slow no-PID legacy session as crashed. Bounded to old sessions without `last_pid`; low risk.

No double-owner *deletion* path was found. No unbounded-growth path survives the caps + O(live) sweep. No reclaim-erases-history path: salvage requeues or fails-with-checkpoint but preserves `task_progress` history (`swarm.rs:445-470`).

---

## 6. R01 / R03A / R05B / R13 separation, restated with evidence

- **R01 (target authority):** R04 restart/session snapshot fields are an incomplete projection of the R01 identity tuple (R01 ledger:41,62). The gap is **R01-owned** (its blocked pilot slice 2). R04's only obligation is to record the corresponding `version_label`/fingerprint/channel/executable when R01 supplies the projection contract; R04 must not invent identity. **No R04 change is authorized here** beyond consuming an R01-defined field.
- **R03A (reconnect verdict):** R04 owns interrupt-and-handoff (`reload.rs` intents); R03A owns the terminal incompatible action on reconnect (R03A ledger:82-83, currently a blocker on the server-continuation defect). The two compose via invariant 6; R04 must not short-circuit R03A's verdict. R04 review confirms no R04 site emits or consumes a `HandshakeVerdict` (that is R03A/R03B).
- **R05B (assignment/reclaim):** R04's `salvage_plan_assignments_of` (`swarm.rs:427-472`) detects the dead member and **calls** R05B's reclaim primitives (`reclaim_stranded_assignment`, `MAX_DEAD_ASSIGNEE_RECLAIMS`). The cap/counter semantics are R05B's; R04 owns only the death-detection trigger and the terminal member transition. The incident-path validation must be done **jointly** with R05B (`RESPONSIBILITIES.md:94`).
- **R13 (invalidation):** R04's new/child/recovered-session resets are already enumerated and classified in the R13 census (R13 ledger:46) and re-confirmed in §4d. R04 owns these reset sites; R13 owns compaction-completion resets. No R04 reset clears only the persisted copy while leaving a stale agent copy.

---

## 7. Deterministic, no-live-daemon fixtures and pilot relevance

All fixtures use `TempDir`, isolated `JCODE_HOME`/`JCODE_RUNTIME_DIR`/`JCODE_SOCKET`, no network, no credentials, no live user daemon. Existing regression floor confirmed by execution (§9).

| Fixture | Required observable | Current status |
|---|---|---|
| Marker compare-and-delete race | A live owner that rewrote its marker between a caller's read and the sweep is **preserved**; a dead marker is removed | **Exists, passes:** `conditional_cleanup_preserves_a_replaced_live_marker`, `stale_marker_sweep_removes_dead_and_invalid_but_preserves_live`, `lock_failure_leaves_marker_state_untouched` |
| Crash consume-order | `Crashed` is persisted before the marker is consumed; save failure leaves the marker | **Partial:** covered indirectly by `explicit_sweep_removes_dead_marker_without_session_data`; a dedicated save-failure fixture is **missing** |
| Idle safety exit under debug-control | `persistent_should_exit(0, elapsed>=timeout, timeout)` true even with debug control; never disabled | **Exists:** `server::lifecycle` unit tests (`lifecycle.rs:329-390`), app-core filtered run passed exit 0 |
| Session cap backstop | Creation refused at `MAX_TOTAL_SESSIONS` | **Exists:** `session_backstop_rejects_at_cap`, `session_backstop_allows_one_below_cap` (`headless.rs:296-303`) |
| Tester spawn/depth cap | Refuse at depth>=1 or live>=8 | **Exists:** `check_tester_spawn_allowed` unit tests (`debug_testers.rs`) |
| Dead-PID sweep O(live) | Terminal members not re-loaded from disk; crashed sessions mirrored to member state | **Partial:** logic present and commented; a load-count assertion fixture is **missing** |
| Detached-task adoption | `Running(detached)` with dead PID finalizes to `Completed/Failed` and publishes completion | **Exists** in `jcode-base` background tests (shared code) |
| Exec-reload orphan | `Running(non-detached)` with same PID but different `owner_instance` finalizes to `Failed`; same instance token is left alone | **Exists** via `status_is_reconcilable_orphan` unit coverage (shared code) |
| Reload-recovery directive round trip | Persist → peek (non-consuming) → idempotent mark-delivered; path-traversal rejected; TTL GC | **Exists:** `reload_recovery.rs` unit tests `:405-653` (persist/peek/idempotent/GC/traversal) |
| Reload handoff → reconnect verdict (joint R01/R03A/R04) | Two dirty same-commit builds carry distinct R01 identity through restart snapshot and R03A subscribe | **Missing, cross-seam blocker** (shared with R01 slice 2 / R03A slice 2) |

**Pilot relevance.** Per `RESPONSIBILITIES.md:86`, R04 is **not** a prerequisite for the approved bounded pilot (one no-tool turn, no reload/resume/cancel/detached task). I confirm the approved pilot exercises none of R04's terminal writers: no reload signal, no crash, no background/detached task, no swarm member. Therefore R04 imposes **no new pilot blocker** at the approved scope. **If** the pilot is widened to add reload, resume, cancellation, or a detached/background task, R04 becomes a full prerequisite and the joint reload-handoff fixture above is a hard blocker.

---

## 8. Disposition, implementation slices, and R09 debt

### Disposition

- **`retain-fork`** for the fork-owned incident defenses (lifecycle idle monitor, crash consume-order, PID-marker lifecycle, session/tester caps). There is no upstream counterpart to adopt or compose; deleting them re-opens the forkbomb incident class.
- **Retain shared base code** (reload.rs, reload_recovery.rs, background.rs/model.rs, overnight.rs reset) unchanged: fork and upstream are byte-identical, so no adopt/compose decision exists.
- **No upstream opportunity** in R04 at the fixed refs: every fork-owned surface has no upstream analog, and every shared surface is already identical.

### Bounded implementation slices (each with rollback/stop)

| Slice | Class | Change | Acceptance | Rollback / stop |
|---|---|---|---|---|
| 1 | `sync` | None. Preserve fork incident defenses; recheck fixed refs before any comparison. | `base..upstream` remains 0/0 on lifecycle.rs/crash.rs; storage markers still fork-dominant. | Stop if an upstream marker/idle counterpart appears (would require a compose review). |
| 2 | `fix` | Add the missing crash **save-failure** fixture: on `save()` error the marker is retained and reconsumed on a later pass. | Deterministic temp-`JCODE_HOME` test asserts marker survives a forced save failure and is consumed only after a successful save. | Stop if it needs a live daemon or real signal delivery. |
| 3 | `fix` | Add the dead-PID sweep **load-count** fixture: assert `Session::load` count is O(live), not O(all), across a member set with many terminal members. | Load counter proportional to live members only. | Stop if it requires real process spawning beyond fixtures. |
| 4 | `fix` (joint R01/R03A/R04) | Carry the R01 projection through the R04 restart snapshot and assert R03A reconnect sees it; two dirty same-commit builds distinguished. | Joint fixture passes with distinct `version_label`/fingerprint/channel; R03A labels build_hash compatibility-only. | Stop if it needs an unowned R01 identity writer, live builds, or an R03A protocol semantic change without governance. This is **blocked** on R01 slice 2 and R03A slice 2. |
| 5 | `refactor` | None authorized. `swarm.rs` (3432 LOC) and `client_session.rs` (1502 LOC) are oversized but must not be split until slices 2-4 pin behavior. | A future split lists every terminal writer and preserves all fixtures. | Stop if refactor crosses into R05B assignment or R03B transport. |
| 6 | `docs` | Record disposition, terminal-writer census, marker-race guard, and joint reload blocker. | Ledger matches passing fixtures. | Stop on any overclaim of identity or test result. |

### R09 debt attribution (owned by R04)

R09 requires each behavior seam to enumerate its owned red debt before implementation (`R09 ledger:47-50`, gaps). Static findings on R04-owned files at head:

- **Panic/unwrap/expect in production R04 code: 0.** Every `unwrap()/expect()/panic!` in `active_pids.rs` (31), `swarm.rs` (26), `overnight.rs` (37), `crash.rs` (4), `reload_recovery.rs` (4) sits **inside `#[cfg(test)]`** (verified: first non-test occurrence is at/after the `mod tests` boundary in each). R04 introduces **no new production panic debt**.
- **Production-size (oversized) files R04 owns:** `swarm.rs` 3138→3432 LOC, `client_session.rs` 1456→1502, `overnight.rs` 1274→1275 (`check_code_size_budget.py`, exit 1). These are attributed to R04 (and, for `client_session.rs`, shared with R03B). R04 owns paying these down but only after behavior is pinned (slice 5). No `--update` may be used.
- **Test-size / swallowed-error:** not enumerated per-file here; R04's `sync`-only slice 1 changes no Rust source and therefore introduces no new swallowed-error debt. Any future R04 code slice must rerun the R09 gate matrix without `--update` and keep inherited red visible.

---

## 9. Commands run and exact results (evidence, checkpoint 8)

Read-only; no live daemon, network, credentials, or edits outside `/tmp/jcode-r04-opus-review.md`.

```
# Fixed refs / ancestry
git rev-parse --verify 7ff4fc6be...^{commit} 802f69098...^{commit} 631935dd1...^{commit}   -> all resolve
git merge-base 7ff4fc6be... 802f69098...            -> 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d
git merge-base --is-ancestor 631935dd1... HEAD       -> base-ancestor-OK
git merge-base --is-ancestor 7ff4fc6be... HEAD       -> fork-ancestor-OK
git diff --stat 7ff4fc6be... HEAD -- crates src      -> EMPTY (head is docs-only vs fork)
git rev-parse vendor/upstream                        -> 631935dd1... (pinned to base, per R00)

# Provenance (numstat base->fork, base->up; fork-vs-up identity) — see §3 table
git diff --numstat <base> <fork|up> -- <file>        -> values in §3
git cat-file -e <base>:crates/jcode-app-core/src/server/reload.rs   -> exists (shared)

# Narrow, no-network tests
bash scripts/dev_cargo.sh test -p jcode-storage --lib
  -> running 7 tests; test result: ok. 7 passed; 0 failed
     incl. conditional_cleanup_preserves_a_replaced_live_marker,
           stale_marker_sweep_removes_dead_and_invalid_but_preserves_live,
           lock_failure_leaves_marker_state_untouched,
           explicit_sweep_removes_dead_marker_without_session_data,
           sweep_reclaims_atomic_write_temp_residue_idempotently
bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- server::lifecycle server::debug_testers server::headless
  -> completed exit_code 0 (filtered run, task 300463nwki, 332s incl. full compile)

# R09 debt attribution
python3 scripts/check_code_size_budget.py            -> exit 1; R04-owned: swarm.rs 3138->3432,
                                                        client_session.rs 1456->1502, overnight.rs 1274->1275
grep -nE '\.unwrap\(\)|\.expect\(|panic!|unreachable!' <R04 files>, minus mod-tests boundary
                                                     -> 0 production occurrences in R04-owned files
```

Attempted but not decisive within budget: a full `jcode-app-core --lib` compile for a fresh filtered run timed out on a clean rebuild (~5-6 min compile); the earlier filtered lifecycle/tester/headless run had already completed exit 0, so I did not re-expand. No timeout is treated as a pass; the storage suite and the completed app-core filtered run are the executed evidence.

---

## 10. Negative findings

- **No double-owner marker-deletion path.** The compare-and-delete guard under a shared lock preserves any live/replaced marker (§5).
- **No unbounded session/process growth path survives** the caps (`MAX_TOTAL_SESSIONS`, `MAX_TESTERS`, `MAX_TESTER_DEPTH`, `MAX_OWNED_MCP_CHILDREN`) plus the O(live) dead-PID sweep.
- **No reclaim-erases-history path.** Salvage requeues or fails-with-checkpoint while preserving `task_progress` (`swarm.rs:445-470`), satisfying invariant 5's "reclaim cannot erase history."
- **No fork-vs-upstream divergence in detached-task adoption or reload-recovery intents.** Those surfaces are byte-identical across all three refs; there is nothing to adopt or compose.
- **No R04 site derives R01 identity, emits an R03A verdict, sets an R05B reclaim cap, or performs an R13 compaction reset.** Authority separation holds.
- **No production panic debt introduced by R04.** All unwraps/expects are in test modules.
- **No upstream R04 patch opportunity** at the fixed refs.

## 11. Confidence and remaining gaps

- **High:** provenance and two-population split; terminal-writer census; marker-race guard correctness; caps/idle-exit incident defenses; authority separation from R01/R03A/R05B/R13; storage suite pass.
- **Medium-high:** reload-handoff ordering (invariant 6) — logic and traces are present and unit-tested for the recovery-intent round trip, but no multi-daemon interleaving test exists.
- **Medium:** exceptional multi-daemon interleavings (concurrent sweep + reload + crash), PID-reuse edge under `marker_contents_are_live`, and the joint R01/R03A/R04 dirty-build restart projection (blocked on R01/R03A slices). The crash save-failure fixture and dead-PID load-count fixture are missing and are the two cheapest R04-only gaps to close (slices 2-3).

## 12. Supported disposition (single)

**`retain-fork`** for R04's fork-owned incident defenses; retain shared base mechanics unchanged. R04 is **not a blocker for the approved bounded pilot** and imposes no new pilot prerequisite at that scope. R04 becomes a **full prerequisite and is blocked** for any pilot variant adding reload, resume, cancellation, or a detached/background task, with the joint R01/R03A/R04 reload-handoff fixture as the hard gate. No source, docs, ref, stash, or worktree was edited; only `/tmp/jcode-r04-opus-review.md` was written.

Review-head SHA: `5baf343ba6da564afc3f6c58c5edca7a64d6e67f`.
