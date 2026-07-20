# W1 synthesis: runtime ownership and persistence closed

All ten W1 children accepted:

| Node | What closed | Evidence |
|---|---|---|
| F01 | Activity/shutdown architecture critique + design | evidence/F01 |
| F02 | Shutdown coordinator implementation (lease lattice, drains, watchdog) | evidence/F02 |
| F03 | Lease classes verified: fixtures + runtime matrix (41 assertions) | evidence/F03 |
| F04 | Background task manager activity integration | evidence/F04 |
| F05 | Durable TaskStatusStore (atomic fsync writes, cross-instance mutexes) | evidence/F05 |
| F06 | MCP child ownership: owner-pid contract, tracker, bounded reap | evidence/F06 |
| F07 | MCP dead/hung detection, identity-checked eviction, one bounded reconnect, cooldown | evidence/F07 |
| F08 | Integrated adversarial gate: 3 rounds, all matrices, zero residue | evidence/F08 |
| R01 | Reload/build subsystem repair (atomic publish, target decisions, alias binding) | evidence/R01 |
| R03 | TUI terminal-mode ownership across exec handoff | evidence/R03 |

Adjacent: R04 (reload drain vs accept-loop-exit race, discovered live during
this wave's reload) accepted as a W2 child at c71628498 with the startup
beacon; its fix is running on the current daemon.

Every implement child passed independent adversarial review (two required a
FAIL->fix->re-review cycle: R01, F07). F08's integrated gate is repeatable
via evidence/F08/run_integrated_gate.sh.
