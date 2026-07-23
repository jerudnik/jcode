# Sticky server: `server stop --force` reports ESRCH while the process is alive

Reported by human (2026-07-23, screenshot `STICKY_SERVER.png`). Original note:

> was having auth issues with a stale deepseek key / openrouter getting force
> pinned as provider and tried refreshing server, noticed that process didn't
> want to quit (reportedly did not exist), and was stubborn. Eventually quit by
> itself, though.

## Symptoms (from the screenshot)

1. `jcode server reload` → "Server reload skipped: no strictly newer approved
   reload target by mtime." (current-exe was a `dirty` build newer than the
   `selfdev`/`stable` candidates).
2. `jcode server stop --force` (twice) →
   `Failed to signal jcode server (pid 1140): No such process (os error 3)` and
   `jcode server did not exit cleanly; it may still be shutting down.` — while
   Activity Monitor shows **pid 1140 alive** (198.8 MB, 24 threads).

## Root cause (analysis, verified live)

### Symptom 2 is a real bug: process-group vs single-process signal mismatch

`server stop` mixes two granularities of the same PID:

| Step | Code | Target |
|------|------|--------|
| liveness check | `src/cli/commands.rs:2305` `is_process_running(pid)` → `kill(pid, 0)` | the **process** |
| the signal | `src/cli/commands.rs:2310` → `crates/jcode-base/src/platform.rs:260` `kill(-pid, SIGTERM)` | the **process group** (PGID) |
| SIGKILL escalation | `src/cli/commands.rs:2369` `kill(-pid, SIGKILL)` | the **process group** |

`kill(-pid, ...)` only reaches a group whose **PGID == pid** (i.e. `pid` is a
group leader). When the server process is **not** a group leader (PGID != PID),
`kill(-pid)` returns **ESRCH** ("No such process") even though the process is
alive. Both the SIGTERM and the SIGKILL-escalation use `-pid`, so neither can
kill it. The daemon becomes unkillable via `stop --force` and only dies when it
exits on its own — exactly the "eventually quit by itself" observation.

Verified the mechanism live: `kill(-pid)` succeeds iff `ps -o pgid= -p <pid>`
equals `<pid>`. The servers running at analysis time happened to be group
leaders (so stop worked on them), confirming the bug is specific to the
non-leader path that pid 1140 was on.

### Why PGID != PID happens: unchecked `setsid()`

The daemon spawn detaches via `pre_exec` at
`crates/jcode-app-core/src/server/socket.rs:262`:

```rust
libc::setsid();   // return value ignored
```

`setsid()` fails with EPERM when the caller is already a process-group leader.
On that path the child does **not** start a new session, keeps the inherited
process group, and ends up with `pid != pgid`. Nothing detects or records this,
so `stop`'s group-signal later misses it.

### Symptom 1 is NOT a bug

"reload skipped: no strictly newer approved reload target by mtime"
(`crates/jcode-app-core/src/server/util.rs:260`) is the mtime guard correctly
refusing to reload onto an **older** binary. The `dirty` current-exe was newer
than the `selfdev` candidate, so reload declined by design. Working as intended;
unrelated to the auth issue that prompted the refresh.

## Proposed fix (two small, independent changes)

1. **Stop side (fixes the symptom regardless of cause).** In
   `src/cli/commands.rs`, when the group signal returns `ESRCH` but
   `is_process_running(pid)` is still true, fall back to a **single-process**
   `kill(pid, sig)`. Apply to both the initial SIGTERM (`:2310`) and the SIGKILL
   escalation (`:2369`). This makes `stop --force` correct even for daemons that
   never became group leaders.
2. **Spawn side (fixes the cause).** In `socket.rs:262`, stop ignoring the
   `setsid()` result. Options: retry after `fork()` so the child is guaranteed
   not to be a group leader, or fall back to `setpgid(0, 0)` to establish a
   distinct group, or at minimum record when the daemon is not its own leader so
   stop chooses single-process signaling deterministically.

## Test to add

`platform_tests.rs` already has
`signal_detached_process_group_terminates_descendant_tree`. Add a case that
spawns a child that is **already a group leader's non-leader member** (PGID !=
PID), then asserts the stop path still terminates it (exercising the
single-process fallback). This pins the regression.

## Status

Analysis complete and verified. NOT fixed (touching signal + reload paths and
reloading the runtime is a coordinated action; deferred to a dedicated session).
Actionable tracking mirrored to `~/notes/projects/jcode/proposals/`.
