# W2 synthesis: recovery reconciliation and global resource bounds closed

All seven W2 children accepted:

| Node | What closed | Evidence |
|---|---|---|
| R04 | Reload drain vs accept-loop-exit race + startup beacon | evidence/R04 |
| F09 | Selfdev pending-activation reconciliation (liveness + identity + rollback) | evidence/F09 |
| F10 | Durable disconnect-cleanup intent, reason-aware startup reconcile, bounded retry | evidence/F10 |
| F11 | Independent verification: 18-cell combined recovery matrix, 4 standalone probes | evidence/F11 |
| F12 | Race-safe global caps (pooled MCP children, background tasks) with explicit refusals | evidence/F12 |
| F13 | Cap verification under concurrency/cancellation/restart (16-way burst probe) | evidence/F13 |
| F14 | Rerunnable no-provider lifecycle matrix, 2 clean rounds of all W1+W2 invariants | evidence/F14 |

Review cycles: F12 took FAIL->FAIL->PASS (TOCTOU then torn-read ordering);
F09/F10/F14 first-round PASS with recorded follow-ups; F11/F13 verify
nodes PASSed with independent probes beyond the implementer's tests.

Cross-cutting follow-ups carried forward (recorded in reviews):
manifest file lock for cross-process reconcile races (F09), tri-state
pid-marker liveness (F09/F10), network-denied demonstration and live
exec-handoff reload in the matrix (F14), background spawn/insert
cancellation window (F12).
