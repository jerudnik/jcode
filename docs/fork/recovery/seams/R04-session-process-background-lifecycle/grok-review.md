# R04 independent adversarial review, Grok-style

Review target: `/Users/jrudnik/labs/jcode-seam-r04` at `5baf343ba6da564afc3f6c58c5edca7a64d6e67f`.

Boundary: R04 lifecycle/session/process/task responsibility. I did not read `/tmp/jcode-r04-opus-review.md`. I made no repository mutations. This artifact is the only write target.

## Disposition

**Not pilot-ready on this review pass.** The implementation has strong, specific R04 fixes for reload gating, cancel handoff, reload recovery, and background orphan reconciliation. I did not find a clear fork-bomb-class lifecycle hole in the inspected paths. However, pilot approval should be withheld until R09 can produce at least one successful narrow lifecycle test run in this worktree and until reload-interrupted wait/tool terminal semantics are made unambiguous to downstream consumers.

Main blockers:

1. **Validation blocker:** both targeted Cargo attempts timed out during compile/build-lock work, so no deterministic R04 test completed in this review.
2. **Terminal semantics blocker:** reload-interrupted wait-like tools are intentionally written as non-error `Ok` evidence while the message says the underlying operation may still be running. This can become false terminal success if any UI, evidence, or automation consumer interprets `Ok` as operation completion rather than resumable handoff.

## Fixed-ref and scope evidence

Read-only ref check:

```text
HEAD=5baf343ba6da564afc3f6c58c5edca7a64d6e67f
branch=recovery/seam-r04-20260715
base=631935dd1d3b2e31e167e2b12ad463e54bcf4b8d
status=
```

The provided fork/upstream merge base matched `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`. Working tree status was clean at the point of the final ref check.

---

## Checkpoint 1: reload marker/backoff/liveness

Judgment: **mostly sound, no stale-marker pilot blocker found.**

Evidence:

- Reload marker activity is age-bounded and limited to `Starting` or `SocketReady` phases, with old markers removed on read: `reload_state.rs:42-68`.
- Stale markers from another PID are cleared when the current server reaches the socket-ready publish point: `reload_state.rs:96-119`.
- Wait status is marker-driven while the marker is fresh, fails if the marked PID is dead, and otherwise falls back to socket readiness/liveness: `reload_state.rs:157-193`.
- New client turns are rejected while a recent `Starting` marker exists: `client_lifecycle.rs:243-248`, `client_lifecycle.rs:2723-2730`.
- Unit coverage exists for recent `Starting` versus `SocketReady` and for rejecting a new turn without spawning provider work: `client_lifecycle_tests.rs:676-697`, `client_lifecycle_tests.rs:699-780`.

Risk notes:

- The marker path is process/runtime-global, so correctness depends on `JCODE_RUNTIME_DIR` isolation in tests and on runtime-dir consistency in production.
- I did not prove all reload wait callers use `inspect_reload_wait_status`; I verified the core primitive only.

---

## Checkpoint 2: disconnect cleanup and terminal state correctness

Judgment: **functionally defensive, but terminal labels are semantically mixed.**

Evidence:

- Disconnect disposition is computed from whether the client was processing or has a live processing task: `client_disconnect_cleanup.rs:74-79`.
- Processing disconnect during a recent reload is classified as `Reloading`, otherwise as `Crashed`: `client_disconnect_cleanup.rs:26-35`.
- Cleanup unregisters debug/client/event sender state before destructive work and skips destructive cleanup if a live successor is already attached: `client_disconnect_cleanup.rs:81-103`.
- Agent session state is marked closed, crashed, or crashed-with-reload-reason under the agent lock: `client_disconnect_cleanup.rs:106-170`.
- Swarm status maps `Reloading` to `stopped` with detail `server reload in progress`, while the session itself is marked crashed with reason `Server reload interrupted processing`: `client_disconnect_cleanup.rs:181-190`.
- Cleanup finally removes shutdown signals, background-tool signals, soft interrupt queues, aborts the local processing task, and aborts the event task: `client_disconnect_cleanup.rs:240-251`.

