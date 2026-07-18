# F02 independent implementation review

## Verdict

**FAIL.**

Reviewed implementation commits:

- `9d9762e6fcfaf43c508f8a79959fbbccedb03c51` — `F02: add jcode-core activity-lease seam (F01 design 3.0)`
- `e5059a3c24bd918232b588ed3055a88804230ad9` — `F02: shutdown coordinator, lease authority, and background finalize`
- `2609e7a8d81902d7f171752c51edd3aa22d6cbfb` — `F02: wire all exit paths and lease sites through the coordinator`

Review baseline: `ef67216ada9c4549d4c1b084332a4caf19acdbb1` (`docs(ideal-base): F02 evidence and implemented checkpoint`).

Reviewer route: **OpenAI `gpt-5.6-sol`, high effort**.

The watchdog terminal-outcome CAS is sound, the normal exit-site consolidation is substantially present, and the focused tests pass. F02 nevertheless does not meet the accepted F01 invariants or both acceptance gates. Idle shutdown is not claimed atomically against client/lease acquisition, the required `ScheduledDelivery` lease is not wired, reload drain does not stop intake, and several production work classes silently continue after a `ShuttingDown` lease refusal.

## Validation performed

### Source and history review

- Read `git show` for all three implementation commits named above.
- Read `docs/fork/ideal-base/evidence/F02/README.md` and the accepted F01 revision-4 design at `docs/fork/ideal-base/evidence/F01/design.md`.
- Read both F02 copies in `docs/fork/ideal-base/WORK_GRAPH.json`; both have the same two acceptance gates and owned paths.
- Read the requested key files at `ef67216ad`, including:
  - `crates/jcode-core/src/activity.rs`
  - `crates/jcode-app-core/src/server/shutdown.rs`
  - `crates/jcode-app-core/src/server/lifecycle.rs`
  - `crates/jcode-app-core/src/server.rs`
  - `crates/jcode-app-core/src/server/reload.rs`
  - `crates/jcode-app-core/src/server/client_lifecycle.rs`
  - `crates/jcode-base/src/mcp/manager.rs`
  - `crates/jcode-base/src/mcp/pool.rs`
  - `crates/jcode-base/src/background.rs`
  - `src/cli/dispatch.rs`
- Independently enumerated production `ActivityClass`/`acquire_lease` sites, `process_message_streaming_mpsc` callers, `Server::run`/coordinator callers, and daemon-image `process::exit` sites.
- Additionally read the scheduled/ambient dequeue and dispatch implementation in `crates/jcode-app-core/src/ambient/runner.rs` and `ambient/manager.rs`, because the accepted 3.3.3 census requires C9 coverage there.

### Focused tests run

All three requested commands passed:

1. `scripts/dev_cargo.sh test -p jcode-core --lib`
   - **34 passed; 0 failed**.
2. `scripts/dev_cargo.sh test -p jcode-base --lib background`
   - **26 passed; 0 failed; 1146 filtered out**.
3. `scripts/dev_cargo.sh test -p jcode-app-core --lib server::shutdown`
   - **10 passed; 0 failed; 1126 filtered out**.

The helper re-entered the repository Nix development shell because `cargo` was not on the ambient `PATH`.

### Runtime fixture

The binary `target/selfdev/jcode` was executable and timestamped after the final implementation commit. I ran:

```text
docs/fork/ideal-base/evidence/F02/exit_mode_fixtures.sh target/selfdev/jcode
```

The first run occurred while the three Nix/Cargo test jobs were contending for shared locks/resources. Its temporary-idle case did not finish inside the script's 60-second wall-clock limit; the SIGTERM case passed. A preserved-log reproduction immediately afterward exited temporary-idle with code 44 in about 22 seconds and left only its log. I then reran the unmodified official fixture after build contention ended:

```text
PASS: temporary-idle exit code 44
PASS: temporary-idle zero socket/hash/metadata residue
PASS: SIGTERM exit code 0
PASS: SIGTERM zero socket/hash/metadata residue
ALL FIXTURES PASSED
```

The passing rerun validates the two happy-path process modes exercised by the script. It does not exercise the blocking races below.

## Findings

### Blocking

#### F02-B1: idle shutdown is not atomically claimed against new clients or leases

The lifecycle monitors take a non-atomic snapshot and later call `begin`:

- `server/lifecycle.rs:168-187` reads `client_count`, then separately reads `drain_blocking_count`, advances `IdleClock`, and calls `begin(PersistentIdle)`.
- `server/lifecycle.rs:226-245` does the same for temporary idle.
- `server/shutdown.rs:595-611` changes the coordinator to `Draining` and only afterward calls `lease_authority().refuse_new()`.
- `server/shutdown.rs:292-299` gives idle reasons a zero drain budget; `execute` can therefore abandon a lease that arrived in the decision-to-begin gap (`:691-704`).

