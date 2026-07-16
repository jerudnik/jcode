# R05A Plan, DAG, and control-log semantics: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light` |
| Research budget | `6 decisive checkpoints; 6 consumed without expansion` |
| Authority challenged | Fork is the only fixed-ref implementation of the control log. The DAG retry semantics changed on both fork and upstream, so upstream provenance was not accepted as authority. |
| Recommended disposition | `retain-fork` |
| Confidence | high for pure DAG, log fold, and tested dual-write topology; medium for every exceptional server mutation and multi-host ordering (gaps below) |

R05A owns dependency readiness, graph/node transitions, append-only control-event vocabulary and fold, replay, task artifacts, and coordinator control state. It excludes process spawn and worker health/reclaim/backoff (R05B), session/process lifecycle and restart policy (R04), and all render state (R08B). R00, R09, and R11 bind this ledger. The R05 split is deliberate: `RESPONSIBILITIES.md:27-28` assigns graph truth here and incident-bearing worker dispatch to R05B; the only cited forkbomb incident attributes unbounded process/session growth to MCP registration and worker lifecycle, not to the pure graph state machine.

## Six-checkpoint evidence ledger

| # | Finding | Fixed-ref evidence and deterministic reproduction | Consequence |
|---:|---|---|---|
| 1 | The control log is fork-only, while DAG retries are contested two-sided behavior. | `git cat-file -e <ref>:crates/jcode-swarm-core/src/control_log.rs` is present only at fork `7ff4fc6be`, absent at merge base and upstream `802f69098`. `git diff --numstat 631935dd..7ff4fc6be -- crates/jcode-plan/src/dag` is `228` insertions/`19` deletions; upstream is `170`/`18`. | Retain the fork log, but do not call either DAG side authoritative merely because of ancestry. The disposition is one bounded `retain-fork`, not an unreviewed compose/adopt claim. |
| 2 | DAG transitions enforce readiness, ownership, acyclicity, and retry-safe seed replay. | Fork `crates/jcode-plan/src/dag/ops.rs:14-75` validates a full seed on a clone, treats an exact re-seed as a no-op, rejects changed definitions, and commits only an acyclic graph. `:220-260` requires an owned running non-gate node before expansion; `:377-435` completes only an owned running node. Tests `dag/tests.rs:45-109`, `:145-175`, `:180-235`, and `:275-279` cover replay, readiness/dispatch, ownership, and light-mode completion. | A retry cannot silently overwrite a definition or partially mutate a graph. R05B chooses who is dispatched or reclaimed, but cannot redefine these transition preconditions. |
| 3 | The control event vocabulary is append-only and its fold is deterministic. | Fork `jcode-swarm-core/src/control_log.rs:32-89` defines the member, task, heartbeat, removal, rename, and `ArtifactFiled` events. `:144-235` derives coordinator state deterministically and folds events; `:237-354` computes sorted deltas; `:356-414` appends one flushed JSONL line with per-origin sequence. `:19-21` specifies a torn final line is ignored. | Control state is reconstructible from events, and artifact evidence is distinct from an unqualified terminal-status string. Event variants must only be added, never repurposed or removed. |
| 4 | Server state has multiple in-memory writers, but one persistence/sync funnel records member/task deltas and a single explicit artifact writer. | `control_log_sync.rs:1-20,129-220` projects maps, calls `diff_events` from `persist_swarm_state_for`, updates the cached fold on successful append, and exposes only `append_control_event` for non-derivable `ArtifactFiled`. `comm_graph.rs:210-557` mutates graph state through seed, expand, complete, and gap handlers; `comm_plan.rs:44-480` owns proposal/approval map changes; `comm_control.rs:1314-2344` changes role, assignment, and task-control maps; `swarm_mutation_state.rs` and `swarm.rs` change membership/status/progress; `swarm_persistence.rs:57-83,195-205` persists snapshot/control-log offset. | The state-writer census rejects the false claim that a single map is authoritative. The control-log writer is the only durable control-event writer, while the named handlers are map writers obligated to reach the sync/persist funnel. |
| 5 | Snapshot restart replay is bounded by an offset and preserves graph/control truth, while lifecycle interpretation stays outside R05A. | `swarm_persistence.rs:74-82` records `control_log_covered_offset`; `:195-205` locates the adjacent JSONL log; `:277-289` marks running work stale after restart. `swarm_persistence_tests.rs:30-144` verifies snapshot round trip, preserved coordinator/progress, and stale recovery. | R05A owns snapshot-plus-tail replay semantics. R04 decides process/session consequences of restart; R05B decides whether a stale assignment may be reclaimed. |
| 6 | Offline fixtures exercise the pure fold, retry behavior, and real handler sequence without a daemon, provider, MCP server, network, or credentials. | With `CARGO_NET_OFFLINE=true`, disposable `JCODE_HOME` and `JCODE_RUNTIME_DIR`, `bash scripts/dev_cargo.sh test -p jcode-plan --lib` passed `79/79`; `bash scripts/dev_cargo.sh test -p jcode-swarm-core --test control_log_properties` passed `2/2` (`fold_agrees_with_task_graph_driven_through_engine_ops`, `replay_matches_in_memory_fold_for_arbitrary_sequences`). The app-core target contains `control_log_fold_tracks_maps_through_handler_sequence` (`control_log_dual_write.rs:14-206`) and `scan_from_tail_offset_finds_artifact_once` (`control_log_scan.rs:12-91`) with temp runtime guards. | This establishes deterministic no-network fixture coverage for R05A. It does not exercise worker processes or live swarm admission, which remain excluded. |

