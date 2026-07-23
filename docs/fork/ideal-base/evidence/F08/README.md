# F08 integrated MCP and lifecycle adversarial gate

Verify node. Gate executed via `run_integrated_gate.sh` (in this directory),
which runs three full rounds of the accepted W1 matrices with per-round
residue checks:

1. F03 lease-class runtime matrix (`../F03/lease_class_fixtures.sh`) against
   the current selfdev binary: real daemons, per-class lease hold/release vs
   5s idle timeout, SIGTERM/SIGKILL exit modes, successor boot over stale
   socket, residue assertions (41 checks per round).
2. Shutdown coordinator suite (F03/R04): 42 in-process fixtures including
   the R04 accept-loop-exit classification and beacon tests.
3. MCP lifecycle suite (F06/F07): 46 tests including real-process kill,
   hang, one-bounded-reconnect, crash-loop cooldown, stale-generation
   eviction, and cancelled-leader fixtures.
4. Background status durability suite (F05): 43 tests.

## Result

`integrated_gate_run.log` final line:

    F08 INTEGRATED GATE: PASS (3 rounds, all matrices, no residue)

- 12/12 matrix executions passed across 3 rounds (123 lease-matrix
  assertions + 3x(42+46+43) suite tests).
- `residue_report.txt`: zero orphaned MCP fixture children and zero fresh
  smoke sockets after every lease-matrix and mcp-suite phase (gate 1: zero
  surviving owned descendants in every exit mode).
- Repeatability (gate 2): three consecutive full rounds on one invocation;
  earlier invocations on the same day also completed green rounds.

## Defects found and fixed by this gate (its purpose)

1. `edde05580`: the F07 hung-child fixture set the 500ms health-deadline
   env override BEFORE the MCP handshake; under load the deadline starved
   initialize itself and the test failed at connect. Override now applied
   only after a successful handshake.
2. Gate-harness environment hardening (same commit): background shells
   inherit a stripped PATH and stale IN_NIX_SHELL/DEV_CARGO_NIX_REEXEC
   markers; the gate now pins a full PATH and clears both so
   `scripts/dev_cargo.sh` toolchain recovery works from cron/detached
   contexts.

## Flake note

One transient `[client-connection] daemon exited within 4s of release`
failure was observed while a second, orphaned gate instance was running
concurrently (process contention with 5s idle-timeout daemons). Standalone
and clean-gate reruns pass 41/41 and 3/3 rounds respectively. Recorded as
load sensitivity of the 4s-alive assertion, not a quiescence-epoch bug;
follow-up candidate: serialize gate instances with a lockfile.

## Reproduce

    bash docs/fork/ideal-base/evidence/F08/run_integrated_gate.sh 3
