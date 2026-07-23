# F06 independent implementation review

## Verdict

**PASS.**

Reviewed exact commit `84dc0aa2b5989a14a3ca7d4d215636fb4ecf0c1b`
(`F06: track and reap owned MCP children`), which is HEAD of `main`.

Reviewer route: **Anthropic Claude, routed as `claude-opus-4-8`** (stated
honestly). This route was reached only after the intended cross-vendor
reviewers failed: Kimi k3 (tool-schema incompatibility), `cursor-grok-4.5-high`
and MiniMax-M3 (both rejected as unknown-model-ID on the Cursor route). I ran
the tests and read the source directly; I did not rely on the worker's
transcript.

Both F06 acceptance gates are honestly met:

1. *Pooled children receive the owning daemon PID* - **MET.** Every spawned MCP
   child (pooled leader + per-session owned) is spawned through
   `McpClient::connect_with_tracker`, which injects `JCODE_MCP_OWNER_PID` into
   the child environment *after* merging config env, so a server config cannot
   spoof or erase the owner identity (`client.rs:484-491`). The child PID plus
   owner PID are recorded in a process-wide registry (`client.rs:503`,
   `TrackedMcpChild{server_name,pid,owner_pid}`), exposed via
   `pool.tracked_children()` / `pool.owner_pid()` and a `Serialize` record
   (`pool.rs:192-208`, `client.rs:27-33`). Proven by the real-process test that
   deliberately spoofs `JCODE_MCP_OWNER_PID=1` in config and confirms both the
   child env *and* the tracking record hold the daemon PID
   (`pool.rs` `pooled_child_receives_and_records_owning_daemon_pid`).
2. *Shutdown coordinator disconnects and reaps all tracked children within
   grace* - **MET.** Cleanup step 6 (`shutdown.rs:1118-1155`) runs disconnect
   under a 25 ms timeout then `reap_tracked_children(175 ms)`, entirely inside
   the 250 ms `bounded_step` (`25 + 175 = 200 < 250`, asserted by
   `mcp_disconnect_and_reap_fit_cleanup_step_budget`). The reap does a bounded
   graceful -> TERM -> KILL escalation and unregisters every tracked PID so the
   step leaves a zero-tracked postcondition (`client.rs:142-184`), which the
   coordinator re-checks via `pool.tracked_children()` (`shutdown.rs:1134`).
   Proven by the real-process test that spawns a TERM-resistant child and
   observes TERM, then KILL, `unreaped=[]`, `tracked_after=0`, sub-second.

The gates are met for the production single-process daemon topology. The
findings below are real but do not defeat either required gate.

## Validation performed

All commands run in the repo Nix dev shell on macOS aarch64 at HEAD
`84dc0aa2b`.

### Gate test suites (both required)

```text
$ scripts/dev_cargo.sh test -p jcode-base --lib mcp
running 41 tests
...
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 1151 filtered out; finished in 30.02s

$ scripts/dev_cargo.sh test -p jcode-app-core --lib server::shutdown
running 24 tests
...
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 1126 filtered out; finished in 2.08s
```

Both suites pass at the required counts (41 and 24).

### Real-process re-execution (not replayed logs)

I re-ran the two real-process tests with `--nocapture`. Both emitted **fresh
PIDs** that differ from the committed `process-tree.txt` (which shows
`child_pid=98203/owner=98202` and `pid=98725`), proving the tests genuinely
fork processes rather than printing canned strings:

```text
$ ... real_child_ignoring_term_is_killed_and_reaped_within_grace -- --nocapture
F06_REAP pid=91448 owner_pid=91447 term=[91448] kill=[91448] unreaped=[] tracked_after=0 elapsed_ms=243
test result: ok. 1 passed; 0 failed

$ ... pooled_child_receives_and_records_owning_daemon_pid -- --nocapture
F06_OWNER server=owner-pid-fixture child_pid=92005 owner_pid=92004 env_owner_pid=92004
test result: ok. 1 passed; 0 failed
```

