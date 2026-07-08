# Server Lifecycle Invariants

See also:

- [`architecture/MCP_SERVE_FORKBOMB_INCIDENT.md`](./architecture/MCP_SERVE_FORKBOMB_INCIDENT.md)
- [`architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md`](./architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md)
- [`fork/SYNC_MODEL.md`](./fork/SYNC_MODEL.md)

This is the supervision and lifecycle contract for the persistent shared daemon
and temporary server variants.

## Invariant 1 — the safety exit always runs

The persistent shared server always installs an idle-timeout monitor. With no
connected clients for 300 seconds, the daemon unregisters itself and exits.

`JCODE_DEBUG_CONTROL` and self-dev mode MUST NEVER disable this monitor. The
forkbomb incident happens because that regression lets an orphaned self-dev
daemon spin forever.

The relevant implementation is:

- `crates/jcode-app-core/src/server.rs`
- `crates/jcode-app-core/src/server/lifecycle.rs`
- `spawn_persistent_lifecycle_monitor`

## Invariant 2 — bounded, non-starving shutdown

Every shutdown or exit path uses `unregister_server_bounded`, which applies a
2-second timeout to registry cleanup.

SIGTERM also starts a 3-second OS-thread watchdog before cleanup. The watchdog
is scheduled outside Tokio, so process exit is still guaranteed even when the
async runtime is saturated.

There must be no unbounded await between receiving a shutdown signal and
process exit.

The relevant implementation is:

- `crates/jcode-base/src/registry.rs`
- `crates/jcode-app-core/src/server.rs`
- `unregister_server_bounded`

## Note on orphan detection

The daemon is spawned by the interactive client via a single `setsid()`. There
is no separate launcher process kept alive as the daemon's parent.

That means `getppid() == 1` becomes true as soon as the user quits normally.
Raw `getppid()` is therefore not a safe "abandoned daemon" signal: using it
would collapse the warm-reconnect window for every normal session.

Faster abandoned-daemon cleanup than the 300-second idle timeout needs a real
spawner heartbeat. That mechanism is deferred.

## Caps table

| Cap | Layer | File | Purpose |
|---|---|---|---|
| `MAX_OWNED_MCP_CHILDREN = 64` | Daemon/process | `crates/jcode-base/src/mcp/client.rs` | Caps per-session owned MCP children process-wide. |
| `MAX_TESTERS = 8` | Daemon/process | `crates/jcode-app-core/src/server/debug_testers.rs` | Caps live tester daemons spawned by one daemon. |
| `MAX_TESTER_DEPTH = 1` / `JCODE_TESTER_DEPTH` | Daemon/process | `crates/jcode-app-core/src/server/debug_testers.rs` | Prevents testers from spawning further testers. |
| `MAX_TOTAL_SESSIONS = 1500` | Daemon/process | `crates/jcode-app-core/src/server/headless.rs` | Backstops runaway headless session creation. |
| `MAX_SWARM_MEMBERS = 1000` | Swarm membership | `crates/jcode-swarm-core/src/lib.rs` | Caps live members inside one swarm. |

`MAX_SWARM_MEMBERS` belongs to swarm membership, not daemon/process lifecycle.
Do not treat it as a replacement for the daemon-level process and session caps.
