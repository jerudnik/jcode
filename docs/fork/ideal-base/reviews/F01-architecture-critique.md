# F01 adversarial architecture critique

- Reviewed graph node: `F01`.
- Reviewed commit: `93c59a218` (`F01: fix design.md section reference in evidence README`).
- Primary artifact: `docs/fork/ideal-base/evidence/F01/design.md`.
- Reviewer model actually used: OpenAI `gpt-5.6-sol`, high effort.
- Review mode: independent, adversarial architecture critique.
- Date: 2026-07-18.

## Verdict

**FAIL. Revision is required before F01 can satisfy its architecture gate.**

The design has a useful exit/work census and the right high-level goal, but the
acceptance gate "Independent architecture critique finds no owner/lease gap" is
not met. There are three blocking gaps:

1. the proposed authority placement and F02 write ownership cannot instrument
   all production MCP calls;
2. the coordinator state machine has no complete concurrent runtime protocol and
   its `ReloadHandoff` lease can block its own drain;
3. the stated provider/headless wiring does not cover the actual turn lifetime
   boundary.

There are also material contradictions in idle-window semantics, watchdog
behavior, temporary-server reload handling, and cleanup APIs. Implementing F02
literally from this document would either fail to compile across crate layers or
ship paths that remain outside the claimed single authority.

## Scope and validation performed

I reviewed the exact committed tree at `93c59a218`, not the dirty worktree.
Although the evidence header names source commit
`c96c4b57de57438d63e23796e6b038027265fca4`, the only changes from that commit to
`93c59a218` are the two F01 evidence files, so the cited Rust source is unchanged.

Read-only checks included:

- full read of `evidence/F01/design.md`, `evidence/F01/README.md`,
  `ACCEPTANCE_STANDARD.md`, and F01-F03 records in `WORK_GRAPH.json`;
- termination-authority grep for `process::exit`, exec, and abort sites;
- inspection of server lifecycle, reload, runtime task scope, accept loops,
  temporary-server startup, daemon lock, provider-turn entry points, headless
  creation/recovery, debug jobs, background manager, swarm waits, and MCP
  manager/pool/client/tool paths;
- crate dependency and work-graph ownership checks;
- comparison of `c96c4b57...93c59a218` to establish source provenance.

No source or runtime behavior was modified or executed by this review.

## Blocking findings

### B1. The authority placement and F02 ownership cannot cover production MCP calls

The design puts both new authorities in
`crates/jcode-app-core/src/server/` (`design.md:94-99`) and assigns C7 acquisition
to `pool.rs:232` (`design.md:188-197`). In the reviewed tree, however, MCP lives
in the lower `jcode-base` crate:

- `crates/jcode-base/src/mcp/pool.rs:231-243` performs pooled calls;
- `crates/jcode-base/src/mcp/manager.rs:342-404` performs both pooled-handle and
  per-session owned-client calls;
- `crates/jcode-base/src/mcp/tool.rs:49-58` reaches that manager from the actual
  registered tool;
- `jcode-app-core` depends on `jcode-base`, so `jcode-base` cannot import a
  server-local authority from `jcode-app-core` without a dependency cycle.

The census is also incomplete. C7 is described only as "Pooled MCP calls in
flight" (`design.md:55-58`), while daemon-mode `McpManager` explicitly supports
non-shared per-session children (`mcp/manager.rs:1-5,44-54`) and calls them on
the production path (`mcp/manager.rs:357-364,384-388`). A1 requires "MCP calls,"
not only shared-pool calls (`ACCEPTANCE_STANDARD.md:17-23`).

F02's owned paths include server files and
`crates/jcode-base/src/background.rs`, but not `crates/jcode-base/src/mcp/**`.
Thus the prescribed C7 acquisition point is outside both the proposed
component's crate layer and F02's authorized write set.

**Failure scenario:** a non-shared/stateful MCP tool is in flight with zero
clients. All server-local lease counts are zero because the actual call path in
`jcode-base` cannot acquire the app-core lease. Persistent idle exit kills it,
contradicting A1 and the F01 gate.