The reap test used a 300 ms grace and completed in 243 ms (graceful quarter
elapses, TERM at ~75 ms is trapped, KILL at ~225 ms, reaped by `waitpid`
shortly after), consistent with the escalation math.

### Evidence integrity

```text
$ shasum -a 256 -c docs/fork/ideal-base/evidence/F06/SHA256SUMS
crates/jcode-base/src/mcp/client.rs: OK
crates/jcode-base/src/mcp/manager.rs: OK
crates/jcode-base/src/mcp/mod.rs: OK
crates/jcode-base/src/mcp/pool.rs: OK
crates/jcode-app-core/src/server/shutdown.rs: OK
src/cli/mcp_serve.rs: OK
docs/fork/ideal-base/evidence/F06/README.md: OK
docs/fork/ideal-base/evidence/F06/process-tree.txt: OK
```

All eight tracked files match the committed source. `process-tree.txt` records
genuine `F06_OWNER`/`F06_REAP` transcript lines with concrete PIDs, not claims.

### Scope

`git show 84dc0aa2b --name-only` touched only:
`crates/jcode-base/src/mcp/{client,manager,mod,pool}.rs`,
`crates/jcode-app-core/src/server/shutdown.rs`, `src/cli/mcp_serve.rs`, and
`docs/fork/ideal-base/evidence/F06/**`. All are inside the F06 `owned_paths`
(`crates/jcode-base/src/mcp/**`, `crates/jcode-app-core/src/server/**`,
`src/cli/mcp_serve.rs`, `src/cli/dispatch.rs`, `evidence/F06/**`).
`src/cli/dispatch.rs` is owned but was not modified (permitted, not required).
No out-of-scope file was touched. The changed `SharedMcpPool::disconnect_all`
signature (`() -> Vec<u32>`) is consumed only by its two callers
(`shutdown.rs:1125`, `pool.rs:306`); the identically-named
`McpManager::disconnect_all` is a distinct method and is unaffected.

## Findings

### Blocking

None. Both required gates are honestly met and independently reproduced.

### Important

1. **`mcp-serve` owner self-liveness is defeated by owner-PID reuse.**
   `poll_owner` (`mcp_serve.rs:66-77`) uses `kill(owner_pid, 0)`: `ESRCH ->
   Dead`, `EPERM -> Live`, else `Unknown`, polling every 250 ms
   (`OWNER_POLL_INTERVAL`). If the owning daemon dies and the OS recycles its
   PID onto an unrelated process before the next 250 ms poll, `kill(0)` returns
   `0` (or `EPERM`), the poller reports `Live`, and `mcp-serve` never exits.
   The orphan then survives on stdin/EOF alone. There is no start-time or
   owner-token cross-check to disambiguate a recycled PID. This is the classic
   PID-reuse race; it is narrow (requires death + recycle inside one poll
   interval) and it affects the self-added third behavior, **not** either
   required acceptance gate (the two gates concern pooled-child ownership and
   coordinator reap, both of which pass). Worth a token/start-time check in a
   follow-up.

2. **ECHILD -> `kill(pid,0)` fallback can mislabel a recycled PID as live
   under concurrent reap.** `child_is_live` (`client.rs:230-256`) normally
   reaps via `waitpid(WNOHANG)`; because these are the daemon's own children, a
   zombie holds its PID until the daemon reaps it, so the common path cannot
   signal a recycled PID. The exception is the `ECHILD` branch: if the same PID
   is reaped by another concurrent reaper *while still registered* (e.g. the
   `Drop`-spawned reap task at `client.rs:743-746` racing the explicit
   pool/manager reap on the same tracker), a second `child_is_live` observes
   `ECHILD`, falls back to `kill(pid,0)`, and a freshly recycled same-user PID
   would report `Live` and then receive SIGTERM/SIGKILL. This is a genuine but
   very narrow race: the primary shutdown path (gate 2) runs a single reaper
   over owned children whose PIDs cannot be recycled before that reaper acts,
   so the gate is not affected. The `shutdown_started` flag (`client.rs:733`)
   suppresses the `Drop` path for the graceful shutdown route, further
   shrinking the window. Recommend routing every PID through exactly one reaper
   and/or verifying the tokio `Child` exit status instead of `kill(0)` in the
   ECHILD branch.

