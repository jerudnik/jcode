# F07 implementation review (adversarial, opus-class)

Reviewed commits: 5c9ac8b42, d3fcba412. Verdict: **FAIL**.

## Blocking findings

1. Stale-generation eviction kills healthy children. `evict_dead_server`
   (pool.rs) and `evict_dead_pool_handle` (manager.rs) evict by NAME with no
   identity check against the dead handle. Session B holding a stale dead
   clone can evict-and-kill the healthy replacement child session A just
   reconnected, repeatedly (N stale sessions = N kills). Fix: compare
   handle identity (e.g. Arc<DeathState> ptr-eq) with the pool's current
   handle before evicting.
2. Health-deadline retry causes double side effects on slow tools. Any call
   exceeding the deadline (default 15s; tools previously had 30s) marks the
   child dead, eviction kills a child possibly still EXECUTING the call,
   and reconnect_and_retry_once re-sends the same tools/call. Legitimate
   20s tools are killed mid-execution and executed twice. The deadline must
   not fire while a request is legitimately executing, or retry must be
   gated on the request provably not having been delivered.

## Important findings

3. Lost-wakeup race in ensure_connected waiters: notify_waiters only wakes
   registered waiters; a fast leader can drop the guard before the waiter
   polls notified(), hanging it forever (no timeout wraps the pooled
   reconnect). Create the Notified future under the map lock or enable()
   it before release.
4. Explicit `connect_server` of a legitimately restarted server is refused
   for up to 30s by the died-cooldown with no bypass.
5. Per-session manager cooldown vs daemon-global pool cooldown
   inconsistency (bounded, cosmetic).

## Minor findings

- health_deadline() reads env per request.
- ~150 lines of duplicated reconnect/retry logic between pool and manager.
- Cooldown maps never pruned (bounded by server names; fine).
- Lock discipline clean; F06 flag addressed.

## Gates executed

- `scripts/dev_cargo.sh test -p jcode-base --lib mcp`: 45 passed. F07
  fixtures present and passing (kill+reconnect, hung deadline, crash-loop
  cooldown with spawn counter, cancelled-leader guard).
- Gates run only on the OWNED manager path; the pooled shared path (where
  finding 1 lives) has no end-to-end gate.

## Not checked

Windows paths; full lib suite (trusted evidence); live multi-session
reproduction of finding 1; tokio shutdown mid-reap.
