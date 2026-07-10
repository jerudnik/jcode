# Swarm Lifecycle Remediation

Incident evidence: ~/notes/projects/jcode/maintenance/bug-run-plan-spawn-storm.md

Status: **proposed** (not started). Prerequisite #3 (version drift) is done and
deployed; this covers the two remaining lifecycle leaks that gate safe swarm use.

## The shared disease

Three incidents — the self-spawn fork bomb, the 231 orphaned `mcp-serve`
children, the ~5,300 stale session markers — are the same failure: jcode trusts a
signal that does not hold when a process dies abnormally.

- **Reaping** trusts OS process-cascade and Rust `Drop`. Neither fires for a
  parent that `std::process::exit`s or is `SIGKILL`ed after its children have
  reparented to init.
- **Markers** trust graceful-exit cleanup. A `SIGKILL`/panic/power-loss leaves
  them behind forever.

The cure is the same shape both times: **explicit PID-liveness checks at the
right lifecycle points**, not implicit cascade. (Version drift, #3, was the third
face of this — trusting version-string equality — and is already fixed.)

---

## Issue #1 — Swarm-run child reaping

Gates modality **(c) regular + swarms**. This is the leak that stranded 231
`mcp-serve` children when a swarm run's private server died.

### Spawn topology

- The swarm run's **private server** is a `jcode serve` daemon started with
  `--temporary-server --owner-pid --temp-idle-timeout-secs` by the external
  swarm-run harness. It is spawned with `setsid()`
  (`crates/jcode-app-core/src/server/socket.rs:262`), so it is a session/process-
  group leader that **reparents to init when its client dies** (deliberate; see
  `crates/jcode-app-core/src/server/lifecycle.rs:82-89`).
- Each **member** is a `jcode mcp-serve` stdio bridge, spawned by the MCP client
  layer with a plain `Command::spawn()` and **no `setsid`/`process_group`**
  (`crates/jcode-base/src/mcp/client.rs:188-195`). Members therefore stay in the
  server's process group (`pgid == server_pid`).

**Key consequence:** the members were always reachable by a single
`kill(-server_pgid, …)`. Nobody ever sent one.

### The gaps

1. **No group-kill / `disconnect_all` on any server-exit path.** The SIGTERM
   handler (`server.rs:1058-1071`), idle exit (`lifecycle.rs:178`), and temp/
   owner-death shutdown (`lifecycle.rs:246-256`) all call `std::process::exit`,
   which skips `McpClient::Drop` (`crates/jcode-base/src/mcp/client.rs:409-413`,
   the code that would kill the child). Graceful *per-session* disconnect drops
   the agent and its children (`server/client_disconnect_cleanup.rs:106-179`),
   but server-level death does not.
2. **Owner-death monitor is on the server, not the members.** The only owner
   liveness check is `spawn_temporary_lifecycle_monitor`
   (`lifecycle.rs:204-213`); on owner death it calls `shutdown_temporary_server`
   → `process::exit` (still no child kill). Members carry no `owner_pid` and no
   monitor of their own.
3. **Members idle forever.** The `mcp-serve` loop (`src/cli/mcp_serve.rs:58-118`)
   only exits on stdin EOF; a reparented member blocks on `read_line` with no
   liveness fallback.

### Fix — two layers (belt + suspenders)

**Layer 1 — explicit reap before exit (belt).** Record member child PIDs
(extend the existing `OwnedChildPermit`, `client.rs:36-61`, to carry the pid),
and add one synchronous `reap_member_children()` (SIGTERM → grace → SIGKILL) that
runs before **every** server `process::exit` path. Explicit-pid rather than
`kill(-pgid)` because the server is in its own group and would re-signal itself.
Reuse `signal_detached_process_group` (`crates/jcode-base/src/platform.rs:281`)
and `try_reap_child_process` (`platform.rs:319`).

**Layer 2 — member self-exit (suspenders).** Thread `--owner-pid` into the
`mcp-serve` spawn (the server supplies its own pid at `client.rs:188`) and add a
liveness monitor to the run loop (`mcp_serve.rs:58-118`) that exits when the
owner dies. This survives even a server `SIGKILL`, which Layer 1 cannot. Model on
`spawn_temporary_lifecycle_monitor` (`lifecycle.rs:190-244`).

