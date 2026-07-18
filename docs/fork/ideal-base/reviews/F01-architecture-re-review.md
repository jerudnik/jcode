# F01 architecture re-review, revision 2

- Reviewed graph node: `F01`.
- Exact reviewed commit: `09f36709838c30c5ae3edb3394621b1d44e10d11` (`F01: revision 2 design resolving all FAIL-review findings (F01-R)`).
- Reviewer model actually used: OpenAI `gpt-5.6-sol`, high effort, per D009/D011 routing.
- Review mode: independent, adversarial re-review.
- Date: 2026-07-18.

## Verdict

**FAIL. Two blocking gaps remain.**

Revision 2 fixes the crate inversion, MCP surface coverage, reload self-lease, idle epoch, temporary-reload disposition, cleanup API naming, and most of the concurrent coordinator protocol. It does not, however, satisfy the original revision gate in full:

1. the provider-turn caller-family census omits the startup headless reload-recovery caller at `crates/jcode-app-core/src/server.rs:1009`, so the promised per-entry-family fixture set is incomplete; and
2. the accept-loop protocol still cannot both await and return a terminal cleanup result and have the same executor terminate the process at its sole termination call site.

Either gap is sufficient to withhold PASS under the instruction that any remaining blocker means FAIL.

## Scope and validation performed

I reviewed the committed tree at exact commit `09f367098`, using a detached archive of that tree for source inspection. The current checkout was at later commit `94b768f5b`; the only change from `09f367098` to that checkout before this review was `docs/fork/ideal-base/STATE.json`. There are no Rust, Cargo, F01 evidence, acceptance-standard, or work-graph changes between the exact reviewed commit and that later checkout. There are also no crate-source changes between the design's stated source-verification commit `398b51c07` and `09f367098`.

I read completely:

- `docs/fork/ideal-base/evidence/F01/design.md` revision 2;
- `docs/fork/ideal-base/evidence/F01/revision_response.md`;
- the original FAIL review at commit `7563a1237`;
- `docs/fork/ideal-base/ACCEPTANCE_STANDARD.md`, especially A0/A1;
- both copies of the F02/F03 nodes in `WORK_GRAPH.json`.

Read-only validation included:

- auditing every file-and-line citation in revision 2 against the exact source tree;
- inspecting the three relevant Cargo manifests;
- independently grepping all references to `process_message_streaming_mpsc`;
- inspecting `McpManager::call_tool`, its pooled, owned, and connect-on-first-call paths, the registered MCP tool caller, and `SharedMcpPool::call_tool`;
- checking the shutdown reason lattice, deadline rule, actor ownership, reload phases, watchdog outcome/budget, temporary reload refusal, cleanup APIs, residue contract, and accept-loop arms;
- checking B1-B3, I1-I5, and all ten original revision-gate items.

Per instruction, I did not run Cargo builds or tests.

## Findings

### Blocking

#### B-R1. The provider-turn caller-family table is not complete

Revision 2 says the caller families were enumerated and lists six families at `design.md:364-383`: client message tasks, client actions, swarm assignment, spawned/headless initial turns, Jade relay, and live wake turns. An independent exact-tree grep finds one additional production call:

- `crates/jcode-app-core/src/server.rs:1009-1016`, inside `recover_headless_sessions_on_startup`, directly invokes `process_message_streaming_mpsc` to resume a headless continuation after reload.

The complete non-test call-site set at `09f367098` is:

- `server.rs:1009`;
- `client_lifecycle.rs:2861`;
- `client_actions.rs:1101`;
- `comm_control.rs:991`;
- `comm_session.rs:886`;
- `jade_relay.rs:1211` and `:1242`;
- `live_turn.rs:120`.

The common-boundary guard proposed at `client_lifecycle.rs:3179` would protect the omitted call at runtime, so this is not a lease-placement hole. It is nevertheless a blocking coverage and verification hole. The design requires “one fixture per distinct `ProviderTurn` entry family from the 3.3 caller census” at `design.md:578-583`, and original revision-gate item 10 requires runtime fixtures for every distinct work entry path (`F01-architecture-critique.md:305-306`). Because startup reload recovery is absent from the census, the prescribed fixture matrix can pass while never exercising that distinct entry path. The revision response repeats the incomplete enumeration at `revision_response.md:84-93`.

Required correction: add the `server.rs:1009` startup headless reload-recovery family to section 3.3 and require its own F03 runtime fixture. The wiring-census test must distinguish definitions/imports/tests from all production call sites.

#### B-R2. Awaitable accept-loop completion contradicts executor-owned process termination

Revision 2 specifies all of the following:

