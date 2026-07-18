# W0.3 evidence: graph seeding and ownership/dependency projection

Recorded: 2026-07-18T06:32Z at HEAD `468dc3daa`.

## What happened

1. The persisted swarm plan for `/Users/jrudnik/labs/jcode/.git` still held the
   completed historical recovery program (94 nodes, 73 completed). Seeding
   W0.2 merged into it and `run_plan` resurrected stale queued nodes twice
   (five workers first, then four more). All stray workers were stopped with
   `swarm stop force=true`; one partial diff was preserved as a stash (D007).
2. Plan snapshots preserved here:
   - `pre_reseed_plan_snapshot.json` (before the first stop wave, version 238)
   - `final_stale_plan_snapshot.json` (immediately before clearing, version 242)
3. The stale plan was cleared with `swarm:clear_plan` (94 items, version 242).
4. W0.2 itself completed inside the stale plan: its census files were
   committed at `fb00ab840` and accepted by the coordinator after independent
   spot-checks (see STATE.json W0.2 summary).

## Ownership and dependency projection

Projected from `WORK_GRAPH.json` with the W0 wave accepted:

- Runnable next: root `W1`, initial child `F01` (design; no dependencies).
- F02, F04, F06 unlock on F01 -> F02; F03/F05/F07 follow their implement
  parents; F08 joins F03+F05+F07.
- W2 (F09, F10 first) unlocks only after W1 closes; W3 after W2; W4 after W3;
  W5 last. Gated G01-G05 sit under W5 pending authorization dispositions.
- No two implementation nodes own the same path in the same wave after the
  D008 amendments (F02 and F04 both name `crates/jcode-base/src/background.rs`;
  they are serialized by the F01->F02->F04 dependency chain, and F04 is the
  sole background.rs owner while active; F02's touch is limited to
  lease/shutdown wiring and must land before F04 starts).

## Seeding discipline going forward

The live deep task graph is seeded fresh per wave from
`ideal_base_railway.py next --json`, one wave at a time, into the now-clean
plan. `STATE.json` remains the durable restart authority.
