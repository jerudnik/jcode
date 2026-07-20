# F14: no-provider real-process lifecycle matrix

Verify node. Harness: `scripts/run_lifecycle_matrix.sh` (rerunnable;
`bash scripts/run_lifecycle_matrix.sh 2`).

## Coverage mapping (gate 1)

| Invariant | Mechanism | Source |
|---|---|---|
| cancel | turn cancel/interrupt suite (R12 scripted fixtures, mid-stream + pre-open cancel) | agent:: suite |
| exit | per-lease-class idle exits, SIGTERM drain (exit 0), exit-44 idle windows | F03 matrix (real daemons) |
| reload | shutdown coordinator incl. R04 accept-loop-exit classification; R01 reload state/recovery suites | app-core suites |
| crash | SIGKILL daemon, no cleanup runs | F03 matrix |
| restart | successor boots over stale socket, reaps residue | F03 matrix |
| recovery | orphaned background tasks finalized (F04/F05); disconnect-cleanup startup sweep (F10); pending-activation reconcile (F09) | jcode-base + app-core + build-support suites |
| residue | zero orphaned fixture children after each round; per-fixture socket/hash/metadata checks inside the F03 matrix | pgrep + matrix assertions |

MCP child lifecycle (kill/hang/reconnect/cooldown/caps, F06/F07/F12) and
background caps (F12) run in both rounds via their real-process fixtures.

## Gate 2: no provider/network dependency

The harness strips ANTHROPIC/OPENAI/GEMINI/OPENROUTER/GROQ keys to empty
strings before every step, so any accidental provider call fails loudly.
All fixtures use real local daemons, scripted in-process providers, and
fake shell MCP servers. Both rounds passed with keys stripped.

## Result

`lifecycle_matrix_run.log`: 2 rounds, 18/18 steps PASS, final line
`F14 LIFECYCLE MATRIX: PASS (2 rounds)`. Runtime ~15.5 min for both
rounds on this machine.