Risk notes:

- The session-level `mark_crashed` and swarm-level `stopped` split is defensible but easy to misinterpret in dashboards and automation. If `stopped` is treated as successful or user-requested termination, reload-interrupted processing can be underreported.
- The cleanup path uses a 2 second agent-lock timeout and then skips graceful session marking if the lock is stuck: `client_disconnect_cleanup.rs:110-176`. This prevents a shutdown hang, but creates an evidence gap for exactly the stuck-turn class R04 cares about.

---

## Checkpoint 3: reload interruption ordering and graceful shutdown

Judgment: **stronger than baseline, with explicit bounded wait.**

Evidence:

- Reload writes `Starting`, acknowledges the signal, persists recovery intents, then asks active sessions to checkpoint before exec: `reload.rs:95-137`.
- Exec failure and missing-binary paths write `Failed` before process exit: `reload.rs:176-206`.
- Graceful shutdown selects only members with `status == "running"` and partitions signalable versus unsignalable sessions by the shutdown-signal registry: `reload.rs:359-373`.
- Unsignalable running sessions are logged and do not consume the reload grace period: `reload.rs:375-388`.
- Signals are fired for all signalable running sessions, including the triggerer: `reload.rs:403-427`.
- The triggering session is excluded from the checkpoint wait set but still receives the signal: `reload.rs:429-445`.
- The checkpoint wait is bounded and proceeds after timeout with explicit warning: `reload.rs:477-507`.
- Tests assert all running sessions including initiator are signaled, the triggering session does not block reload, idle sessions are skipped, and timeouts are honored: `reload_tests.rs:155-223`, `reload_tests.rs:225-325`, `reload_tests.rs:551-652`.

Risk notes:

- Excluding the triggering session from the wait set is the right anti-deadlock tradeoff for `selfdev reload`, but it increases reliance on the recovery-intent path and on terminal-state clarity for the initiating tool.

---

## Checkpoint 4: reload recovery handoff and durable delivery

Judgment: **good handoff design, one semantic dependency remains.**

Evidence:

- Recovery intents are persisted for running candidates, plus the triggerer if absent from the running set: `reload.rs:215-255`.
- Role assignment distinguishes headless, initiator, and interrupted peer: `reload.rs:286-302`.
- History/bootstrap attachment explicitly does not mark intent delivered because the client may disconnect before queueing the continuation: `reload_recovery.rs:234-241`.
- Delivery is marked only when the accepted continuation message matches the persisted directive exactly: `reload_recovery.rs:286-334`.
- Delivered records are removed, with failure to remove treated as cleanup debt rather than rejected delivery: `reload_recovery.rs:332-364`.
- Garbage collection removes delivered records and stale/corrupt pending records older than seven days: `reload_recovery.rs:112-184`.

Risk notes:

- The delivery key is the continuation message string. That is simple and robust against accidental delivery, but fragile if future formatting changes alter the hidden continuation text.
- Cheapest hardening is a fixture that persists an intent, simulates a history frame loss, then accepts the matching continuation and verifies exactly one delivery plus record removal.

---

## Checkpoint 5: cancel/shutdown/resume handoff, including no-local-task cancel

Judgment: **R04 regression is explicitly covered and the architecture is plausible.**

Evidence:

- Busy-session control handle refresh uses a lock-free cancel-only handle when it cannot acquire the agent lock, relying on the turn-cancel registry to reach the actual running turn: `client_lifecycle.rs:278-311`.
- Streaming turns register their active graceful-shutdown signal in the process-global turn-cancel registry at turn start: `turn_streaming_mpsc.rs:87-94`.
- Streaming watches graceful shutdown both before stream open and while waiting on stream events: `turn_streaming_mpsc.rs:234-252`, `turn_streaming_mpsc.rs:363-385`.
- The regression test describes the post-reload/no-local-task/stale-stop-signal path and asserts the detached streaming turn aborts within two seconds, unregisters the signal, and does not leak cancellation into the next turn: `client_lifecycle_tests.rs:382-495`.