**Required correction:** choose and document an implementable inversion seam.
For example, put a small process-activity interface/guard in a lower neutral
crate that both layers can depend on, or inject a lease callback/trait into
`McpManager` and `BackgroundTaskManager` from the composition root. Expand F02
ownership to the selected MCP files. Census and test both pooled and non-shared
MCP calls.

### B2. The coordinator lacks a complete concurrent ownership protocol, and reload can self-block

The pure phase sketch is not enough to establish one runtime authority. The
artifact says `begin(reason)` is "idempotent-with-priority" and that a stronger
reason upgrades the deadline (`design.md:156-160`), but it does not define:

- the complete priority order among `SigTerm`, both idle reasons,
  `TemporaryOwnerExit`, `Reload`, `ReloadExecFailed`, and
  `AcceptLoopFailure`;
- whether a deadline upgrade shortens, extends, or preserves an already-running
  drain;
- which task owns the one transition executor;
- how simultaneous callers observe or await terminal completion;
- how exactly-once cleanup and exactly-once process termination are enforced.

Only `SIGTERM > idle` is stated. That does not "serialize E1 vs E4 by
construction" as claimed at `design.md:158-160`, because E4 is reload, not
SIGTERM. The matrix adds "reload loses to SIGTERM" (`design.md:234-242`) but
still does not order reload against idle or accept-loop failure.

There is also a self-dependency. `ReloadHandoff` is a lease acquired from signal
receipt through exec/failure (`design.md:103-114,188-197`). `begin(Reload)` enters
`Draining`, where the coordinator waits for leases and rejects new acquisition
(`design.md:148-169`). The reload lease cannot be released until exec/failure,
which occurs only in `Handoff` after draining (`design.md:179-183`). Unless the
coordinator silently excludes its own lease, every reload waits until the drain
deadline because it is waiting on itself. No exclusion or administrative-lease
rule is specified.

**Failure scenario:** reload acquires C10, calls `begin(Reload)`, enters
`Draining`, and can never observe an empty lease table. A simultaneous idle or
accept-loop signal has undefined precedence. Two callers may each start runtime
cleanup because the pure `begin` result is not tied to a single executor.

**Required correction:** specify a shared coordinator handle backed by one
serialized executor/actor, a total or explicit partial priority lattice, exact
deadline-update rules, and `begin_and_wait(reason)` semantics for paths that may
return. Make coordinator-internal activity distinct from drain-blocking work, or
remove `ReloadHandoff` from the lease table and represent it solely as phase
state. Add race tests for every pair of reasons, not just generic random
transitions.

### B3. Provider and headless lease wiring misses the actual turn boundary

The enum promises `ProviderTurn` for "any streaming turn" and `HeadlessTurn`
for restored/headless work (`design.md:103-114`). The wiring table instead says:

- C2 acquires at `live_turn.rs:93`;
- C3 acquires at `headless.rs:38` plus the startup recovery window
  (`design.md:188-197`).

Those are not the complete or correct lifetimes.

`process_message_streaming_mpsc` is the common provider-turn boundary
(`client_lifecycle.rs:3178-3205`). It is called by:

- client message tasks (`client_lifecycle.rs:2858-2869`);
- live wake turns (`live_turn.rs:113-127`);
- swarm task assignment (`comm_control.rs:987-998`);
- spawned/headless initial turns (`comm_session.rs:878-893`);
- additional session, client action, and Jade relay paths found by reference
  search.

Leasing only `live_turn.rs:93` protects one caller family and leaves other
server-initiated turns unleased. Conversely, `create_headless_session`
(`headless.rs:38-284`) creates and registers an idle session but does not own the
subsequent provider turn. Holding `HeadlessTurn` for the whole session would pin
an otherwise idle daemon indefinitely; dropping it when creation returns would
not protect the later turn.

**Failure scenario:** a swarm assignment or spawned headless initial turn calls
`process_message_streaming_mpsc` directly while no client is connected. No C2
lease exists if F02 follows the table. The daemon can idle-exit mid-turn. An
attempted fix at `create_headless_session` either releases too early or leaks for
the session lifetime.