- one actor performs every transition, cleanup step, and final termination call (`design.md:221-252`);
- `begin_and_wait` awaits the terminal outcome of the whole shutdown (`design.md:227-247`);
- `TerminalOutcome::Exited` means full cleanup ran (`design.md:243-247`, `:565-568`);
- accept-loop failure calls `begin_and_wait(AcceptLoopFailure)` and **only then returns** `Err(AcceptLoopFailed)` (`design.md:319-324`, `:482`);
- the process terminates at exactly one call site inside the executor (`design.md:250-252`).

Those statements are not jointly implementable if “termination” is the process exit required by the exit-code table. Once the executor invokes its sole process-termination call, the waiting `Server::run` future cannot resume and return `Err(AcceptLoopFailed)`. If the actor publishes `Exited` before calling process exit so the waiter can resume, `Exited` no longer denotes the terminal outcome of the whole shutdown and creates a race between the caller's error return and executor termination. If the actor does not exit for this reason and leaves exit-code mapping to `run()`'s caller, then termination is not exclusively at the executor's one call site.

The actual source confirms why this matters: both listener arms are inside `Server::run` at `server.rs:2181-2196`, and the local daemon-lock guard remains scoped in that function. Original I5 required this path to await cleanup, return a distinct nonzero error/code, and keep the lock alive through cleanup (`F01-architecture-critique.md:255-268`). Revision 2 claims all three without defining a coherent ownership handoff.

Required correction: choose one exact protocol. One viable design is for the coordinator to return a `Cleaned { reason, code }` pre-termination result specifically to an owning top-level runner, with that runner being the sole termination authority after resource guards drop. Another is for the executor to terminate and make accept-loop `begin_and_wait` non-returning. The design must then remove the claim that `Server::run` returns a distinct error. The chosen protocol must preserve exactly-once termination and lock ordering.

### Important

#### I-R1. The background acquisition citation does not identify acquisition branches

`design.md:420` cites `background.rs:454/529/656/740` as examples of “non-detached spawn/registration branches.” Those lines are `detached: false` fields in initial/final status structures, not task spawn or live-map registration boundaries. For the first branch, the future is spawned at `background.rs:483-484` and inserted into the live map at `:584-600`; the adopted-task branch has analogous later boundaries. This does not invalidate the lower-crate injection design, but it is an inaccurate wiring citation and leaves F02 without the precise RAII guard scope revision 2 otherwise demands.

Required correction: cite the method entry and the actual spawn/adopt plus registration lifetime, and specify whether the guard is created before the future can execute and retained until terminal pruning.

#### I-R2. The watchdog model retains an internal single-call-site ambiguity

The design distinguishes `ForcedExit` from `Exited`, assigns code 70, records an armed marker, and makes the budget inequality explicit (`design.md:326-349`). That resolves the substance of original I2. However, `design.md:250-252` still says process termination occurs at exactly one call site inside the executor, while the watchdog is an OS thread that must itself terminate a stuck executor. The revision response acknowledges the watchdog as an exception, but the design should state the two authorized termination sites explicitly: normal executor termination and coordinator-armed watchdog forced termination. This is important documentation precision, not an additional blocker because the outcome and residue semantics are otherwise defined.

### Minor

#### M-R1. Two stale-socket citations are off target

`design.md:31` and `design.md:517` cite `socket.rs:71` for stale-socket reap. At the reviewed commit, line 71 is merely the closing brace before `socket_has_live_listener`; the stale-reap rationale begins at `socket.rs:76` and `reap_stale_socket_if_dead` begins at `socket.rs:92`. The claimed mechanism exists, but the cited line does not match.

#### M-R2. Deadline wording should distinguish durations from absolute deadlines

The upgrade rule at `design.md:266-269` says the deadline becomes `min(remaining_deadline, full_deadline(new_reason))`. This is directionally correct and guarantees no extension, but the names can denote unlike quantities. F02 should define either `new_deadline = min(current_absolute_deadline, now + full_budget(new_reason))` or `new_remaining = min(current_deadline - now, full_budget(new_reason))`. This is not blocking because the intended rule is otherwise unambiguous.

## Citation audit results

I audited all revision-2 source citation spans against exact commit `09f367098`. The cited source tree is unchanged from the document's `398b51c07` verification base.

**Citations that do not match their stated claim:**

1. `design.md:420` -> `background.rs:454/529/656/740`: these are status-structure fields, not spawn/registration acquisition sites. Actual relevant boundaries include `background.rs:483-484` and `:584-600` for `spawn_with_notify`, with analogous boundaries in the adopt branch.
2. `design.md:31` -> `socket.rs:71`: stale-socket reap is not at line 71; rationale/function are at `socket.rs:76-126`.
3. `design.md:517` -> `socket.rs:71`: same mismatch.

**Material omission discovered by citation/reference audit:**

- The caller table at `design.md:372-379` omits `crates/jcode-app-core/src/server.rs:1009`, the startup headless reload-recovery direct call.

