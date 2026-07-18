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
