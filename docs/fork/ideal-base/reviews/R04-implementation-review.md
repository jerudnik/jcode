# R04 implementation review (adversarial, opus-class)

Reviewed commit: c71628498. Verdict: **PASS**.

The classification fix is sound: the phase flips off `Running` under the
state lock before `cancel_intake` in both `begin` and `begin_reload_drain`,
so a drain-induced accept-loop exit always observes `has_begun()`. All three
gates execute green; the genuine-failure path (exit 45) is preserved.

## Blocking findings

None.

## Important findings (non-blocking; follow-up candidates)

1. Beacon clobbering in multi-server setups: one global
   `state/server-beacon.json`, but concurrent daemons (persistent +
   temporary) both write it unconditionally. A temporary server can
   overwrite the persistent daemon's beacon, making the latter's hard crash
   undetectable. Suggest keying by server name or pid.
2. "Positively indicates a hard crash" is overstated: any exit that skips
   the coordinator's Cleaned publish (early startup error, successor
   bind failure) classifies as hard crash. The true signal is "did not exit
   through coordinator cleanup".
3. Wedge window in the AwaitTerminal path: a reload task dying between
   Handoff and exec leaves no executor and no armed watchdog; run() waits
   forever with intake cancelled. Pre-existing, not a regression; a
   watchdog arm at Handoff entry would close it.
4. `beacon_indicates_hard_crash` is production-dead-code; only scripts read
   the JSON. Wiring into `jcode doctor` is a follow-up.

## Minor findings

- `finalize_beacon` read-modify-write not atomic (consistent with existing
  best-effort markers).
- A genuine crash racing just after drain-begin loses its reason from the
  exit record (logged, acceptable).
- No full begin->execute->finalize beacon integration fixture (unit helpers
  only).

## Gates executed

- `scripts/dev_cargo.sh test -p jcode-app-core --lib shutdown`: 42 passed.
- `accept_loop_exit` fixtures: 3 passed. `beacon` tests: 4 passed.
- Traced reload-exec-failed path: `reload_exec_failed()` arms watchdog,
  publishes Cleaned{42} via send_replace; no hang.

## Not checked

Live SIGKILL/reload-under-load; client-task stall during runtime.shutdown;
Windows accept loop; gateway accept loop; sentinel scripts.
