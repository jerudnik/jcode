# W2 / R05B Low-Friction Scope-Repair Re-Adjudication (independent, read-only)

- **Reviewer role:** verify (adversarial, read-mostly), Fable slot
- **Repo:** `/Users/jrudnik/labs/jcode-w2-r05b`, branch `recovery/fix-r05b-spawn-reclaim-2026-07-15`
- **Final HEAD adjudicated:** `f8c5f8204056ff783d99769e4088e7bcceb56d73` (`docs: record W2 low-friction scope repair`)
- **Base authority:** `602709895be96a85a6090690c0b27d5681d17321`; `docs/fork/recovery/RECOVERY_PLAN.md` (W2 at :97-103, gate rules at :215); `docs/fork/recovery/seams/R05B-worker-dispatch-reclaim/ledger.md`
- **Prior FAIL under repair:** Opus scope adjudication at `66cc39541` head `a342cd5fb` (`reviews/2026-07-16-w2-scope-adjudication.md`, SHA-256 reproduced `b44b7acd0324...3b2b` prefix `b44b7acd0324a4fe76bf1696f4d44792b56832396b7c00884ef3a3b1e3be9a2b`)
- **Repair commits reviewed:** `6dfe2cdb6` (fix), `f13620596` (test), `f8c5f8204` (docs)
- **Constraints honored:** no file edits in the repo; offline-only deterministic checks with `FORK_NUDGE_MAX_AGE=2147483647 FORK_NUDGE_AUTOSYNC=0 CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0` inside `nix develop --offline`; no live daemon/terminal/network/credentials.

## Top-line verdict

**PASS — confidence HIGH (scope), MEDIUM-HIGH (ledger completeness, one durability gap noted below).**

The low-friction repair fully removes the protocol/replay widening that caused the Opus FAIL. At final HEAD, `crates/jcode-protocol/src/wire.rs`, `crates/jcode-app-core/src/server/swarm_mutation_state.rs`, and `swarm_mutation_state_tests.rs` are **byte-identical to base** (verified by git blob-hash equality, not diff emptiness). `PROTOCOL_VERSION` remains `1`. Every remaining changed path is R05B-owned spawn/reclaim behavior, its tests, or append-only recovery docs. No R03A, R04, R05A, R06A, or durable-schema crossing remains. The append-only ledger accurately preserves both the failed status-event fixture attempt and the successful 14-check rerun, corroborated by raw logs — though those raw logs are currently **untracked**, which is the one finding worth acting on.

---

## 1. Removal of the widening — verified (HIGH)

| Check | Evidence | Result |
|---|---|---|
| `wire.rs` restored | `git rev-parse 602709895:...wire.rs == f8c5f8204:...wire.rs` — identical blob | PASS |
| `swarm_mutation_state.rs` restored | identical blob vs base | PASS |
| `swarm_mutation_state_tests.rs` restored | identical blob vs base | PASS |
| `CommSpawnResponse` shape | at HEAD: `id`, `session_id`, `new_session_id`, `initial_prompt_delivered` only (`wire.rs:1492-1498`); at `66cc39541` it carried the 3 optional fields | PASS |
| Forbidden symbols | `grep -rn 'requested_spawn_mode|spawn_fallback_detail|SwarmSpawnOutcome' crates/` — zero hits. The only surviving `resolved_spawn_mode` occurrences are **local variable names** in `comm_session.rs:642,682,712`, not fields | PASS |
| `PROTOCOL_VERSION` | `crates/jcode-protocol/src/lib.rs:26` = `1` at base and HEAD | PASS |
| `spawn_swarm_agent` return type | `anyhow::Result<String>` at HEAD (`comm_session.rs:607-630`), matching pre-widening shape; `handle_comm_assign_next` consumes the plain session ID (`comm_control.rs:2058-2065`) | PASS |
| Tool consumer | `communicate.rs` spawn-response handler back to `{ new_session_id, .. }` with plain "Spawned new agent" output | PASS |
| Durable replay | no `PersistedSwarmMutationResponse` field additions anywhere; `SwarmTaskProgress` (jcode-plan) gained **no new fields** — provenance is appended into the existing `checkpoint_summary` string via `append_progress_provenance` | PASS |

The repair diff `66cc39541..f8c5f8204` touches exactly: `comm_control.rs` (revert to plain session ID), `comm_session.rs` (drop `SwarmSpawnOutcome` plumbing, keep creation-resolution logic), `comm_session_tests.rs` (drop response-field assertions, keep member-detail/history/event assertions), `swarm_mutation_state{,_tests}.rs` (full revert), `communicate.rs` (revert response rendering), `wire.rs` (full revert), plus the append-only ledger amendment. Nothing else.

## 2. Changed-path audit vs declared W2 surface (base `602709895` → HEAD)