### Files

`server.rs` (SIGTERM + idle exits) · `server/lifecycle.rs` (owner-death +
`shutdown_temporary_server`) · `mcp/client.rs` (spawn, Drop, permit+pid) ·
`cli/mcp_serve.rs` (owner self-exit) · `mcp/manager.rs` + `mcp/pool.rs`
(`disconnect_all` as the reap entry point) · reuse `platform.rs:281,319`.

### Test

Spawn a parent with N children in its group, `process::exit` the parent, assert
`reap_member_children` leaves zero survivors. Plus a `mcp-serve` unit: with a
dead `--owner-pid`, the loop self-exits.

---

## Issue #2 — Marker file lifecycle

Independent of swarms but the same disease; fixes the menubar over-count and
unbounded disk growth. **Two separate marker subsystems**, not unified.

### Subsystem A — `~/.jcode/active_pids/`

`crates/jcode-storage/src/active_pids.rs`. Filename = `session_id`, content =
owning PID. Written by `register_active_pid` (`active_pids.rs:26-31`) via
`session.rs:1046,1054`. Removed **only** by `mark_closed`/`mark_crashed`
(`session.rs:1026,1032`) — no `Drop` guard (contrast `StreamingGuard`,
`active_pids.rs:61-77`). The only reaper, `reconcile_active_sessions`
(`session.rs:37-48`), runs on demand (`server/swarm.rs:271`,
`cli/tui_launch.rs:424`) and is gated on session-JSON load + `Active` status, so
a killed session with missing/corrupt JSON leaks forever.

### Subsystem B — `~/.jcode/telemetry_active_sessions/`

`crates/jcode-telemetry-core/src/state_support.rs` (**duplicated** at
`crates/jcode-app-core/src/telemetry_state.rs:165-213`). Filename =
`{session_id}.active`, content = `"1"`. Removed gracefully by
`unregister_active_session` (`telemetry-core/src/lifecycle.rs:327`). Pruned only
by `prune_active_session_files` (`state_support.rs:165-190`) — **by 24h mtime,
not PID liveness** (there is no PID in the file). This freshness count is what
inflated the "~2,581 active sessions" telemetry number.

### The reader

The menubar (`src/cli/commands/menubar.rs:68,74,90,522`) already filters by
liveness via `session_counts`/`session_presence` (`active_pids.rs:152-200`, using
`process_is_running`, `active_pids.rs:108-115`) — so it *skips* dead markers but
**never deletes them** (disk grows unbounded), and the telemetry side counts
freshness, not liveness.

### Fix

1. **Liveness sweep that deletes** dead markers in both dirs (not just skips),
   run unconditionally at server startup and periodically. Standardize on
   `jcode_base::platform::is_process_running` (`crates/jcode-base/src/platform.rs:244`).
2. **Put a PID in the telemetry `.active` file** so its prune is liveness-based,
   not 24h-mtime.
3. **RAII guard** on `register_active_pid` (model `StreamingGuard`) for graceful
   exits; the liveness sweep is the SIGKILL-proof net behind it.
4. **De-dup** `telemetry_state.rs` vs `state_support.rs` and the
   `session_active_pids.rs` re-export while here.

### Files

`jcode-storage/src/active_pids.rs` · `jcode-base/src/session.rs` ·
`jcode-telemetry-core/src/state_support.rs` +
`jcode-app-core/src/telemetry_state.rs` (dedupe) ·
`jcode-telemetry-core/src/lifecycle.rs` · verify `cli/commands/menubar.rs`
triggers the sweep.

### Test

Write markers for a dead PID and a live PID; run the sweep; assert the dead one
is deleted and the live one preserved.

---

## Sequencing

1. **#2 markers** first — isolated, low-risk, needs no swarm to test, and
   immediately fixes the menubar/telemetry over-count.
2. **#1 reaping** — the gate on modality (c); validate with a real swarm run.
3. Re-test (c) swarms end-to-end.

#3 (version drift) is already done: nix-managed mode now bypasses the `builds/`
shadow entirely, so the running server can no longer pin itself to a stale build.
