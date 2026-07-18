# F01-R evidence: design-repair inputs and plan snapshot

Recorded: 2026-07-18, during recovery of the interrupted coordinator session
(`session_fish_1784354921760`). See DECISIONS.md D012.

Contents:

- `worker-artifacts/`: seven typed `complete_node` artifacts recovered from
  the swarm worker session journals. Each was produced read-only against the
  tree at `398b51c07` (or `93c59a218` for the earliest) by the OpenAI-routed
  workers mandated by D009/D009a after Anthropic exhaustion:
  - `b1.json` — blocking finding B1 sustained (worker turkey, gpt-5.5)
  - `i1.json` — important finding I1 sustained (worker parrot, gpt-5.5)
  - `i2.json` — important finding I2 sustained (worker jaguar, gpt-5.6-terra)
  - `F01-R-watchdog-review-lines.json` — exact review-line map
    (worker koala, gpt-5.6-sol)
  - `F01-R-source-seam.json` — lower-crate inversion seam derivation
    (worker shark, gpt-5.5)
  - `F01-R-entry-families.json` — provider-turn/MCP entry family enumeration
    (worker panda, gpt-5.5)
  - `F01-R-reloadhandoff.json` — ReloadHandoff removal recommendation
    (worker penguin, gpt-5.5)
- `pre_clear_plan_snapshot.json`: the full 148-item swarm plan
  (version 64) captured immediately before the over-decomposed F01-R
  analysis plan was cleared (D012). 4 completed, 1 failed (b2, GLM worker
  crash), 143 queued.

The b2/b3/i3-i5/gate analyses that never ran are subsumed: the design
revision in `../F01/design.md` responds to every review finding directly, and
the independent F01-V re-review re-validates all of them against source.
