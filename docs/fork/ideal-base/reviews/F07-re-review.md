# F07 re-review after blocker fixes (adversarial, opus-class)

Reviewed commit: 58a806401 (fix round for the FAIL verdict in
F07-implementation-review.md). Verdict: **PASS**.

Both blockers verified fixed at every traced call site; the 46-test mcp
suite (including the new stale-generation fixture) passes.

## Verified

- BLOCKING-1: eviction identity-checked via Arc::ptr_eq on shared
  DeathState; name-based paths (disconnect/reload/shutdown) correctly
  remain name-based; exhaustive trace found no name-only death eviction.
- BLOCKING-2: reconnect_and_retry_once reachable only from provably
  unsent failures (pre-send dead flag, writer-channel send failure);
  probe-failure death, recv-closed, and alive-but-slow timeouts return
  unmarked errors; no delivered request can be re-sent. Busy
  single-threaded servers fail the ping probe and are killed (accepted
  design limit) but never double-executed.
- Important-3 lost-wakeup: enable-then-recheck ordering proven race-free
  against leader-drop interleavings; ptr_eq defeats ABA.
- No new lock-across-await, deadlock, or recursion.

## Minor findings (non-blocking)

1. Double-dead-generation pre-send path fails one call without retry
   (self-healing next call).
2. ref_counts skew after eviction cycles (debug-only consumer; comment
   recommended).
3. Alive-but-slow wall time can reach ~32s (probe time not deducted);
   health-deadline override > 30s starves the remaining-wait to zero.
4. Pre-existing microsecond evict/finish_connect two-phase race
   (self-healing; identical structure predates F07).
5. Waiter may report stale last_errors while a new leader connects
   (spurious one-call failure; pre-existing).

## Gates executed

- git show 58a806401 --stat: 3 files, +402/-62; ancestor of HEAD; no
  later mcp diffs.
- scripts/dev_cargo.sh test -p jcode-base --lib mcp: 46 passed.

## Not checked

Full lib suite (commit message claims 1194 green); runtime reproduction
of the microsecond race; client.rs internals beyond the diffed region;
non-ping-compliant servers (degrade to pre-fix behavior); CI flakiness
margin of 500ms fixture sleeps.