A client can connect, or any covered work class can acquire a lease, after the monitor's reads but before `refuse_new`. The idle reason is still accepted even though the system is no longer quiescent. Because idle drain has zero grace, `Cleaned(PersistentIdle|TemporaryIdle)` can then be reached without the accepted F01 I1 condition that quiescence held continuously through the transition.

The declared `client_count` deviation makes the client half of this race unavoidable: `client_count` and the lease table are different locks and cannot form the design's single atomic source of truth. Even after C1 unification, the idle transition still needs a lease-table operation that atomically verifies quiescence and closes acquisition, or an equivalent retry/claim protocol.

This directly fails gate 1: **"Idle exit requires zero clients and zero active leases."**

#### F02-B2: the required C9 `ScheduledDelivery` dispatch-gap lease is absent

The accepted design requires the ambient runner to acquire `ScheduledDelivery` when it dequeues a due task, holding it until the delivered turn acquires `ProviderTurn` or dispatch fails (`F01 design` sections 3.3.2 and 3.3.3).

At HEAD:

- `activity.rs` defines `ActivityClass::ScheduledDelivery`.
- No production Rust source acquires that class.
- `ambient/runner.rs:627-650` removes ready direct items from durable queue state with `take_ready_direct_items()`.
- `ambient/runner.rs:659-661` dispatches them without a lease.
- The fallback paths can perform full provider work directly through `Agent::run_once_capture` (`ambient/runner.rs:380-400` and `:403-474`), bypassing the server's common `ProviderTurn` boundary entirely.

A due item can therefore be durably dequeued while there are zero clients and zero counted leases, after which an idle monitor may terminate the daemon during delivery or direct headless provider execution. This is the exact dispatch gap C9 was introduced to close.

This independently fails gate 1 and contradicts the evidence README's claim that the complete design 3.3.3 lease census was wired.

#### F02-B3: reload drain does not stop intake

`begin_reload_drain` transitions to `Draining` and calls only `lease_authority().refuse_new()` (`server/shutdown.rs:750-771`). It never calls `cancel_intake()`. By contrast, ordinary `begin` explicitly does both at `server/shutdown.rs:607-611`.

The accepted reload protocol requires intake cancellation at `Draining` start. During the entire reload drain and handoff preparation, the main/debug accept loops can continue accepting connections. Their subsequent provider work may be refused, but the sockets and connection tasks remain live until exec or an upgraded termination completes. This is not the accepted bounded reload entry and makes the evidence statement that reload "stops intake" false.

This fails gate 2 for reload: **"SIGTERM, reload, persistent idle, and temporary-owner exits invoke bounded shutdown."**

#### F02-B4: multiple production lease refusals are silently discarded, so work starts after drain without a lease

F01 I6 requires `acquire` to fail after drain and the caller not to start new work silently. Several implemented sites convert that refusal into unleased execution:

- `server/debug_jobs.rs:86-110` and `:121-140`: the async debug job is created/spawned first; the spawned body ignores the `Result` from `acquire_lease` and runs the provider job anyway.
- `server/comm_await.rs:364-372`: the live watcher ignores refusal and continues for its full parked lifetime.
- `server.rs:752-760`: startup recovery ignores refusal and continues its enumeration/scheduling window.
- `jcode-base/src/background.rs:74-94`: `ActivityLeaseError` is explicitly erased with `.ok()`; the comment says the task still runs after refusal.
- `jcode-base/src/background.rs:517-535` and `:685-703`: `spawn_with_notify` proceeds and spawns/tracks the task with `activity_lease: None`.
- `jcode-base/src/background.rs:889-907`: adoption likewise continues with no guard.

These are not harmless cleanup-finalization cases. A provider turn that was already in flight can spawn a background task after drain begins, return and release its `ProviderTurn` lease, while the newly spawned background future continues unleased. Similarly, an already accepted debug command can race drain and start provider work after refusal. The executor can observe zero drain-blocking leases and publish `Cleaned` while such work still runs or is being finalized concurrently.

`process_message_streaming_mpsc` itself correctly maps `ShuttingDown` to an `anyhow` error before taking the agent lock (`client_lifecycle.rs:3183-3201`), and representative callers propagate or translate that error sanely. The blocking problem is the other work classes that discard the typed refusal.

This violates accepted I6 and the coherent bounded-shutdown contract underlying gate 2.

#### F02-B5: watchdog thread creation failure silently removes the hard shutdown bound