Risk notes:

- This is exactly the class of bug R04 must prevent. The code and fixture are well targeted.
- I could not execute the test in this review because the build did not complete.

---

## Checkpoint 6: background tasks, detached adoption, and orphan reconciliation

Judgment: **substantial fix exists, but legacy/no-owner tasks remain a known edge.**

Evidence:

- Non-detached background tasks write owner PID and process-instance token in the initial status file: `background.rs:439-459`.
- Natural completion writes terminal status, exit code, error, duration, and terminal event history: `background.rs:514-541`.
- Detached tasks are explicitly marked `detached: true`, have a PID, and clear in-process owner metadata so orphan reconciliation does not clobber them: `background.rs:360-391`.
- Detached finalization reaps or checks the PID, reads output/exit marker, writes Completed only for exit code 0, otherwise Failed, then publishes a completion bus event: `background.rs:139-207`.
- Non-detached orphan detection is conservative and keys on running status, no PID, non-detached, owner PID, and mismatched/dead process instance: `background.rs:212-246`.
- Orphan finalization writes Failed with a reload/crash explanation and publishes a completion bus event: `background.rs:248-302`.
- Startup/status reconciliation is exposed by `reconcile_orphaned_tasks` and status reads self-heal orphaned files: `background.rs:307-341`, `background_tests.rs:445-471`.
- Tests cover same-PID new-instance reload orphan, dead-process orphan, non-orphans, and status-read self-healing: `background_tests.rs:344-443`, `background_tests.rs:445-471`.

Risk notes:

- Files without owner metadata are intentionally left alone by the conservative predicate: `background.rs:228-246`. That is safe for not killing live work, but it means pre-fix or malformed running status files can remain phantom-running. Treat as migration debt, not an immediate R04 blocker unless pilot includes long-lived users carrying old task files.

---

## Checkpoint 7: provider-session reset/resume consistency

Judgment: **no immediate lifecycle bug found in inspected paths.**

Evidence:

- Session restore loads persisted `session.provider_session_id` back into `agent.provider_session_id`: `turn_execution.rs:581-600`.
- Runtime state reset after session assignment clears transient turn/session controls but does not clear provider session IDs: `turn_execution.rs:609-612`, `agent.rs:610-628`.
- Explicit provider-session reset exists and clears both agent and persisted session fields: `turn_execution.rs:196-212`.
- Streaming passes the current `provider_session_id` as resume session ID to the provider and updates both agent and session copies when a provider session ID is returned: `turn_streaming_mpsc.rs:213-240`, `turn_streaming_mpsc.rs:790-791`.
- Compaction-related paths clear provider session IDs where history continuity is intentionally broken: `agent.rs:646-688`, `agent/compaction.rs:4-10`, `agent/compaction.rs:110-191`.

Risk notes:

- I did not prove provider-specific semantics for every provider. I only checked that the R04 lifecycle/resume paths preserve or clear IDs consistently at the agent/session boundary.

---

## Checkpoint 8: false terminal success and R09 validation debt

Judgment: **pilot blocker.**

Evidence:

- Reload-interrupted wait-like tools return a message that says the underlying operation may still be running and instructs the user to resume the wait: `turn_streaming_mpsc.rs:55-67`.
- For selfdev reload and wait-like tools, the code maps interruption to non-error evidence: `SessionLogStatus::Ok`, `ToolDone.error = None`, and `ToolResult.is_error = Some(false)`: `turn_streaming_mpsc.rs:1530-1577`.
- The test encodes that `bg wait` interruption is non-error and resumable: `turn_streaming_mpsc.rs:1680-1692`.
- Non-wait tools remain error on reload interruption: `turn_streaming_mpsc.rs:1694-1702`.

