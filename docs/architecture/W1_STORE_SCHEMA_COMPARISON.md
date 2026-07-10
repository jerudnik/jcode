<!-- Promoted 2026-07-10 from the closed notes workstream projects/jcode/proposals/orchestration-hardening/_attachments/ (W1 control-log store decision). Durable design/runbook home is here; PM history remains in notes archive. -->

---
title: "W1 store schema decision: SQLite vs event log vs session substrate"
type: note
created: 2026-07-04
updated: 2026-07-04
tags: [jcode, swarm, orchestration, w1, architecture, decision]
---

# W1 command-as-state: store schema comparison

The proposal mandates this comparison BEFORE W1 code. This is that note.
Written after the failure-scoreboard session, so it argues from the five
fixed defects (evidence in `failure_scoreboard.rs`), not from taste.

## What exists today (survey of the fork, 2026-07-04)

| Mechanism | Location | Durability | Shape |
|---|---|---|---|
| Swarm snapshot | `server/swarm_persistence.rs` | JSON file per swarm (`jcode-swarm-state/<id>.json`), whole-state overwrite on mutation | Mutable snapshot |
| Event history | `server/state.rs` (`MAX_EVENT_HISTORY = 5000`) | In-memory ring only, lost on restart | Bounded log, no offsets |
| Await state | `server/await_members_state.rs` | JSON file per await key | Mutable snapshot |
| Mutation replay | `server/swarm_mutation_state.rs` | JSON file per request key | Idempotency ledger (log-ish!) |
| Sessions | `jcode-base/session.rs` | JSON file per session, append-mostly messages | Message log inside a snapshot wrapper |

No sqlite dependency exists anywhere in the workspace today. Note the
irony: the codebase already contains four ad-hoc persistence mechanisms,
which is itself the W1 argument.

## The three candidates

### A. SQLite (riptide-style status columns)

Rows: `swarm_session(id, swarm_id, role, status, owner, heartbeat_at,
command)`, `dag_node(id, swarm_id, status, owner, artifact, ...)`.
Commands are status-column writes (`interrupt_requested`, `resuming`);
workers poll/reconcile.

- (+) Queries are direct: every scoreboard fix needed a *query over
  current state* (F1: which assignees are live members; F2: which
  sessions hold non-terminal tasks; F3: is the owner still a member;
  F4: is the coordinator entry live). A table answers these in one
  statement, no replay.
- (+) Transactions kill the read-modify-write races that produced F4
  (coordinators map and role string updated separately).
- (+) riptide prior art: heartbeats + orphan detection are literally
  `WHERE heartbeat_at < now - 15s`.
- (-) New dependency (rusqlite) in a workspace that has none.
- (-) History is gone unless you also keep an audit table (which is a
  log, so you end up building candidate B anyway, badly).
- (-) Cross-machine sync (daemon hierarchy experiment) needs a
  replication story SQLite does not give you.

### B. Event log with offset resume (+ materialized view)

Append-only per-swarm log: `AssignmentWritten`, `NodeCompleted`,
`RoleChanged`, `HeartbeatMissed`, ... Current state is a fold; consumers
carry offsets.

- (+) Staleness becomes *detectable* rather than silent. Every one of the
  five failures was a stale-snapshot bug: a decision made on partial or
  outdated state. A consumer that knows its offset knows it is behind.
- (+) W2's "append-only per-session event log with offset resume" and W5's
  operator UI replay are the same mechanism; wedge recovery = re-fold.
- (+) Cross-machine sync is log shipping, nearly free (daemon experiment).
- (+) The mutation-replay ledger shows the codebase already reinvented a
  log where correctness demanded it.
- (-) Every current-state question needs the materialized view, and the
  view is exactly the kind of second-source-of-truth that caused F4,
  unless it is derived ONLY from the log (never written directly).
- (-) Compaction/snapshotting is a real design burden (5000-event ring
  exists because unbounded logs were already a concern).
- (-) More novel code than either alternative; nothing in the workspace
  folds events today.

### C. Session substrate (reuse `Session` JSON files)

Swarm control state as messages in a control session.

- (+) Zero new mechanisms; reload/resume machinery exists.
- (-) Sessions are transcripts, not state: no queries, no transactions,
  no offsets (messages have indexes but consumers do not track them).
- (-) Couples control-plane schema evolution to transcript schema.
- (-) The scoreboard queries (live members, non-terminal tasks) would be
  full-file scans of JSON transcripts. This is the current design with
  extra steps.

Verdict on C: reject. It is the status quo's failure mode formalized.

## Decision (proposed, needs operator sign-off before code)

**B with a strict rule, phased through A's query shapes:**

1. The log is the ONLY writable surface. `assign`, `complete_node`,
   `assign_role`, heartbeats are events appended to a per-swarm log
   (JSON-lines file first; the format matters less than the discipline).