`Watchdog::arm` changes the atomic state from `IDLE` to `ARMED`, then calls `std::thread::Builder::spawn(...).ok()` and ignores failure (`server/shutdown.rs:417-432`). If OS thread creation fails, the watchdog remains logically armed but no thread exists to claim `FIRING` and force exit. Cleanup completion can still cancel the nonexistent watchdog, masking the failure; if the Tokio runtime or a synchronous cleanup operation stalls, no terminal outcome is guaranteed.

The accepted F01 I5 bound relies on the coordinator-owned OS-thread watchdog. Thread creation failure must either be handled synchronously/fail closed or leave a functioning alternative. As written, gate 2's boundedness is not unconditional.

### Important but nonblocking

#### F02-I1: the spawn-on-first-`begin` deviation is safe only under the current caller census

`begin` calls `tokio::spawn(self.execute())` after releasing the coordinator lock (`server/shutdown.rs:602-611`). Lock reentrancy is avoided and the current production callers are Tokio tasks: lifecycle monitors, SIGTERM task, and async accept-loop paths. The watchdog thread does not call `begin`; it only reads the current reason before forced exit. I found no current production missed-executor path from the watchdog.

However, `begin` is a synchronous method whose type does not encode the Tokio-runtime precondition. A future call from a plain OS thread would panic after phase mutation, potentially leaving `Draining` with no executor. The deviation is acceptable for the current caller set only if that caller census is guarded by a test or `begin` uses a runtime handle/standing actor that makes the precondition explicit.

#### F02-I2: startup recovery is called "bounded" but has no implemented hard TTL

The accepted design gives `StartupRecovery` a default hard TTL. The implementation acquires an optional guard at `server.rs:752-760` and holds it until the whole recovery function returns, with no timeout wrapper. Ignoring refusal is already blocking under F02-B4. Separately, an ordinary recovery hang can pin idle shutdown indefinitely. This is the safe failure direction for gate 1, but it is not the declared design behavior and needs F03/follow-up coverage.

#### F02-I3: focused coordinator tests do not exercise the real coordinator state machine

The ten `server::shutdown` tests cover helper models, budgets, and the isolated watchdog atomic, but they do not drive `ShutdownCoordinator::begin`, `begin_reload_drain`, `reload_exec_failed`, executor spawning, terminal publication, intake cancellation, or the idle acquisition race. The accepted design requested exhaustive small-state/random transition testing and pairwise reason races. Those remain essential F03 work, but the blockers above must be fixed before such tests can pass honestly.

### Minor

#### F02-M1: the forced-exit marker remains as `"armed"` after a clean shutdown

The watchdog writes an `armed` marker at `server/shutdown.rs:427` and cancellation does not remove or rewrite it. This is not a `Cleaned`/forced-exit race and does not affect the two gates, but a durable post-mortem surface can misleadingly retain an old armed state after clean completion. Record cancellation/clean completion or make next-boot interpretation explicitly process/PID scoped.

## Gate checklist

| Acceptance gate | Result | Evidence |
|---|---|---|
| Idle exit requires zero clients and zero active leases | **FAIL** | F02-B1 permits client/lease acquisition between the monitor snapshot and drain refusal; idle has zero drain grace. F02-B2 leaves scheduled delivery and direct fallback provider execution entirely outside the lease table. |
| SIGTERM, reload, persistent idle, and temporary-owner exits invoke bounded shutdown | **FAIL** | SIGTERM and temporary-owner paths do call the coordinator, and the happy-path fixtures pass. Reload does not cancel intake (F02-B3), post-drain work can continue after ignored refusals (F02-B4), and watchdog thread-spawn failure removes the hard terminal bound (F02-B5). |

## Deviation disposition

### 1. First accepted `begin` spawns the serialized executor instead of using a standing actor

**Conditionally acceptable, important follow-up.** Only the first `Running -> Draining` transition spawns the ordinary executor, the state lock is released before `tokio::spawn`, and reload's inline drain hands upgraded termination to one executor. I found no current double-executor path in the reviewed source. The unencoded runtime-context precondition remains F02-I1.

### 2. `LeaseTable` uses `Instant` directly instead of an injected `Tick`

**Acceptable for the F02 runtime implementation, but it weakens the specified validation model.** Pure helper tests can inject `Instant` values, and this does not itself violate either gate. It does make exhaustive deterministic state-machine testing less direct; the missing transition/race tests are noted in F02-I3.

### 3. `client_count` remains separate instead of becoming `ClientConnection` leases

**Not acceptable for F02's gate.** Separate locks do not preserve A1/I1 atomically. The monitor can observe zero clients and then accept idle shutdown after a client connects. This deviation is part of blocking F02-B1 and cannot be deferred as a mere C1-unification cleanup while gate 1 is claimed complete.

## What I did not check