## State-writer and cross-seam contract

| Surface | Writer/reader authority | Contract and boundary |
|---|---|---|
| DAG definition and node metadata | `jcode-plan` validated operations, invoked by `comm_graph` | Seed/expand/complete/inject must preserve idempotence, owner checks, dependency readiness, and acyclicity. Render projections are R08B, not a state authority. |
| Live member/task maps | `comm_graph`, `comm_plan`, `comm_control`, `swarm_mutation_state`, and `swarm` | Every persisted mutation must converge via `persist_swarm_state_for` and `control_log_sync`; new direct map mutation without that funnel is an R05A durability defect. |
| Durable control state | `ControlLogWriter` through `sync_swarm_control_log*` and the explicit `append_control_event` artifact path | `fold(control log) ==` the projected member/task views at a successful sync. The watch channel is only a wake nudge, never truth (`control_log_sync.rs:46-55`). |
| Restart/replay | `swarm_persistence` snapshot plus log tail | Snapshot cursor plus replay must not duplicate, resurrect tombstoned state, or lose an artifact. R04 owns lifecycle action after recovery. |
| Worker/process effects | R05B and R04 | R05A may expose a stale/failed/queued graph state but may not spawn, assess liveness, choose spawn mode, reclaim, or back off. The MCP forkbomb incident therefore escalates R05B/R04, not this ledger. |
| Tool/network boundary | R07A/R07B/R07C | This ledger's fixtures have no tools. If a graph driver opens tools/MCP, discovery, or network admission, stop and escalate to R07A/R07B; consent/telemetry remains R07C. |
| Evidence and docs | R06A, R11, and R12 | `ArtifactFiled` is control-plane summary evidence, not the artifact body or session transcript. R06A owns durable evidence schema, R12 owns agent-turn request/result emission, and R11 preserves this ledger, incident links, and explicit gaps. |

## Pilot relevance, negative findings, and R09 debt