Declared surface (`RECOVERY_PLAN.md:102`): `crates/jcode-app-core/src/server/{comm_control,comm_session,swarm*}.rs`, `crates/jcode-plan` (read-mostly), `tool/communicate.rs` dispatch portions; plus global-rule targeted tests and append-only docs.

| Path | Class | Verdict |
|---|---|---|
| `server/comm_control.rs` | production | In surface. Stale-takeover history preservation (entry/or_default + `append_progress_provenance` at :1723-1750) and liveness delegation (:734 → `super::swarm::member_status_is_dead`). R05B-owned. |
| `server/comm_session.rs` | production | In surface. `resolve_swarm_spawn_creation` (Visible fail-closed, Auto labeled fallback), `auto_fallback_status_detail`/`auto_fallback_member_label`, detail-survival on running/ready/failed status updates. All internal member-detail/label/event observability, no protocol change. One `#[cfg(test)]` env hook `JCODE_TEST_VISIBLE_SPAWN_ERROR` (test-compile only). R05B-owned. |
| `server/swarm.rs` | production+tests | In surface. `member_status_is_dead` delegates to `swarm_verbs` (:372-373); cap-fail salvage appends instead of overwriting (:451-461); large additions are `mod tests` only (hunks at :2059+, :3011+ are inside tests). R05B-owned. |
| `swarm_verbs.rs` | production | **Not in the literal surface glob** (it lives at `src/swarm_verbs.rs`, not `src/server/`). See finding L1. Change is a rename `is_failed` → `member_status_is_dead` with identical match arms, `pub(crate)`, required by ledger fixture 4 ("remove hand-written dead triples", third copy located by the ledger at `swarm_verbs.rs:55`). Semantics unchanged. |
| `tool/communicate.rs` | production | In surface (dispatch portion). Residue-policy text added to the churn abort diagnostic; `max_created_sessions_before_abort` helper is `#[cfg(test)]`. R05B-owned. |
| `jcode-plan/src/lib.rs` | production+tests | In surface ("read-mostly"). One small write: `append_progress_provenance` + `reclaim_stranded_assignment` now appends. This is exactly ledger slice 2's named fix to `jcode-plan::reclaim_stranded_assignment`; no schema field added. |
| `comm_control_tests.rs`, `comm_control_tests/assign_task.rs`, `comm_session_tests.rs`, `communicate_tests.rs`, `communicate_tests/end_to_end.rs` | tests | Targeted slice tests per global rule. Fine. |
| `reviews/2026-07-15-w2-grok-review.md`, `reviews/2026-07-16-w2-scope-adjudication.md` | docs | Byte-preserved review artifacts; both SHA-256 hashes independently reproduced (`6b3df2d0...`, `b44b7acd...`) and match the ledger citations. |
| `seams/R05B.../ledger.md` | docs | Append-only: **zero deleted lines** across base→HEAD (`git diff | grep -c '^-[^-]'` = 0). |

**Untouched, confirmed:** `crates/jcode-protocol`, `crates/jcode-tui`, `crates/jcode-client`, `crates/jcode-config-types`, session/process lifecycle files, control-log/fold code, durable evidence storage.

## 3. Cross-seam crossing audit — none remaining

- **R03A (wire):** `jcode-protocol` diff vs base is empty; no `ServerEvent` variant/field changes anywhere; `PROTOCOL_VERSION=1`. The response-leg observability is explicitly deferred to R03A in the ledger amendment. **Clean.**
- **R04 (process/session lifecycle):** status vocabulary unchanged (`failed|stopped|crashed`); no lifecycle writer modified. The dead-PID chain fixture *consumes* R04 APIs (`mark_active_with_pid`, `sweep_dead_pid_swarm_members`) read-only in tests. **Clean.**
- **R05A (DAG/control-log):** no control-log/fold/replay code touched; both R05A entry-criterion tests (`control_log_fold_tracks_maps_through_handler_sequence`, `scan_from_tail_offset_finds_artifact_once`) are green in the preserved rerun log. **Clean.**
- **R06A (durable evidence schema):** no persisted schema widening. `PersistedSwarmMutationResponse` restored to base. `SwarmTaskProgress` field set unchanged; provenance appends reuse `checkpoint_summary` (` | `-joined), which is a *content* convention, not a schema change, and is exactly slice 2's prescribed "append, not substitute" design. Slice-2's stop condition ("stop if durable schema/replay ownership crosses R06A") is **not** triggered at this HEAD; it *was* triggered by the now-removed replay widening. **Clean.**
- **R09:** no `--update` recorded anywhere; expected-red budgets carried with per-file W2 attribution in the ledger. Not re-executed by me (see "not checked").

## 4. R05B behavior retention — verified in source and by rerun

All of the following were confirmed present at HEAD in the diff and re-executed green by me (fresh offline runs, not just log reading):