### Minor

3. **`Drop` reap is best-effort during runtime teardown.** `Drop`
   (`client.rs:743-746`) spawns a detached `reap_pids` task when a Tokio
   runtime is current, else sends a single SIGTERM (no KILL escalation,
   `client.rs:747-748`). If the runtime is already winding down, the detached
   task may never run and the child is left to the OS. This does not affect
   gate 2, which reaps explicitly through the pool tracker rather than via
   `Drop`, but abrupt (non-`disconnect_all`) session teardown relies on a
   best-effort path.

4. **Raw `waitpid` competes with the retained tokio `Child`.**
   `child_is_live` calls `libc::waitpid(pid, WNOHANG)` directly while
   `McpClient` still owns `self.child: tokio::process::Child`
   (`client.rs:459`). This reaps the child out from under the tokio handle; a
   later `is_running()` -> `self.child.try_wait()` then hits `ECHILD` and is
   folded to `false` (`client.rs:677-681`), which is benign but means the two
   reaping mechanisms are not coordinated. Functionally harmless today.

5. **Windows and non-unix paths are compile-reviewed only.** `signal_child`
   (taskkill `/T`, `/F`) and `poll_owner` (OpenProcess/GetExitCodeProcess) are
   plausible but were not executed on this macOS host, as the evidence README
   itself acknowledges (README "Edge cases", final bullet).

## Gate checklist

| Gate | Status | Basis |
|---|---|---|
| Pooled children receive the owning daemon PID | PASS | Owner PID injected post-config-merge for pooled + owned clients (`client.rs:484-503`, `pool.rs:435`, `manager.rs:209,289`); recorded and introspectable (`pool.rs:192-208`); proven by real-process spoof-resistant test |
| Shutdown coordinator disconnects and reaps all tracked children within grace | PASS | Bounded disconnect+reap inside 250 ms step (`shutdown.rs:1118-1155`), TERM->KILL escalation with zero-tracked postcondition (`client.rs:142-184`), budget-fit test, and real TERM-resistant reap test (`unreaped=[]`, `tracked_after=0`, sub-second) |
| (`mcp-serve` self-liveness, F06 content but not a required gate) | Works with a caveat | Fail-safe `Dead`-only exit, 250 ms poll, `EPERM`/`Unknown` keep alive, `None` owner waits forever; defeated only by owner PID reuse (Important #1) |
| Scope confined to owned paths | PASS | Only mcp/**, server/shutdown.rs, mcp_serve.rs, evidence/F06/** touched |

## What I did not check

- Windows and non-unix `signal_child` / `poll_owner` runtime behavior (no host
  available; compile-reviewed only).
- End-to-end daemon shutdown against real long-lived MCP servers under load; I
  exercised the unit and real-child fixtures, not a full production daemon
  lifecycle with many concurrent sessions.
- The exact interaction of the `Drop`-spawned reap task racing the explicit
  coordinator reap under real timing (Important #2 reasoned from code, not
  triggered with a stress test).
- Behavior under a poisoned tracker mutex beyond the `into_inner()` recovery
  pattern (`client.rs:96-114`); recovery is coded but I did not force a panic
  to observe it.
- I did not audit non-MCP cleanup steps (1-5) beyond confirming step 6 stays
  within the shared `CLEANUP_BUDGET`.

## Confidence

**High** that both required acceptance gates are honestly met: I reproduced
both test suites (41 + 24), re-ran both real-process tests with fresh PIDs,
verified SHA256SUMS, and read the production code paths (not just the diff).
**Medium** on the long-tail robustness captured in the Important findings
(owner-PID reuse and the concurrent-reap ECHILD window); those are real
race-condition gaps but are narrow, do not block either gate, and warrant a
follow-up rather than a FAIL.