- I did not run the entire workspace test suite or the full `jcode-app-core --lib` suite; I ran exactly the three requested focused commands.
- I did not run Miri, Loom, sanitizers, model checking, or fault-inject thread creation/runtime stalls.
- I did not execute reload success, reload exec failure, temporary reload refusal, accept-loop failure, temporary-owner exit, forced watchdog exit, parent-SIGKILL, pairwise reason races, or one fixture per lease class. Those are primarily F03 fixtures, but several would currently expose the blocking findings above.
- I did not validate Windows-specific process behavior or non-Unix signal equivalents.
- I did not inspect every background-task tool implementation above `BackgroundTaskManager`; the common manager behavior is sufficient to establish the refusal defect.
- I did not modify implementation code.

## Confidence

**High (98%).** The verdict rests on direct source paths and accepted-design invariants, not only on absent tests. F02-B1 and F02-B2 each independently defeat the idle gate. F02-B3 and F02-B4 are direct contradictions of the accepted drain protocol and typed-refusal contract. The requested focused tests and the uncontended runtime fixture rerun all passed, so the FAIL is specifically about uncovered implementation semantics rather than a general build/test failure.

# Round 2: blocker-fix re-review

## Verdict

**FAIL.**

Re-reviewed exact fix commit `8a09a289d2ac5deb6054aa301066feb69eaa651a` (`F02: fix all round-1 review blockers (B1-B5) and important findings`) at HEAD `8a09a289d2ac5deb6054aa301066feb69eaa651a`.

Reviewer route: **OpenAI `gpt-5.6-sol`, high effort**.

The fix closes the original scheduled-delivery gap, reload intake omission, ordinary lease-refusal paths, production-context watchdog fallback, startup-recovery TTL, and stale watchdog marker. Two blocking defects remain. The connection mirror does not fail closed when its `ClientConnection` lease loses to an idle claim, so a counted main/gateway client can still be admitted after the atomic empty-table claim. The adopted-background rationale is also incorrect: cleanup aborts only the wrapper awaiting the original `JoinHandle`; dropping that handle detaches rather than aborts the already-running original task.

## Validation performed

### Exact source review

- Confirmed HEAD is exactly `8a09a289d2ac5deb6054aa301066feb69eaa651a`, with a clean baseline before this review append.
- Read `git show 8a09a289d` completely for all ten changed paths.
- Read the new `Review round 1 blocker fixes` section in `docs/fork/ideal-base/evidence/F02/README.md`.
- Re-read the affected production files at HEAD, including `server/shutdown.rs`, `server/lifecycle.rs`, `server/runtime.rs`, `ambient/runner.rs`, `server.rs`, `server/comm_await.rs`, `server/debug_jobs.rs`, and `jcode-base/src/background.rs`.
- Enumerated every production `increment_client_count`/`decrement_client_count` call. The main socket and gateway paths both use the mirrored methods; debug clients remain deliberately excluded. No direct counter mutation bypasses those methods.
- Enumerated every production `take_ready_direct_items`/`pop_ready` call. The ambient runner is the sole production direct-item dequeue; tests are the only other callers of the persistence primitives.
- Rechecked watchdog arm callers, executor spawn contexts, reload ordering, refusal propagation, StartupRecovery guard lifetime, and the previously clean ProviderTurn/MCP/exit-site areas for regression.

### Tests run

1. `scripts/dev_cargo.sh test -p jcode-app-core --lib server::shutdown`
   - **11 passed; 0 failed; 1126 filtered out**.
   - Includes the new `idle_claim_is_atomic_against_any_lease` test.
2. `scripts/dev_cargo.sh test -p jcode-base --lib background`
   - **26 passed; 0 failed; 1146 filtered out**.

Both commands re-entered the repository Nix development shell because `cargo` was not on the ambient `PATH`.

I did not rerun the runtime fixture. `target/selfdev/jcode` was built at 07:45:35, before the fix commit timestamp 07:46:27; it may have been built from the pre-commit worktree containing the fixes, but the binary cannot be tied unambiguously to the exact reviewed commit. The updated evidence records a passing rebuilt run. The remaining blockers are concurrency/ownership paths not exercised by the two happy-path fixtures.

## Findings

### Blocking

#### F02-R2-B1: a main/gateway connection that loses lease acquisition to the idle claim is still counted and admitted

`LeaseTable::try_claim_idle_shutdown` itself is correct: it tests complete table emptiness and sets `refusing = true` under one table mutex (`server/shutdown.rs:113-124`). The idle `begin` path uses that claim and returns `Refused(NotQuiescent)` on an existing lease (`server/shutdown.rs:670-705`); both monitors restart their epoch on that refusal (`server/lifecycle.rs:184-202`, `:258-276`).

The remaining race is at connection admission:

