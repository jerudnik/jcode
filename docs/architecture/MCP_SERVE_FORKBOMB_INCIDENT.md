# MCP Serve Forkbomb Incident

See also:

- [`MCP_SERVER_REGISTRATION_GUARDRAILS.md`](./MCP_SERVER_REGISTRATION_GUARDRAILS.md)
- [`../SERVER_LIFECYCLE_INVARIANTS.md`](../SERVER_LIFECYCLE_INVARIANTS.md)
- [`../fork/SYNC_MODEL.md`](../fork/SYNC_MODEL.md)

## Summary

A self-referential MCP entry in `~/.jcode/mcp.json` made the daemon recursively
spawn `jcode mcp-serve` children:

```json
{
  "jcode": {
    "command": ".../jcode",
    "args": ["mcp-serve", "..."],
    "shared": false
  }
}
```

That entry turns the daemon into its own MCP client. Each daemon session loads
the config, starts another `jcode mcp-serve`, and causes that shim to create
another daemon session. The result is an unbounded session/process fork bomb
that pins about 600% CPU.

The incident is compounded by a supervision gap: the orphaned self-dev daemon
does not self-exit, and SIGTERM hangs. SIGKILL is the only reliable stop signal
once the runtime is saturated.

## Root cause chain

The recursion chain is:

1. A daemon session registers MCP tools through `register_mcp_tools_for_dir`
   (`crates/jcode-app-core/src/tool/mod.rs`).
2. The MCP config entry has `shared:false`, so `McpManager::connect_all`
   (`crates/jcode-base/src/mcp/manager.rs`) routes it to the per-session owned
   path instead of the shared pool.
3. The owned path spawns `jcode mcp-serve` through `McpClient::connect`
   (`crates/jcode-base/src/mcp/client.rs`).
4. The MCP client warms the server by calling `tools/list`.
5. `jcode mcp-serve` handles `tools/list` by calling `ensure_session`
   (`src/cli/mcp_serve.rs`).
6. `ensure_session` sends `create_session` to the daemon.
7. `create_session` creates a new headless session in the daemon
   (`crates/jcode-app-core/src/server/headless.rs`).
8. The new headless session registers MCP tools again.
9. That registration reloads the same self-referential MCP entry, starts another
   owned `jcode mcp-serve`, and repeats.

`shared:false` defeats the shared-pool deduplication that would otherwise bound
the configured server to one process per name. It makes the shim a per-session
owned process, so every recursive session gets its own child.

## Why it never stopped

`JCODE_DEBUG_CONTROL` is auto-set for self-dev, meaning any `jcode` run inside
the repository runs with debug control enabled. The regression is that debug
control disables the idle-timeout monitor entirely, so the orphaned persistent
self-dev daemon never reaches the normal no-clients safety exit.

Owner-death checks are wired only for temporary servers. The persistent shared
server is detached with `setsid()`, so it does not have a safe owner PID signal
to watch.

The SIGTERM handler awaits registry cleanup without a bounded timeout. Under
runtime saturation, that unbounded registry I/O can sit on a starvable Tokio
task and prevent the process from exiting.

## CPU driver

CPU burn comes from two effects compounding:

- unbounded session growth from the recursive `mcp-serve` creation path
- an O(N) `Session::load` per member inside `sweep_dead_pid_swarm_members`
  (`crates/jcode-app-core/src/server/swarm.rs`), fired from swarm-status
  broadcasts

As the member set grows, each status broadcast can trigger more disk-backed
session loads, which amplifies the runaway process/session count.

## Fixes shipped

The shipped guardrails are documented in
[`MCP_SERVER_REGISTRATION_GUARDRAILS.md`](./MCP_SERVER_REGISTRATION_GUARDRAILS.md)
and the lifecycle invariants are documented in
[`../SERVER_LIFECYCLE_INVARIANTS.md`](../SERVER_LIFECYCLE_INVARIANTS.md).

The fixes are:

- a self-reference guard that drops `jcode mcp-serve` shims at config load
- an owned-MCP child process cap
- a `mcp-serve`/headless session cap
- a tester process cap plus tester spawn-depth guard
- an always-on idle monitor decoupled from debug control
- bounded shutdown registry cleanup plus a SIGTERM watchdog
- a dead-PID swarm sweep that skips terminal members before touching disk

## Blast radius / reachability

The failure is reachable by default by running `jcode` inside the repository
while the self-referential `~/.jcode/mcp.json` entry is present. No special
debug command or explicit stress path is required.