**Required correction:** acquire the provider-turn guard at one truly central
execution boundary, preferably around the full future in
`process_message_streaming_mpsc` or a still-lower `Agent` turn entry shared by
all callers. Treat startup restoration/loading as a separate bounded recovery
lease. Do not use headless session existence as turn activity. Enumerate all
callers in the wiring table and add one test per distinct entry family.

## Important findings

### I1. Idle-window semantics do not implement the stated invariant

I1 requires clients and leases to remain zero for the **entire** idle window
(`design.md:252-256`). The runtime prose says the idle clock resets whenever a
client is present or "any lease is acquired" (`design.md:203-222`). Reset-on-
acquire alone is insufficient.

If a lease is acquired, held longer than the idle timeout, and then released,
an elapsed timer that continued from acquisition can allow exit immediately on
release. The pure `should_exit` function cannot detect this because it receives
only the current snapshot and an externally computed elapsed value
(`design.md:208-217`).

**Required correction:** define one quiescence epoch. `idle_since` must be
`None` whenever either clients or any drain-blocking lease is nonzero, and set
only on transition to the fully quiescent state. Exit is allowed only after that
continuous quiescent interval. Property-test a lease held longer than timeout,
then released, and require a full new timeout.

### I2. The hard watchdog contradicts the single-authority and phase invariants

The design preserves a direct OS-thread `process::exit` watchdog
(`design.md:165-178`) while declaring that every voluntary exit converges on the
coordinator and direct exits outside it are violations (`design.md:140-144`).
I2 says every `Exited` is preceded by `Draining` and `CleaningUp` (or reload
handoff), while I7 allows watchdog preemption to skip cleanup
(`design.md:252-269`). Those cannot all be true.

The current watchdog can fire during draining, before `CleaningUp`, and cannot
reliably log each skipped step after the process is gone. A0 does not grant a
"watchdog skipped it" exception to coherent-or-removed sidecars
(`ACCEPTANCE_STANDARD.md:7-15`).

**Required correction:** make the watchdog an implementation detail owned and
armed by the single coordinator, define its forced-exit outcome separately from
a successfully completed `Exited` state, and budget drain plus cleanup below the
hard deadline. Tests must exercise the forced path and assert the exact residue
contract rather than treating skipped cleanup as equivalent to success.

### I3. Cleanup relies on an API that does not exist and under-specifies residue

The cleanup list says it will finalize non-detached background statuses
(`design.md:170-178`), and the non-goals say the manager "already exposes" that
finalize hook (`design.md:285-290`). At `93c59a218`,
`BackgroundTaskManager` exposes orphan reconciliation and per-task cancellation,
but no coordinator-wide finalize/cancel hook. A repository search for such an
API finds only `reconcile_orphaned_tasks` (`background.rs:317`).

The test plan's zero-residue list includes sockets, metadata, hash, PID markers,
and processes (`design.md:271-281`) but omits the daemon lock and registry entry,
both explicitly required by A0. The lock is held by a local
`DaemonLockGuard` in `Server::run` (`server.rs:2091-2100`), and its file is
removed only when that guard drops (`socket.rs:154-169`). The design does not
state how direct coordinator exit, watchdog exit, or exec handoff interacts with
that guard.

The MCP cleanup citation also names the per-client shutdown method rather than
the pool-level `disconnect_all` API (`jcode-base/src/mcp/pool.rs:144-164`).

**Required correction:** specify concrete cleanup interfaces, ownership, error
semantics, and per-step budgets before F02. Add a manager-wide atomic
finalize/cancel operation for non-detached tasks, call pool-level MCP shutdown,
and include socket, debug socket, daemon lock, hash, temporary metadata,
registry, reload state, PID markers, and owned descendants in every applicable
residue fixture.

### I4. Temporary servers can reload, contrary to the coverage matrix

The matrix marks C10 for `TemporaryOwnerExit` as `n/a` because "temp servers do
not reload" (`design.md:234-242`). In the reviewed code,
`spawn_background_tasks` starts `await_reload_signal` unconditionally
(`server.rs:1133-1202`) before choosing temporary versus persistent lifecycle
monitor (`server.rs:1668-1685`). Therefore a temporary server can receive a
reload signal.