2. Current state is a fold, cached in memory, rebuilt on restart by
   replay. The fold output is exactly today's `SwarmState` maps, which
   means the scoreboard tests run unchanged against the new engine.
3. Consumers (run_plan driver, await watchers, future daemons/UI) carry
   offsets. `await_members` becomes "wake me when an event past offset N
   satisfies P" - which retires F2's entire class: no more status-string
   sampling.
4. SQLite is NOT adopted in phase one. If fold/query performance or
   concurrent-writer pressure demands it, the log moves into a
   two-table SQLite (events + snapshot) WITHOUT changing the event
   schema. The event schema is the durable decision; the container is
   reversible.

Why not A outright: the audit's root cause was "RPC against in-memory
state with multiple sources of truth". A mutable table fixes the
durability but keeps state transitions as in-place overwrites, so
debugging a wedge still means guessing what happened. The log IS the
diagnostic. We just spent a session reverse-engineering five wedges from
symptoms; the log would have made each a `grep`.

## Migration sketch (for the W1 session)

1. Define `SwarmControlEvent` enum + JSONL writer/reader with offsets
   (new crate or module in jcode-swarm-core; no server coupling).
2. Property test: fold(replay(log)) == in-memory maps after arbitrary
   op sequences (use the existing dag sim as the op generator).
3. Dual-write behind a flag: mutations append events AND update maps;
   assert equivalence in tests. Scoreboard must stay green.
4. Flip reads: restart recovery replays the log instead of loading
   snapshot JSON (keep snapshot as a compaction checkpoint).
5. Retire per-seam shims (the F1/F4 reconciliation patches) once their
   scoreboard tests pass against the fold.

## Open questions for the operator

- Event log per swarm or one global log with swarm_id field? (Per-swarm
  files match current persistence layout; global simplifies the daemon
  hierarchy super-coordinator.)
- Retention: compaction checkpoint cadence, and whether completed-swarm
  logs archive to the notes/exocortex side.
- Does W2's per-session event log share the same event enum, or is
  control-plane vs transcript-plane separation worth two schemas?

## DECISION (operator sign-off, 2026-07-05)

Path B approved. Additional framing from the operator: the fork is
trending toward a multi-host architecture with observation, evaluation,
and harness-dynamism proposals downstream. Those are NOT being
implemented now, but W1 decisions should be the kind that FIT that
trajectory rather than fight it. Resolutions for the open questions
under that lens:

1. **Log topology: per-swarm log files, host-agnostic event schema.**
   Per-swarm files keep the current persistence layout and make archive/
   GC trivial (a completed swarm is one file). Multi-host readiness comes
   from the schema, not the file layout: every event carries
   `origin` (host/daemon identity) and a per-origin monotonic sequence,
   so per-swarm logs can later be merged/shipped across hosts without
   rewriting history. A future super-coordinator consumes per-swarm logs
   the same way a local fold does.
2. **Retention: snapshot-checkpoint + truncate, archive on completion.**
   Compaction writes a fold snapshot (the existing swarm_persistence JSON
   shape is already exactly this) plus the log offset it covers; the log
   before that offset can be dropped or archived. Completed-swarm logs
   are kept whole - they are the observation/evaluation dataset the
   later proposals will want. Archive location decision deferred until
   the observation proposal is real.
3. **One event enum, two planes.** Control-plane events (assignment,
   completion, role, heartbeat) and transcript-plane events stay separate
   TYPES but share the envelope (origin, seq, timestamp, session/swarm
   ids). Shared envelope is what observation tooling and cross-machine
   sync consume; separate payload types keep the control fold small and
   the schemas independently evolvable.

Design consequences adopted for the implementation:
- Event envelope from day one: `{origin, seq, wall_ms, swarm_id, event}`.
  Origin is a single-host constant today; it costs nothing now and saves
  a history rewrite later.
- The fold is a pure function `fold(events) -> SwarmControlState` in
  jcode-swarm-core (no server coupling), so a daemon on another host can
  reuse it unchanged.
- Offsets are explicit in the reader API (consumers name their position),
  which is the W2 resume mechanism and the future cross-host sync cursor.

## Implementation progress