All other audited citations materially matched the named definitions, call paths, constants, cleanup APIs, or source behavior. In particular:

- `jcode-base` depends on `jcode-core` at `crates/jcode-base/Cargo.toml:104`;
- `jcode-app-core` depends on both at `crates/jcode-app-core/Cargo.toml:88-89`;
- `crates/jcode-core/Cargo.toml` depends on neither `jcode-base` nor `jcode-app-core`;
- `McpManager::call_tool` at `manager.rs:342-408` contains the pooled fast path, owned-client fast path, and both connect-on-first-call retry branches;
- the registered tool reaches it at `mcp/tool.rs:49-58`;
- `SharedMcpPool::call_tool` exists at `pool.rs:232-243`;
- `registry::unregister_server_bounded`, `transport::remove_socket`, `lifecycle::cleanup_temporary_metadata`, and `SharedMcpPool::disconnect_all` exist at the cited locations;
- `BackgroundTaskManager::finalize_non_detached(reason)` does not exist and is honestly declared NEW and F02-owned;
- temporary reload signals are currently wired unconditionally before mode selection;
- the accept-loop arms are at `server.rs:2181-2196` and currently return `Ok(())` after runtime shutdown.

## Revision-gate checklist

| # | Required revision item | Result | Re-review conclusion |
|---|---|---|---|
| 1 | Crate-safe shared activity interface and F02 ownership | **PASS** | `jcode-core` is a feasible neutral seam; F02 owns both core files, both MCP files, background manager, server files, and tool injection. |
| 2 | Pooled and non-shared MCP calls | **PASS** | Manager entry covers pooled, owned, and connect-on-first-call paths; direct pool entry is separately wrapped. |
| 3 | Provider-turn lease at common execution boundary | **PASS** | Acquisition inside `process_message_streaming_mpsc` covers all callers by construction. |
| 4 | Separate headless existence, startup recovery, active turns | **PASS** | Session existence is unleased; bounded `StartupRecovery` covers scheduling; actual turns use `ProviderTurn`. |
| 5 | Serialized executor, precedence, deadline update, awaitable completion | **FAIL** | Actor, total precedence, and deadline direction are supplied, but accept-loop awaitable completion is inconsistent with executor-owned process termination. |
| 6 | Remove/resolve `ReloadHandoff` self-lease | **PASS** | Reload is phase state only and the coordinator holds no drain-blocking lease. |
| 7 | Continuous quiescence epoch | **PASS** | `idle_since` is cleared whenever non-quiescent and starts only on transition to full quiescence. |
| 8 | Reconcile watchdog with I2/I5/I7/A0 | **PASS with important clarification** | `ForcedExit`, code, marker, strict budget inequality, and reconciliation fixture are defined; authorized termination-site wording remains inconsistent. |
| 9 | Real cleanup APIs and complete residue set | **PASS** | Existing APIs were verified; the sole missing API is explicitly NEW and F02-owned; lock and registry are included. |
| 10 | Temporary reload disposition, pairwise races, every entry-path fixture | **FAIL** | Typed temporary refusal and pairwise races are present, but the provider census omits the `server.rs:1009` startup recovery entry path, so its fixture is not required. |

## Original finding disposition

| Original finding | Result |
|---|---|
| B1 crate/owner gap for MCP | Resolved. |
| B2 undefined concurrent coordinator and reload self-dependency | Resolved except for the separate accept-loop terminal ownership contradiction recorded as B-R2. |
| B3 incomplete provider/headless boundary | Runtime lease placement resolved; caller-family enumeration and fixture coverage remain incomplete, recorded as B-R1. |
| I1 idle-window contradiction | Resolved. |
| I2 watchdog loophole | Substantively resolved; termination-site wording needs clarification. |
| I3 fake cleanup API/incomplete residue | Resolved. The new API is honestly declared and owned. |
| I4 temporary reload omitted | Resolved with typed refusal. |
| I5 accept-loop must await completion | Not fully resolved because the described await-and-return protocol conflicts with executor-owned process termination. |

## What I did not check

- I did not compile or run tests, as explicitly instructed for this design-only review.
- I did not execute daemon exit, provider, MCP, process-tree, forced-watchdog, or residue fixtures. Those are F02/F03 work.
- I did not prove Windows-specific watchdog, lock, or process-replacement behavior.
- I did not review ideal-base nodes outside F01-F03 except where cited by A0/A1 or cleanup/recovery ownership.
- I did not evaluate implementation quality of APIs that revision 2 declares new, because they do not yet exist.

## Confidence

**High.** The two blockers follow from an exhaustive exact-tree reference search and a direct contradiction among the design's own executor, terminal-outcome, and accept-loop-return requirements. The crate dependency and MCP checks are source-verified, and the remaining original findings were checked individually rather than accepted from `revision_response.md` claims.