| Behavior | Fixture | My rerun |
|---|---|---|
| Explicit `Visible` fails closed on `Err` and `Ok(false)` | `explicit_visible_launch_{false,error}_fails_closed...` (`visible_launch` filter) | `2 passed` |
| Auto fallback labeled, detail survives prompt status updates, join in history + live event stream | `handle_comm_spawn_auto_fallback_preserves_history_and_detail_with_prompt` | `1 passed` |
| Stale direct takeover preserves heartbeat/detail/checkpoint/reclaim counts, appends takeover provenance, clears `stale_since` | `assign_task_stale_direct_takeover_preserves_progress_history` | `1 passed` |
| Primitive reclaim appends (jcode-plan) | full `jcode-plan --lib` | `79 passed / 0 failed` |
| Salvage requeue + cap-fail preserve history and append reason | `salvage_requeues_dead_members_tasks_and_notifies_coordinator`, `salvage_fails_task_once_reclaim_cap_is_reached` | `1 passed` each |
| One liveness authority (server + verbs agree; assign path delegates) | `member_status_is_dead_matches_terminal_non_success_states`, `f1_assign_next_reclaims_task_from_departed_assignee` | `1 passed` each |
| Configured-concurrency churn-to-abort, bound `2*3=6` provider calls, residue policy in diagnostic, default cleanup leaves zero failed workers | `communicate_run_plan_churns_to_abort_at_configured_concurrency_and_cleans_failed_workers` | `1 passed` |
| R04→R05B dead-PID chain: crashed status, exactly one requeue, coordinator notify, no duplicate assignment, history preserved | `dead_pid_sweep_then_salvage_requeues_once_without_duplicate_assignment` | `1 passed` |

## 5. Ledger accuracy: failed fixture and 14-check rerun

- **Append-only:** zero deletions in `ledger.md` across the whole base→HEAD range and across `66cc39541→f8c5f8204`. The Grok FAIL and the Opus scope FAIL amendments remain intact and are explicitly not rewritten.
- **Failed status-event fixture:** the ledger's "Preserved failed validation attempt" section matches the raw log (`failed-status-event-attempt.log.gz`): test `handle_comm_spawn_auto_fallback_preserves_status_history_and_detail_with_prompt` panicked at `comm_session_tests.rs:834` with `Elapsed(())` after 5.30s, `0 passed; 1 failed; 1101 filtered out`. The ledger's characterization (a new automatic `SwarmStatus` delivery expectation exceeding the bounded contract, narrowed by removing only that assertion, no source regression implicated) is consistent with the log and with the final test body, which retains detail/headless/prompt-delivered/history/event-stream assertions and drops only the unregistered-receiver snapshot-delivery wait.
- **14-check rerun:** `authoritative-offline-rerun.log.gz` contains exactly 13 `test result: ok` sections under named banners matching the ledger's 13 fixture rows (including both R05A entry tests) plus one `package_check` section ending `Finished dev profile` — 14 checks total, with pass/filter counts (`1101`/`1100`/`78 filtered`) matching the ledger table exactly.
- **Ledger factual claims spot-verified:** "wire.rs and swarm_mutation_state{,_tests}.rs restored to pre-W2 content" — true (blob identity). "No requested_spawn_mode, spawn_fallback_detail, or SwarmSpawnOutcome symbol under crates/" — true. "PROTOCOL_VERSION remains 1" — true. Both preserved review hashes reproduce.

## 6. Findings by severity

**HIGH:** none.

**MEDIUM:**
- **M1 — Repair evidence directory is untracked at final HEAD.** `docs/fork/recovery/evidence/2026-07-16-w2-scope-repair/` (README, MANIFEST.sha256, both logs, scope-state.txt) exists only as untracked files (`git status` shows `??`), and neither the ledger nor `PROGRESS.md` references it (`grep '2026-07-16-w2-scope-repair'` in both: no match; its own `scope-state.txt` says `PENDING_EVIDENCE_PATH ...`). The ledger *text* is self-contained and accurate, so the adjudicated record survives, but the raw failed-attempt log — the primary artifact backing the "preserved failure" claim — has no git durability yet. Follow-up: commit the evidence directory (one docs commit) and cross-reference it from the ledger.

