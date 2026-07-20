# F14 verification review (adversarial, opus-class)

Reviewed evidence at e47efacdb. Verdict: **PASS**. All seven invariants
genuinely exercised (real SIGKILL + successor boot verified in the F03
fixture source; real mid-stream/pre-open cancel tests; recovery via
F04/F05/F09/F10 sweeps); log and checksums intact; fresh spot-run
reproduces (client_disconnect_cleanup 16/16).

## Important findings

1. Gate 2 (no provider/network dependency) satisfied by construction,
   not demonstration: empty env keys + env -i private-HOME fixtures +
   in-process scripted providers make network use structurally
   unreachable, but no run was performed with networking disabled.
   Follow-up: network-denied sandbox run or outbound-socket assertion.
2. Reload is the weakest mapping: in-process coordinator/state-machine
   suites only, no real exec/handoff reload with a live attached client
   in this harness. (Live reloads of the actual daemon have occurred
   during the epic, including one exercising the R04 fix, but are not
   part of the rerunnable matrix.)

## Minor findings

run_step logs exit 0 on failure lines (cosmetic); residue pgrep list is
hardcoded; compound transitions (crash during reload, reload under live
load, swarm-worker recovery, multi-daemon successor contention) are
documented gaps not required by the node contract.

## Gates executed

Source cross-check of all seven mappings; log audit (2 rounds, 18 PASS,
0 FAIL, 14 embedded suite results all nonzero-passed); shasum -c OK;
fresh spot-run 16/16.

## Not checked

Full matrix rerun; F03 fixture script rerun; network-down demonstration;
pgrep race with exiting fixture children.