**Required correction:** either explicitly disable reload for temporary servers
with a tested typed refusal, or define temporary reload/handoff behavior,
metadata handling, owner-PID semantics, and reason priority in the matrix.

### I5. Accept-loop failure must await coordinator completion before `run` returns

The wiring table says accept-loop failure should call
`coordinator.begin(AcceptLoopFailure)` "before returning" (`design.md:188-197`).
Today `Server::run` cancels and joins connection tasks and then returns `Ok(())`
(`server.rs:2176-2197`). A fire-and-forget `begin` before that return is not
sufficient: the binary or test runtime may tear down while coordinator cleanup
is still pending, and the local daemon-lock guard drops independently of the
cleanup phase.

**Required correction:** this path must await the one coordinator executor to a
terminal cleanup result, return a distinct nonzero error/code for listener
failure, and keep resources such as the lock guard alive until the cleanup
ordering says they may be released.

## Coverage audit summary

| Item | Result | Architecture issue |
|---|---|---|
| C1 client connection | Partial | Duplicated as both `clients` and `ClientConnection` lease; one source of truth is not specified. |
| C1/C2 provider turns | **Gap** | Proposed `live_turn` wiring misses direct common-turn callers. |
| C3 headless/restored | **Gap** | Session creation is not the turn lifetime; recovery and execution scopes are conflated. |
| C4 debug jobs | Plausible | Concrete guard lifetime still needs to wrap both spawned job tasks. |
| C5 non-detached background | **Gap** | Lower-crate inversion seam and manager-wide shutdown/finalize API are unspecified. |
| C6 detached background | Intentional exclusion | Correct only if status reconciliation remains independent of daemon leases. |
| C7 MCP calls | **Blocking gap** | Pooled and owned calls live in lower, unowned MCP files. |
| C8 swarm waiters | Covered in concept | Background persisted waits may pin the daemon until deadline; this policy should be explicit. |
| C9 scheduled delivery | Partial | "Ambient delivery dispatch" is not identified as a concrete acquire/release boundary. |
| C10 reload handoff | **Blocking contradiction** | Drain waits on the lease that can only release after handoff. |
| SIGTERM | Partial | Watchdog may bypass coordinator cleanup and invariants. |
| Persistent/temporary idle | **Gap** | Continuous zero-work idle window is not operationally defined. |
| Temporary owner exit | Partial | Reload concurrency for temporary servers is omitted. |
| Reload / exec failure | **Gap** | Priority and self-lease semantics are undefined. |
| Accept-loop failure | **Gap** | Return path does not require awaiting coordinator completion. |
| Crash/SIGKILL | Out of coordinator scope | Correctly delegated to reconciliation/F06, subject to later fixtures. |

## Required F01 revision gate

F01 should not be accepted until a revised design does all of the following:

1. selects a crate-safe shared activity interface and updates F02 ownership;
2. covers both pooled and non-shared MCP calls;
3. places provider-turn leasing at the actual common execution boundary;
4. separates headless session existence, startup recovery, and active turns;
5. defines one serialized coordinator executor, full reason precedence,
   deadline-update behavior, and awaitable completion;
6. removes or resolves the `ReloadHandoff` self-lease cycle;
7. defines a continuous quiescence epoch for idle timeout;
8. reconciles watchdog behavior with I2, I5, I7, and A0;
9. specifies real cleanup APIs and the complete residue set;
10. resolves temporary-server reload behavior and adds pairwise reason-race
    tests plus runtime fixtures for every distinct work entry path.

## What I did not check

- I did not compile or run Rust tests because F01 is design-only and no source
  change is under review.
- I did not execute live daemon exit fixtures, provider calls, MCP servers, or
  process-tree probes. Those belong to F02/F03 after the architecture is fixed.
- I did not review unrelated ideal-base graph nodes except where F02/F03
  ownership and acceptance gates constrain F01 implementability.
- I did not evaluate Windows-specific process behavior beyond noting that the
  design claims a cross-platform authority.

## Confidence

**High.** The blocking findings follow directly from committed crate layering,
actual production call paths, explicit F02 ownership, and contradictions within
the F01 state model. The exact implementation choice remains open, but the
current design cannot honestly satisfy the "no owner/lease gap" gate.