- Main accept calls `increment_client_count` immediately after `listener.accept` (`server/runtime.rs:163-176`).
- Gateway accept does the same after receiving a `GatewayClient` (`server/runtime.rs:221-245`).
- `increment_client_count` attempts a `ClientConnection` lease but discards `ShuttingDown`; it increments `client_count` regardless (`server/runtime.rs:319-340`).
- It then attempts to spawn the client task. `RuntimeTaskScope::spawn` rejects only after observing cancellation (`server/runtime.rs:38-56`). Idle `begin` cancels intake only after the lease claim, coordinator phase mutation, watchdog arm, and logging (`server/shutdown.rs:675-705`).

On a multi-threaded runtime, the idle claim can linearize after the OS/gateway accept completes but before `increment_client_count` acquires its lease. The lease is refused, yet the accepted connection is counted and can win the task-registration race before `cancel_intake`. It therefore has no `ClientConnection` guard and is invisible to the supposedly authoritative empty lease table. Even if cancellation tears it down shortly afterward, `Cleaned(idle)` is no longer proven reachable only from zero clients, and new intake did not fail closed after the claim.

The mirror census is otherwise complete, and the pop-any-guard discipline is sound for successfully leased connections because guards are interchangeable for count/quiescence purposes. It becomes unsound only because a refused increment contributes a counter entry without contributing a guard; its later `pop()` can release a different live connection's guard.

Fix by making connection acquisition return an admission result: if the `ClientConnection` lease is refused, close/drop that accepted stream or gateway client without incrementing or spawning. Store one guard in the per-connection task/teardown object, or otherwise preserve a strict successful-acquire-to-decrement pairing.

This keeps gate 1 failed: **idle exit is not yet guaranteed to require zero clients and zero active leases.**

#### F02-R2-B2: adopted background cleanup aborts only the wrapper, not the already-running original future

The evidence says an adopted task that is refused a lease remains tracked so `finalize_non_detached` will abort it. The tracking half exists, but the abort claim is false:

- `adopt_with_options` receives the already-running original `JoinHandle` (`jcode-base/src/background.rs:795-803`).
- It creates `wrapper_handle = tokio::spawn(async move { let tool_result = handle.await; ... })` (`background.rs:852-875`).
- `RunningTask.handle` stores only `wrapper_handle` (`background.rs:954-980`).
- `finalize_non_detached` calls `task.handle.abort()` (`background.rs:434-445`).

Aborting the wrapper drops the captured original `JoinHandle`. Tokio `JoinHandle` drop detaches its task; it does not abort the original future. The original adopted work therefore continues unleased after cleanup has marked it failed-by-shutdown, can race later cleanup steps and sidecar removal, and can still perform side effects after `Cleaned` is published until process termination.

This is particularly material on the exact refusal path: the foreground owner may still be draining or may have been abandoned at its deadline, while adoption inserts an unleased wrapper with no abort handle for the underlying task. Tracking the wrapper does not make the original task abortable.

Store and abort the original task's `AbortHandle` (or retain both original and wrapper abort authorities) in `RunningTask`. Add a test whose adopted original future owns a drop flag, call `finalize_non_detached`, and prove the original future itself is cancelled rather than detached.

This leaves F02-B4 only partially closed and remains a blocking cleanup-honesty defect for gate 2.

### Important but nonblocking

#### F02-R2-I1: reload still flips to `Draining` before closing lease intake

Ordinary `begin` now calls `refuse_new()` before `state.phase = Draining`, closing the original ordering gap (`server/shutdown.rs:675-690`). `begin_reload_drain` still sets `Phase::Draining`, drops the coordinator lock, logs, and only then calls `refuse_new()` and `cancel_intake()` (`server/shutdown.rs:844-869`). A tracked lease can therefore be acquired during a phase that is already reported as `Draining`, contrary to I6.

The lease is visible to the subsequent reload drain loop and bounded by its deadline, so this does not independently defeat the two acceptance gates. The ordering should nevertheless match ordinary `begin`: close acquisition before publishing the phase transition.

#### F02-R2-I2: debug-job refusal leaves a permanently queued job record

Both debug command branches call `create_job` before acquiring `DebugJob` (`server/debug_jobs.rs:86-100`, `:125-136`). On `ShuttingDown`, the request returns an error and no task spawns, which correctly fails closed for work execution. The already-created map entry remains `Queued` with no completion path. Acquire before `create_job`, or mark/remove the record on refusal.

This is state hygiene, not a shutdown-gate blocker.

#### F02-R2-I3: transition/race coverage remains F03 work

The new pure test validates the table operation, but does not drive the accept-versus-idle-claim race, gateway admission, adopted-original cancellation, reload phase/refusal ordering, or the real coordinator state machine. This is the prior F02-I3 and remains appropriately separated as F03/follow-up coverage. The two concrete blockers above are source defects, not merely missing tests.