- **Pilot relevance and defer boundary:** `RESPONSIBILITIES.md` marks R05A `Pilot: no`. The smallest Phase 3 pilot is explicitly no-tool and does not run swarm work. Defer all R05A runtime exercise unless the pilot becomes swarm-driven. A live worker, worker reclaim, spawn mode, tool/MCP, discovery, network, daemon, or credential requirement is a stop-and-escalate boundary, not permission to widen this seam.
- **Negative findings:** no `control_log.rs` counterpart exists at upstream or merge base; no source path differs between fork baseline and this ledger head (`git diff --name-only 7ff4fc6be..HEAD -- crates scripts` was empty before authoring); no live daemon, network, credentials, MCP server, or process was started; no render path was used as truth; and no artifact body is placed in the control log.
- **R09 debt:** R09 requires debt to follow behavioral ownership and forbids `--update`. Existing archive inventory names oversized `comm_plan.rs` and `swarm_persistence.rs` plus panic/expect hardening candidates, but this ledger neither reclassifies nor hides them. Before an R05A implementation change, enumerate the exact production-size, test-size, panic, and swallowed-error deltas for the touched transition/fold paths and run the R09 classifier/ratchets without `--update`. Spawn/reclaim debt belongs to R05B, lifecycle debt to R04.
- **Incident finding:** `docs/architecture/MCP_SERVE_FORKBOMB_INCIDENT.md` names recursive MCP registration, process/session caps, and dead-PID sweep amplification. It is evidence for the R05A/R05B boundary, not evidence that a graph transition is safe or unsafe.

## Disposition and conditions

- **Recommended disposition:** `retain-fork`. The fork-only append-only control log, deterministic fold, offset replay, and graph retry protections have direct source and offline fixture evidence. Retention is narrowly about R05A semantics, not a claim that all fork swarm operations or either side's two-sided DAG divergence is best.
- **Acceptance or retirement condition:** retain until a bounded change proves, with the same offline fixtures, that `fold(log)` equals projected maps after seed, assign, member-status, complete/artifact, heartbeat, snapshot reload, and tail replay. Retire only by deleting the full control-state mechanism with an equivalent deterministic, replayable replacement and an R00-recorded migration/rollback plan.
- **Rollback or stop conditions:** stop before any implementation when an event must be repurposed, a replay is non-idempotent, a mutation bypasses sync/persistence, a snapshot/tail produces divergent coordinator/task state, a fixture needs a real daemon/network/credential/tool/MCP process, or the six-checkpoint scope expands. Roll back an isolated change if its graph/control fixtures regress, leaving the log and snapshot format unchanged.
- **Escalate to full review if:** a multi-host merge ordering rule becomes executable rather than advisory; a new state writer cannot be proven to use the synchronization funnel; event-schema migration or compaction changes old-log replay; a graph transition becomes implicated in a live incident; or a swarm-driven pilot needs R05A behavior.
- **Coordinator approval:** pending.
- **Fable review:** pending.

## Explicit gaps

- The full exceptional-handler census was not executed under failure injection. The named map writers were inspected, but error paths that log and continue in `control_log_sync.rs:171-183,199-212` need a full review if durability guarantees are tightened.
- Multi-origin envelope ordering is only future-facing documentation (`control_log.rs:12-17,91-103`), not a tested conflict-resolution protocol.
- The deferred R05B/reclaim behavior, R04 process recovery, R07A tool/MCP lifecycle, R07B discovery/network admission, R07C telemetry, R06A artifact-body persistence, R12 agent-turn emission, and R08B rendering were intentionally not tested.
- The two app-core integration tests listed at checkpoint 6 remain required evidence for the next swarm-driven change. They use temporary runtime directories and no external service, but their offline invocation timed out after 600 seconds waiting on the shared build target, so no app-core pass is claimed.

## 2026-07-15 W0 approval amendment

Coordinator approval: **PASS as a `retain-fork` light record**. The independent five-ledger review is [`../../reviews/2026-07-15-remaining-light-ledgers-opus-review.md`](../../reviews/2026-07-15-remaining-light-ledgers-opus-review.md), SHA-256 `b537bc5674fdb9385e60c2dd18a44db5e61ba4f57146cd57fbf91f7a58a8a55d`.

The stale Fable-pending line is discharged by corrected Phase 4 Fable plan SHA-256 `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` and independent plan review SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92`. No source change or swarm exercise is authorized. The two app-core integration tests remain W2 entry criteria exactly as recorded in `RECOVERY_PLAN.md`.
