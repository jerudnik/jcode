# F06 evidence: explicit MCP child ownership and bounded reap

## Acceptance-gate mapping

| Gate | Implementation | Tests / evidence |
|---|---|---|
| Pooled children receive the owning daemon PID | `crates/jcode-base/src/mcp/client.rs:21-22` defines `JCODE_MCP_OWNER_PID`; `client.rs:65-140` owns the process-wide `{server_name, child_pid, owner_pid}` registry; `client.rs:490-519` overwrites config-provided owner values before spawn and registers the child PID. `crates/jcode-base/src/mcp/pool.rs:41-76` owns the tracker, and `pool.rs:195-214` exposes `owner_pid()` plus `tracked_children()` introspection. `crates/jcode-base/src/mcp/manager.rs:186-218,278-292` routes non-shared manager children through the same daemon tracker. | `crates/jcode-base/src/mcp/pool.rs:505-591` launches a real stdio MCP fixture, deliberately supplies a spoofed owner value, and proves the child environment and tracking record both contain the daemon PID. `process-tree.txt` contains the emitted child/owner PID tuple. |
| Shutdown coordinator disconnects and reaps all tracked children within grace | `crates/jcode-base/src/mcp/client.rs:142-184` implements bounded graceful wait, SIGTERM, SIGKILL, and final tracker clearing; `client.rs:230-290` probes/reaps direct children and signals stragglers. `crates/jcode-base/src/mcp/pool.rs:170-214` separates disconnect from the bounded global reap surface. `crates/jcode-app-core/src/server/shutdown.rs:417-420,1118-1155` reserves 25 ms for disconnect and 175 ms for reap inside the existing 250 ms cleanup-step budget, then verifies the tracker is empty. | `crates/jcode-base/src/mcp/client.rs:788-815` checks the pure fake-PID escalation plan. `client.rs:817-873` spawns a real TERM-resistant child and proves TERM then KILL, no unreaped PID, zero tracked children, and sub-second completion. `crates/jcode-app-core/src/server/shutdown.rs:1402-1408` proves the MCP sub-budgets fit the cleanup step. `process-tree.txt` records `term=[pid]`, `kill=[pid]`, `unreaped=[]`, and `tracked_after=0`. |
| `mcp-serve` cannot outlive a dead owner | `src/cli/mcp_serve.rs:38-126` parses and polls the owner PID, treating permission/unknown probes fail-safe; `mcp_serve.rs:128-185` races stdin service against owner death and exits cleanly when the owner disappears. | `src/cli/mcp_serve.rs:466-472` unit-tests the pure liveness decision for absent, live, unknown, and dead owner states. |

## Validation

Final commands on macOS aarch64:

```text
scripts/dev_cargo.sh test -p jcode-base --lib mcp
41 passed; 0 failed; 1151 filtered out

scripts/dev_cargo.sh test -p jcode-app-core --lib server::
469 passed; 0 failed; 681 filtered out

scripts/dev_cargo.sh test --lib cli::mcp_serve::tests
4 passed; 0 failed; 210 filtered out
```

Focused real-process transcript: `process-tree.txt`.

## Edge cases covered

- MCP config attempts to spoof `JCODE_MCP_OWNER_PID`; daemon ownership wins.
- Cooperative children can exit during the graceful quarter of the budget.
- TERM-resistant children escalate to SIGKILL and are reaped with `waitpid(WNOHANG)`.
- Already-exited/externally-reaped children are removed without signaling stale PIDs.
- Pool reload reaps only the pooled PIDs it disconnected, while daemon shutdown reaps the full process-wide registry, including per-session owned clients.
- Invalid/zero owner PID input is rejected; unknown liveness results keep `mcp-serve` alive rather than causing a false orphan exit.
- Windows owner polling and child termination have platform-specific implementations; non-Unix runtime behavior was compile-reviewed but not executed in this macOS validation.