## Disposition of B1-B5/I1-I2/M1

| Round-1 item | Round-2 disposition | Evidence |
|---|---|---|
| F02-B1 atomic idle claim/client unification | **PARTIAL, still blocking** | Table claim and monitor retry are correct. Main and gateway mirrors are present, and pop-any is valid among successfully leased connections. Refused connection acquisition still increments/adopts a client, leaving an unguarded admission race (F02-R2-B1). |
| F02-B2 `ScheduledDelivery` | **CLOSED** | Lease is acquired before the sole production direct-item dequeue, dequeue is skipped on refusal, and the guard is held through direct/fallback delivery (`ambient/runner.rs:626-681`). No other production direct-item dequeue bypass was found. |
| F02-B3 reload intake cancellation | **CLOSED for the blocker** | `begin_reload_drain` now calls `cancel_intake()` (`shutdown.rs:865-869`). Phase/refusal ordering remains important F02-R2-I1, but tracked work is still bounded by the reload drain. |
| F02-B4 fail-closed refusal sites | **PARTIAL, still blocking** | Debug, waiter, startup recovery, and `spawn_with_notify` no longer run new unleased work. Adoption tracking does not abort the underlying original future; it aborts only its wrapper (F02-R2-B2). |
| F02-B5 watchdog fallback | **CLOSED for current production callers** | All production arm sites run inside Tokio; OS-thread spawn failure falls back to `spawn_blocking`. A simultaneous off-runtime + OS-thread failure remains logged as unbounded, but no production arm caller uses that context. |
| F02-I1 off-runtime executor spawn | **CLOSED for current production callers; residual double-failure edge** | `spawn_on_runtime` uses the current handle or a dedicated one-shot-runtime thread. Dedicated-thread spawn/runtime-build errors are not recovered, but current production begin callers are on Tokio. |
| F02-I2 StartupRecovery TTL | **CLOSED** | The guard moves to a TTL task and is dropped after 60 seconds or promptly when the recovery function returns (`server.rs:752-791`). Actual restored turns retain their own ProviderTurn leases. |
| F02-M1 stale armed marker | **CLOSED** | Successful watchdog cancellation records `cancelled` before `Cleaned` publication (`shutdown.rs:480-495`). The Armed/Cancelled/Firing mutual exclusion remains intact. |

## Gate checklist

| Acceptance gate | Result | Evidence |
|---|---|---|
| Idle exit requires zero clients and zero active leases | **FAIL** | The atomic table claim is correct, but a main/gateway accept that loses `ClientConnection` acquisition to the claim is still counted and may be registered without a guard (F02-R2-B1). |
| SIGTERM, reload, persistent idle, and temporary-owner exits invoke bounded shutdown | **FAIL** | Exit routing, reload intake cancellation, watchdog CAS, and production watchdog fallback are present. Adopted original work is not actually aborted by cleanup and can continue after failed-by-shutdown finalization/Cleaned (F02-R2-B2), so bounded cleanup is not honest for that work class. |

## What I did not check

- I did not run the full `jcode-app-core --lib` suite, workspace tests, Miri, Loom, sanitizers, or model checking.
- I did not rerun the runtime fixture because the available selfdev binary predates the reviewed commit timestamp and cannot be tied unambiguously to exact commit `8a09a289d`.
- I did not execute forced-watchdog, reload success/failure/refusal, temporary-owner, accept-loop failure, parent-SIGKILL, or pairwise reason-race fixtures. These remain F03 scope.
- I did not write a temporary executable test for Tokio's documented JoinHandle-drop detachment semantics; the finding follows directly from the stored-handle ownership in the reviewed source.
- I did not validate Windows-specific listener and signal behavior.
- I did not modify implementation code.

## Confidence

**High (99%).** The two blockers are direct ownership/admission defects. The first follows from a refused lease being ignored while the counter and client task proceed. The second follows from `RunningTask` storing only a wrapper handle while the original handle is awaited inside that wrapper; aborting the wrapper necessarily loses cancellation authority over the original task. The focused tests pass, but neither defect is represented by those tests.

# Round 3: final re-review

## Verdict

**PASS.**

Re-reviewed exact fix commit `2b560788231e10741267d3dbfe74dc48368225a8` (`F02: fix round-2 review blockers R2-B1 and R2-B2`) at HEAD `2b560788231e10741267d3dbfe74dc48368225a8`.

Reviewer route: **OpenAI `gpt-5.6-sol`, high effort**.

