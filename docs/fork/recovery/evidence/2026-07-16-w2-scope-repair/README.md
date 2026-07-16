# W2 low-friction scope-repair evidence

This directory preserves the failed and successful offline validation attempts
for W2 review HEAD `f8c5f8204056ff783d99769e4088e7bcceb56d73`
against recovery base `602709895be96a85a6090690c0b27d5681d17321`.

## Boundary

The repair removes W2-added spawn metadata from the public
`CommSpawnResponse` and durable mutation replay response. Protocol version
remains `1`. Explicit `Visible` failure, `Auto` fallback, member-detail/history
observability, reclaim history, central liveness, dead-PID recovery, and bounded
churn remain in scope.

No live swarm, terminal, daemon, provider, credential, network, MCP/tool,
reload, publication, release, install, update, or baseline mutation was used.
Every Cargo command set `CARGO_NET_OFFLINE=true` and `CARGO_INCREMENTAL=0`.
Every Nix entry used `nix develop --offline` with
`FORK_NUDGE_MAX_AGE=2147483647` and `FORK_NUDGE_AUTOSYNC=0`.

## Files

- `failed-status-event-attempt.log.gz`: the first fixture attempt, which added an
  automatic `SwarmStatus` delivery expectation beyond the bounded low-friction
  contract. It timed out and is preserved as a failure.
- `authoritative-offline-rerun.log.gz`: the corrected 14-check rerun. Thirteen
  focused fixture commands and the three-package check exited `0`.
- `scope-state.txt`: fixed review head, protocol version, empty forbidden-field
  search, cumulative changed paths, and tracked-tree status before this evidence
  directory is committed.
- `MANIFEST.sha256`: SHA-256 hashes for the evidence members.

The successful fixture counts are also appended to the R05B ledger. A bounded
secret-pattern scan found no credential-like transcript content. Independent
review artifacts are preserved separately under `docs/fork/recovery/reviews/`
when complete.