Risk judgment:

- I agree with the user-facing goal: a reload should not make an in-flight `bg wait` look like the background task failed.
- I disagree with using generic `Ok` as the durable terminal state unless every consumer distinguishes "wait handoff preserved" from "operation completed". The message says the underlying operation may still be running, so a generic success state is too weak for evidence, automation, and future recovery logic.
- Cheapest fix is not necessarily a code change in R04. The minimum pilot gate is a fixture that asserts downstream rendering/evidence labels show "resumable interrupted wait" rather than "completed successfully".

---

## Narrow commands and results

| Command | Result |
|---|---|
| `git rev-parse HEAD; git branch --show-current; git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b; git status --short` | Exit 0. HEAD `5baf343ba6da564afc3f6c58c5edca7a64d6e67f`, branch `recovery/seam-r04-20260715`, merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`, clean status. |
| First targeted Cargo attempt, task `277484mz28` | Exit 1 because multiple test filters were passed in one Cargo invocation. Command-construction failure, not code-test failure. |
| Rerun as separate one-filter invocations, task `301763r7rv` | Exit 124 after 600s. Timed out while compiling, with no test result. Output included dev-shell reentry and compilation progress. |
| `./scripts/dev_cargo.sh test -p jcode-base background::tests::reconcile_marks_orphan_from_reloaded_process_failed -- --exact --nocapture`, task `98125782j1` | Exit 124 after 600s. Timed out while waiting/building, including `Blocking waiting for file lock on build directory`. No test result. |

No live-daemon, network, or destructive commands were run.

## Negative findings

I did **not** find evidence, in the inspected code, that:

- A stale reload marker can indefinitely reject turns once it ages out or moves to a non-starting phase.
- Disconnect cleanup destroys a session while another live successor connection is already attached.
- The no-local-task cancel path necessarily loses the active turn after reload/reattach.
- Detached task reconciliation reports success without an exit code of 0.
- Non-detached orphaned background tasks with owner metadata remain forever-running after reload or owner death.
- Provider-session IDs are blindly cleared during normal session restore.

## Pilot blockers

1. **R09 execution blocker:** obtain a successful narrow deterministic test run in this worktree. At minimum, prebuild once or clear the build lock, then run:
   - `jcode-base background::tests::reconcile_marks_orphan_from_reloaded_process_failed`
   - `jcode-app-core server::client_lifecycle_tests::cancel_aborts_detached_streaming_turn_with_stale_stop_signal`
   - `jcode-app-core server::reload_tests::graceful_shutdown_sessions_signals_all_running_sessions_including_initiator`
   - `jcode-app-core server::reload_tests::graceful_shutdown_sessions_times_out_on_partial_checkpoint`
2. **Terminal semantics blocker:** prove or adjust downstream handling of reload-interrupted wait-like tool `Ok` evidence so it cannot be interpreted as completed background work.

## Cheapest fixtures to add or require before pilot

1. A pure unit for `reload_interrupted_tool_result` plus evidence rendering that asserts label text is `resumable interrupted wait`, not plain success.
2. A reload recovery fixture that persists an intent, attaches it to history without delivery, then accepts matching continuation and verifies record removal.
3. A background orphan fixture for same-PID different-instance status files, already present, must be part of the R04 required gate.
4. A no-local-task cancel fixture, already present, must be part of the R04 required gate.
5. A legacy running status file fixture that documents the accepted behavior for no owner metadata, either left running with a warning or migrated after a bounded age.

## Confidence and gaps

Confidence: **medium-high on code review, low on executed validation**.

Gaps:

- Cargo tests did not complete because of compile/build-lock timeouts.
- I did not inspect every provider implementation for provider-session semantics.
- I did not run live daemon reload or UI flows, per scope.
- I did not read `/tmp/jcode-r04-opus-review.md`.