Both round-2 blockers are closed. Counted main and gateway connections can now exist only after successful `ClientConnection` acquisition, and every successful admission has exactly one decrement/release path whether task registration succeeds or fails. Adopted background work retains abort authority over the original future, and both shutdown finalization and user cancellation abort the original before the wrapper. The reload ordering and orphaned debug-record findings are also closed. I found no new blocking implementation defect or regression in the previously clean F02 areas.

The runtime artifact supplied in `target/selfdev/jcode` is not, despite the stated provenance, an exact-commit build: it embeds `8a09a289d` and version metadata `84541c5a1, dirty`, and its filesystem timestamp precedes commit `2b5607882`. My optional fixture rerun against that artifact consequently failed temporary-idle with exit 45. This is an evidence/artifact provenance defect, not a source defect at `2b5607882`, and the fixtures do not cover either round-2 blocker. It does not reverse the implementation PASS, but the exact-build fixture claim should not be relied on until the binary and transcript are regenerated and tied to the reviewed SHA.

## Validation performed

### Exact source and history review

- Confirmed `HEAD == 2b560788231e10741267d3dbfe74dc48368225a8` and the worktree was clean before this review append.
- Read `git show 2b5607882` completely, including all eight changed paths.
- Read the changed files at HEAD in context: `server/runtime.rs`, `server/shutdown.rs`, `server/debug_jobs.rs`, `background.rs`, `background/model.rs`, `background/tests.rs`, and the updated F02 evidence files.
- Confirmed the only source changes after round 2 are the intended fix surface. There is no later source drift.
- Re-enumerated all `try_admit_client`, `decrement_client_count`, direct `client_count` mutation, `original_abort`, `begin_reload_drain`, and debug `create_job` sites.
- Rechecked exit routing, intake cancellation, atomic idle claiming, scheduled-delivery coverage, refusal handling, executor spawning, StartupRecovery TTL, watchdog fallback/marker behavior, and the prior ProviderTurn/MCP/temporary-owner/accept-failure areas for regression.

### Focused tests

1. `scripts/dev_cargo.sh test -p jcode-base --lib background`
   - **27 passed; 0 failed; 1146 filtered out**.
   - Includes `finalize_non_detached_aborts_adopted_original_future`.
2. `scripts/dev_cargo.sh test -p jcode-app-core --lib server::shutdown`
   - **11 passed; 0 failed; 1126 filtered out**.
   - Includes `idle_claim_is_atomic_against_any_lease`.

Both commands re-entered the repository Nix development shell because `cargo` was not on the ambient `PATH`.

### Runtime fixture/provenance probe

I optionally ran:

`bash docs/fork/ideal-base/evidence/F02/exit_mode_fixtures.sh target/selfdev/jcode`

Observed result:

- temporary-idle: **exit 45, expected 44**; zero residue;
- SIGTERM: **exit 0**; zero residue;
- overall: **1 fixture failure**.

The failure is explained by artifact provenance, not by the reviewed source:

- commit timestamp: `2b5607882` at `2026-07-18T08:01:43-04:00`;
- binary mtime: `2026-07-18T08:00:22-04:00`;
- binary strings include source SHA `8a09a289d` and build version `84541c5a1, dirty`;
- binary SHA-256: `db2af8c6f3873558918a82d36ca69ff7dbea6542b991c10e2df3dca7abc950bf`.

The committed `SHA256SUMS` references `exit_mode_fixtures_run.log`, but that transcript is not present in commit `2b5607882`; it exists only as an ignored working-tree file. Its contents say both fixtures passed. Because neither the available binary nor transcript is tied to exact commit `2b5607882`, I did not count the transcript as exact-commit validation.

## Findings

### Blocking

None.

### Important but nonblocking

#### F02-R3-I1: exact-build runtime evidence provenance is false for the available artifact

As detailed above, `target/selfdev/jcode` demonstrably predates and embeds a different source revision, and the referenced fixture transcript is absent from the reviewed commit. The optional rerun failed its temporary-idle expectation because that older binary followed the accept-loop-failure path.

This needs evidence repair: rebuild after checking out exact reviewed SHA, capture the embedded SHA/version and binary digest, rerun the fixture, and commit or otherwise durably bind the transcript and binary digest to the evidence record. It is nonblocking for F02 because the implementation gates are established by direct source review and focused tests, the runtime slice is not intended to exercise R2-B1 or R2-B2, and the broader transition/fixture matrix is explicitly F03 scope.

### F03-only coverage gap

The prior F02-I3 remains deferred as designed. There is still no deterministic runtime race test for accept-versus-idle-claim, no reload phase/acquisition interleaving test, and no full coordinator transition/race matrix. These are valuable F03 tests, not defects in the reviewed implementation.

## Disposition

