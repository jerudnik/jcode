# F07 implementation

Implemented in two phases by delegated workers (split to fit the TUI
stream-stall guard budget; see R04 IMPLEMENTATION.md open questions):

- Phase 1 at `5c9ac8b42`: dead/hung detection + eviction.
- Phase 2 at `d3fcba412`: one bounded reconnect + died-cooldown +
  cancellation-leak guard.

## Changes

Phase 1 (`crates/jcode-base/src/mcp/{client,pool,manager}.rs`):

- `DeathState` on `McpHandle` (shared AtomicBool + OnceLock reason): all
  clones, including session-cached ones, observe death.
- Reader EOF / writer failure -> `mark_dead_and_fail_pending`: fails all
  pending oneshots immediately (previously they leaked and every request to
  a dead child burned the full 30s timeout).
- `request()` checks the dead flag pre-send and enforces a health deadline
  (`JCODE_MCP_HEALTH_DEADLINE_MS`, default 15000, capped at the 30s total)
  that marks the handle dead with "health deadline exceeded".
- `SharedMcpPool::call_tool` and both `McpManager::call_tool` fast paths
  evict dead handles (handles + clients + ref_counts) and reap via the F06
  tracker with no lock held across the await.
- No `kill(pid,0)` probing anywhere (avoids the F06-review PID-reuse trap).

Phase 2:

- Pool path: after dead-handle eviction, `reconnect_and_retry_once` ->
  `ensure_connected` (existing dedupe) using the pool's current config, then
  one retry of the failed call. Never `pool.reload()`. Calls that succeeded
  before death are not replayed.
- Crash-loop bound: `record_died_cooldown` reuses `last_errors` with the 30s
  cooldown; while active, calls fail fast with no child spawn.
- Manager: same one-reconnect-then-cooldown on pooled and owned fast paths
  via a `DIED_RETRY_COOLDOWN` map; connect-on-first-call unified through
  `call_fresh_handle_once`, gaining eviction+cooldown it previously lacked.
- Cancellation leak fixed structurally: `connecting` entries are held by an
  RAII `ConnectingEntryGuard` (ptr-eq checked) whose Drop removes the entry
  and `notify_waiters()`, so a cancelled leader can no longer strand waiters
  forever.

## Acceptance gates

1. Killed child detected before request timeout: real-process fixture kills
   the fake server with SIGKILL; call fails well under 5s (sub-second
   locally) with the death error.
2. Hung child detected by health deadline: fake server reads but never
   replies; failure at the (test-shortened) deadline.
3. Exactly one reconnect succeeds without daemon reload: kill-then-reconnect
   fixture succeeds on the retried call; crash-loop fixture proves the
   cooldown suppresses further spawns (spawn-counter file); cancellation
   fixture proves a cancelled leader connect no longer hangs successors.

## Validation

- `scripts/dev_cargo.sh test -p jcode-base --lib mcp`: 45 passed, 0 failed
  (41 before F07).
- `scripts/dev_cargo.sh test -p jcode-base --lib`: 1193 passed, 0 failed,
  3 ignored.
- fmt clean; clippy no new warnings from production code.

## Known gaps (for the reviewer)

- The pooled-shared-server kill+reconnect path is covered indirectly (its
  pieces are tested; the manager path is tested end-to-end). A dedicated
  pooled integration test is a review-round candidate.
- No concurrent-callers-racing-reconnect test (dedupe inherited from the
  tested `begin_connect`).
- Unix-only fixtures; Windows compile-only.
- Manager died-cooldown is per-session; pool cooldown is daemon-global.

## Fix round (review blockers)

First review FAILED (reviews/F07-implementation-review.md): BLOCKING-1
stale-generation eviction could kill healthy replacement children;
BLOCKING-2 health-deadline retry double-executed slow tools. Fixed at
`58a806401`: identity-checked eviction (Arc::ptr_eq on shared DeathState)
with healthy-handle re-fetch, RequestNotDelivered delivery gating on all
auto-retries, ping-probe liveness before hung declaration, and the
ensure_connected lost-wakeup guard. New fixture
`stale_dead_clone_does_not_evict_healthy_replacement`. mcp suite 46
passed; full lib 1194 passed. Re-review PASS (reviews/F07-re-review.md)
with 5 minor non-blocking findings recorded.