- [x] Step 1: `SwarmControlEvent` + envelope + JSONL reader/writer with
  offsets (jcode-swarm-core::control_log) - MERGED to main (b83a11770;
  the orch/* branches were merged and deleted per the fork's
  three-branch rail policy; history is in the merge commits).
- [x] Step 2: fold + property test (dag sim as op generator) proving
  fold(replay(log)) == expected state over arbitrary op sequences.
- [x] Step 3: dual-write in the server mutation paths (merged to main,
  346dfe659). Implementation deviates from "behind a flag" deliberately:
  instead of instrumenting each mutation site, the sync appends the
  diff between fold(log) and the in-memory view at the two funnels every
  mutation already flows through - `persist_swarm_state_for` (members +
  tasks) and `broadcast_swarm_status` (members only, covers
  update_member_status/headless joins which never snapshot). New pure
  `diff_events` in jcode-swarm-core; two new event variants (TaskRemoved,
  MemberRenamed) make the fold total over plan swaps/renames. Equivalence
  asserts added to F1/F3/F4 scoreboard tests + a dedicated handler-sequence
  test. No flag: failures log-and-continue like snapshot failures.
- [x] Step 4: restart recovery replays the log tail (same merge). Snapshot
  carries `control_log_covered_offset` (the compaction checkpoint cursor);
  `load_runtime_state` replays events past it over the persisted records
  before the crash-recovery transforms. Pre-W1 snapshots (offset 0) replay
  the whole log idempotently (tested). Log-only members restore headless
  so recovery marks them crashed instead of inventing live sessions.
  Truncation policy: logs are NOT truncated at checkpoint yet - completed-
  swarm logs are the observation dataset per this record; live-log
  truncation deferred until file size is an observed problem.
- [x] Step 5: F1 reclaim + F4 coordinator repair are fold-derived (same
  merge). `reclaim_stale_plan_assignments` takes its live-member set from
  fold(log); `resync_plan` restores the coordinators map from the DERIVED
  `SwarmControlState::coordinator()` instead of trusting the requester's
  role string. Scoreboard tests pass unchanged. The coordinators map
  itself still exists (permission checks read it); full retirement is a
  follow-up once every coordinator read goes through the fold.

## W2 groundwork (2026-07-05, merged d371b60a8)

Landed ON the control log per the "if time remains" scope:

- `ArtifactFiled {task_id, session_id, confidence}` event, emitted by
  `handle_comm_complete_node` via `append_control_event` BEFORE the finalize
  sync (evidence ordered ahead of the derived status flip). Folds to
  `TaskControlState::last_artifact`; completed-without-evidence = turn
  boundary or salvage, not proof.
- `scan_from(path, offset, predicate) -> Found{next_offset}|NotYet{resume}`:
  the await-on-offset primitive. Tested: F2-class status noise does not
  satisfy an artifact await; the evidence event does; no double-wake.

NOT yet done (next W2 session): migrate `handle_comm_await_members` /
`AwaitMembersRuntime` from status-sampling + swarm_event_tx subscription to
scan_from cursors, and route low-confidence artifacts to gate injection.

## W2 await-on-log (2026-07-05, merged from orch/w2-await-on-log)

- [x] W2 wired: `handle_comm_await_members` / the await watcher now wake
  from the control log; `swarm_event_tx` is a nudge only.

Design resolution for the completion-coverage fork: the wake predicate
(`wake_relevant_event` in comm_await.rs) is deliberately NOT
artifact-only. Light-mode auto-complete, requeue/fail-no-artifact
(turn_end_disposition), and salvage-complete all reach terminal state
without the awaited member filing an artifact, so an artifact-only await
would hang forever on them. Wake-relevant events: `ArtifactFiled`,
TERMINAL `TaskStatusChanged`, `TaskAssigned`/`TaskRemoved` (they move the
F2 busy-gate), and member lifecycle. `TaskHeartbeat`/`RoleChanged`/
`MemberRenamed` are noise. The predicate gates RE-CHECKS only;
satisfaction is still the level check (`awaited_member_statuses` +
`mode_satisfied`), preserving F2 discriminator semantics exactly
(terminal-status for light mode, artifact evidence as the deep signal).

Mechanism:
- Per-path `tokio::watch` append notifiers in control_log_sync: every
  append funnel (`sync_*` diff writes, `append_control_event`) publishes
  the post-append offset. Watchers park on `changed()`; a lost broadcast
  can no longer lose a wake (the scan re-runs from the watcher's own
  durable cursor). Broadcast `Closed` disarms that select arm instead of
  killing the watcher.
- `PersistedAwaitMembersState.scan_offset` (#[serde(default)]): anchored
  to `current_control_log_offset` at await CREATION (ensure_pending_state).
  Legacy/pre-W2 states load as 0 and RE-ANCHOR to the current tail (not
  replay-from-zero, which would re-match pre-await history). Cursor is
  persisted at rest (NotYet), not per matched event.
- Low-confidence routing: satisfied awaits whose done members filed
  `ArtifactFiled{confidence: low}` get a summary note naming the node ids
  and pointing at inject_gap. The await still wakes; enforcement stays in
  the engine (`validate_gate_pass` UnaddressedLowConfidence + gate
  directives). Derived from fold(log), so salvage-filed artifacts count.

Tests (comm_control_tests/await_on_log.rs): light auto-complete and
salvage wakes with NO broadcast nudge (red pre-W2), legacy zero-cursor
re-anchor, low-confidence summary flag. Existing await reload/deadline/
resume tests pass unchanged. Suite: 910 app-core / 15+2 swarm-core / 61
plan, 0 failures.