| Item | Round-3 disposition | Evidence |
|---|---|---|
| F02-R2-B1 refused connection admission | **CLOSED** | Main and gateway are the only counted admission paths and both call `try_admit_client` before spawning (`server/runtime.rs:170-180`, `:241-252`). `try_admit_client` returns false without count mutation on `ShuttingDown`; only successful acquisition pushes a guard and increments (`:334-358`). |
| R2-B1 remaining mutation probe | **CLOSED** | The only production writes are increment at `runtime.rs:351` and decrement at `:366`, both encapsulated by the admission/teardown methods. `increment_client_count` is gone. Debug connections remain deliberately uncounted. |
| R2-B1 spawn-rejection pairing | **CLOSED** | A successful admission followed by task-registration refusal immediately calls one decrement in both main and gateway paths (`:176-181`, `:251-253`). A registered stream calls one decrement after `handle_client` completes or cancellation wins (`:375-421`). No counted path lacks a release, and no unadmitted path decrements. |
| F02-R2-B2 adopted original cancellation | **CLOSED** | Adoption captures `handle.abort_handle()` before moving the original handle into the wrapper and stores it in `RunningTask.original_abort` (`background.rs:856-860`, `:962-972`; `background/model.rs:240`). |
| R2-B2 finalization/cancel ordering | **CLOSED** | `finalize_non_detached` and `cancel_with_grace` both call `original.abort()` before `task.handle.abort()` (`background.rs:443-447`, `:1317-1322`). The new test waits past the original future's completion window and proves its survival flag remains false. |
| F02-R2-I1 reload phase/refusal ordering | **CLOSED** | Under the coordinator state lock, `begin_reload_drain` calls `refuse_new()` before assigning `Phase::Draining` (`shutdown.rs:844-867`), then cancels intake (`:868-872`). |
| F02-R2-I2 orphaned queued debug record | **CLOSED** | Both async debug commands acquire `DebugJob` before `create_job` (`debug_jobs.rs:86-102`, `:126-138`). Refusal therefore leaves no queued record and starts no task. |
| Round-1 B2 scheduled delivery | **REMAINS CLOSED** | No changed source regressed the pre-dequeue `ScheduledDelivery` guard or its delivery lifetime. |
| Round-1 B3 reload intake | **REMAINS CLOSED** | Reload still invokes `cancel_intake()` immediately after publishing the now correctly ordered transition. |
| Round-1 B4 other refusal paths | **REMAINS CLOSED** | Debug, waiter, recovery, ordinary background spawning, and adoption behavior remain fail-closed or cleanup-trackable as previously reviewed. |
| Round-1 B5 / I1 watchdog and executor fallback | **REMAINS CLOSED for current production callers** | No related code changed; production arm/begin callers retain the reviewed Tokio/fallback coverage. |
| Round-1 I2 / M1 | **REMAIN CLOSED** | StartupRecovery TTL and cancelled watchdog marker are unchanged. |

## Gate checklist

| Acceptance gate | Result | Evidence |
|---|---|---|
| Idle exit requires zero clients and zero active leases | **PASS** | Idle claim atomically requires an empty lease table and closes acquisition. Every counted main/gateway connection now requires a successful `ClientConnection` guard first; refused accepted connections are dropped uncounted, and all admitted paths release exactly once. Scheduled delivery and all other enumerated active work remain leased. |
| SIGTERM, reload, persistent idle, and temporary-owner exits invoke bounded shutdown | **PASS** | All reasons retain coordinator routing, bounded drain/cleanup, intake cancellation, watchdog fallback, and terminal publication. Reload now closes acquisition before publishing `Draining`. Adopted original futures are truly abortable during cleanup rather than detached. No reviewed work class can silently begin or survive cleanup without the intended tracking/termination semantics. |

## What I did not check

- I did not run the entire `jcode-app-core --lib` suite, full workspace suite, Miri, Loom, sanitizers, or model checking.
- I did not rebuild `target/selfdev/jcode`; the requested fixture rerun used the supplied artifact and exposed that it was not built from exact commit `2b5607882`.
- I did not execute the F03 lease-class hold/release matrix, forced-watchdog fixture, pairwise reason races, parent-SIGKILL recovery, reload success/failure/refusal fixtures, or Windows-specific process behavior.
- I did not add a deterministic accept-versus-idle-claim runtime test. The source pairing is complete, and the pure lease-table atomicity test passes.
- I did not modify implementation or evidence files.

## Confidence

**High (98%).** The PASS rests on complete source-path and ownership analysis of both remaining blockers, not on the disputed runtime artifact. There are exactly two production client-count writes and every route to them is paired with a successful lease acquisition and exactly one teardown. The adopted original future's abort authority is explicitly retained and exercised by a passing regression test. The only material unresolved issue is evidence provenance, which does not reveal a blocking defect in exact source commit `2b5607882`.
