# F03 evidence: lease-class and exit-mode verification matrix

Recorded: 2026-07-18. Verification of F02 (accepted at `2b5607882`, D014)
against the F01 revision-4 test plan (design 4.3).

## Test surfaces added (F03 owned paths)

- `crates/jcode-app-core/src/server/shutdown_fixture_tests.rs`: 12
  in-process state-machine fixtures driving REAL `ShutdownCoordinator`
  instances (private per-test authority, watchdog disabled in-process):
  begin_and_wait terminal outcomes; idle-claim refusal while leased and
  non-closing refused claims; a 50-iteration concurrent
  idle-claim-vs-acquisition race proving accepted claims imply an empty
  table and closed acquisition; drain-until-release; deadline abandonment;
  upgrade lattice (supersede/upgrade/deadline-shortening); all 16 ordered
  pairwise begin races; reload refusal (temporary + during termination);
  reload mid-drain SIGTERM upgrade handoff; reload drain to Handoff;
  exec-failure re-entry with code 42; 16 concurrent waiters observing one
  outcome.
- Debug socket surface (session-independent): `shutdown:state`,
  `shutdown:hold_lease:<class>`, `shutdown:release_lease:<token>`.
- Test injection: `JCODE_TEST_SHUTDOWN_CLEANUP_HANG_MS` for the forced-exit
  fixture.
- Coordinator refactor: instance-scoped lease authority (the global
  coordinator wires the global authority; tests construct private pairs).

## Defect found and fixed by these fixtures

- `terminal_tx.send` (tokio `watch`) DROPS the value when no receiver is
  subscribed; a fast executor could publish `Cleaned` before any
  `wait_terminal` subscriber existed, hanging `begin_and_wait` callers
  forever. Fixed to `send_replace` (caught by the pairwise race fixture).

## Runtime fixture matrix (`lease_class_fixtures.sh`)

Isolated `env -i` environments per fixture (private JCODE_RUNTIME_DIR +
JCODE_HOME, deferred-auth bootstrap, JCODE_DEBUG_CONTROL=1). Transcript:
`lease_class_fixtures_run.log`.

| Fixture | Result |
|---|---|
| A. Hold/release per lease class (all 8 classes) | alive past 5s idle timeout while held; STILL alive 4s after release (full-new-window assertion, review F03-I1); exit 44; zero residue |
| B. Forced exit (30s injected cleanup hang + SIGTERM) | watchdog exit code 70; durable marker `fired`; successor boots over the forced-exit residue IN THE SAME runtime dir, idle-exits 44 with zero residue (review F03-I2) |
| C. Parent SIGKILL | stale socket residue as expected; successor boots over it, idle-exits 44, zero residue |
| D. Drain refusal typing | covered by unit fixtures (idle-claim atomicity, ShuttingDown refusals) |

## Acceptance gates

- "Short-timeout no-provider fixtures remain alive for each lease class and
  exit after release": matrix row A, all 8 classes PASS.
- "No socket/process residue": residue checks in rows A and C PASS; the
  forced-exit path's residue is intentionally owned by next-boot
  reconciliation (proven by row C's successor boot).

## Test totals

- `jcode-app-core --lib`: 1126/1126 (includes the 12 new fixtures).
- Runtime matrix: 41/41 assertions PASS (strengthened per review findings
  F03-I1 and F03-I2 after the independent PASS verdict at `d8c223d29`).