**LOW:**
- **L1 — `swarm_verbs.rs` sits outside the literal W2 surface glob.** `RECOVERY_PLAN.md:102` names `server/{comm_control,comm_session,swarm*}.rs`; `src/swarm_verbs.rs` is not under `server/`. However, the R05B ledger (the fixture authority the plan defers to) explicitly locates the third copied dead-triple at `swarm_verbs.rs:55` and fixture 4 requires removing hand-written triples and proving verb/report agreement, which is impossible without touching this file. The change is a semantics-preserving rename/visibility widening only, was present since original W2 commit `5ae37a297`, and the prior Opus scope adjudication (which hunted exactly this class of violation) did not object. I read this as a ledger-sanctioned, R05B-owned implicit surface extension, not a seam crossing. Worth one clarifying sentence in the ledger if W2 proceeds.
- **L2 — No `PROGRESS.md` checkpoint for any W2 amendment.** The global slice rule requires a `PROGRESS.md` checkpoint "on completion"; W2 is not complete/integrated (fresh independent review still pending per the ledger), so this is defensible, but the coordinator should ensure a checkpoint lands when W2 closes.
- **L3 — Two production `expect()` calls added** in `resolve_swarm_spawn_creation` (`visible/auto mode should attempt visible spawn`). Locally sound (the `Option` is `Some` by construction of the preceding match), counted in the ledger's panic-budget attribution, but an enum carrying mode+result together would eliminate the panic path.
- **L4 — Test-only runtime env hook in production file.** `JCODE_TEST_VISIBLE_SPAWN_ERROR` in `comm_session.rs:134-137` is `#[cfg(test)]`-gated so it cannot ship, but it is a seam for test-vs-prod drift; a launcher-injection parameter would be cleaner.
- **L5 — Odd bound arithmetic in the churn fixture.** `configured_concurrency * RunPlanChurnGuard::max_created_sessions_before_abort(1)` computes 2×(1×3)=6 rather than the more direct `max_created_sessions_before_abort(2)`. Equivalent value, mildly misleading expression.
- **L6 — Ledger symbol-search claim is precise but narrow.** It claims absence of three symbols and deliberately omits `resolved_spawn_mode`, which survives as a local variable name. Not misleading once checked (no field/protocol/persisted use), but a reader could over-read the claim.

## 7. Residual risks

- The response-leg spawn-mode observability (requested vs resolved on `CommSpawnResponse`) is now genuinely deferred; API consumers cannot distinguish requested from resolved mode from the response alone. This is exactly slice 1's stop condition and is correctly parked at R03A. Operators do get member-detail/label/event observability.
- Fallback detail is folded into the member label/detail strings; a future status writer that unconditionally replaces `detail` could clobber it. The fixture pins current writers only.
- Churn bound covers `run_plan`-driven creation; the live-member cap still does not bound residual on-disk session files outside the run_plan path (pre-existing, acknowledged in the ledger).
- R09 expected-red totals (panic 31→48, swallowed 2987→3074, size budgets) grew under W2 and were recorded pre-repair; the repair removed some code, so those totals in the ledger are slightly stale-high. Not re-enumerated.
- R05B remains `blocked` for swarm widening; this PASS is a scope verdict, not swarm-pilot authorization, and a fresh independent behavioral review is still required per the ledger's own text.

## 8. What I checked (methods)

- Blob-hash identity (not diff text) for the three restored files at base vs HEAD.
- Full diff read of every changed production path base→HEAD and of the repair range `66cc39541→f8c5f8204`.
- Repository-wide greps for the removed symbols, protocol fields, `deny_unknown_fields`-relevant shapes, and liveness-predicate copies.
- SHA-256 reproduction of both preserved W2 review artifacts.
- Append-only verification (zero deleted ledger lines, both ranges).
- Evidence-log inspection: failed-attempt log tail, 13 ok-result count, banner names, filter counts, package_check completion.
- Fresh offline deterministic reruns under the mandated env: full `jcode-plan --lib` (79/0) and 9 targeted app-core fixtures (all green), each single-filter with `--test-threads=1` where the ledger did.

## 9. What I did NOT check

- Full `jcode-app-core` test suite or workspace-wide `cargo test` (only the 10 commands above).
- R09 gate scripts (panic/swallowed/size/warning/dependency/wildcard) — I relied on the ledger's recorded runs.
- rustfmt/pre-commit hook execution.
- Intermediate commit states between base and `66cc39541` (adjudicated previously; I audited endpoints and the repair range).
- Any live daemon, terminal spawn, visible client, network, credential, MCP, reload, or replay-against-old-binary compatibility test (prohibited and unnecessary here since the wire is byte-identical to base).
- The Grok behavioral-review claims themselves (that review's PASS/FAIL basis was superseded by the scope path; a fresh behavioral review remains the ledger's stated next step).

## 10. Disposition

**PASS.** The low-friction repair does exactly what the Opus FAIL demanded and nothing it forbade: the protocol and durable-replay widening is fully reverted (byte-identical files, `PROTOCOL_VERSION=1`), all retained changes are R05B-owned spawn/reclaim safety plus member-detail/event-history observability that predates or stays inside the declared surface, and the append-only ledger truthfully records both the failed fixture attempt and the successful 14-check rerun. Recommended follow-ups before W2 closure: commit the untracked evidence directory (M1), add one ledger sentence covering `swarm_verbs.rs` (L1), and proceed to the fresh independent behavioral review the ledger already requires.
