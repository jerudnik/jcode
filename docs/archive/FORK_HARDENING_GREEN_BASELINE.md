# GREEN ground-truth baseline (captured 2026-07-08, HEAD 75b20b215)

Reproduction commands are inline. Vendor ref: `github/vendor/upstream`
(= `631935dd1` at capture). All groups reference these numbers.

## Divergence surface (was the model's core measurement)

| Metric | Model claim (2026-06-27) | Ground truth (2026-07-08) | Delta |
|---|---|---|---|
| feature commits since vendor | "30 mostly-additive" | 620 commits since 2026-06-29 alone | **~20x** |
| total files touched vs vendor | 107 | 500 | ~4.7x |
| new files (cannot conflict) | 47 | 311 | grew |
| modified files (edits) | 60 | 186 | ~3x |
| deleted files | (n/a) | 3 | new |
| source files deleting >5 upstream lines | **7** | **~13 (.rs, >5 del)** | **~2x** |

`git diff --numstat <vendor> HEAD -- '*.rs' | awk '$2>5'` top offenders:
- provider-anthropic/lib.rs: 135 del / 275 add  (was 58 del in model)
- server/client_comm_message.rs: 40 / 51
- base/provider/tests/model_resolution.rs: 36 / 42
- server/comm_session.rs: 25 / 220
- server/swarm.rs: 21 / 435
- tool/bash_tests.rs: 20 / 39
- cli/acp.rs: 19 / 633
- agent.rs: 19 / 118
- server/comm_await.rs: 16 / 248
- tool/communicate.rs: 16 / 217
- terminal-launch/lib.rs: 15 / 79
- swarm_persistence.rs: 15 / 189
- base/skill.rs: 12 / 40  (model said this was converted to 0-deletion additive!)

Note: `skill.rs` shows 12 deletions again — the model's headline "skill.rs
converted to additive loop (20->0)" appears to have regressed or been re-touched.

## Rerere cache: 100 recorded resolutions (`.rerere-cache/`). Healthy/growing.

## Patch-ledger: 85 lines, last touched 2026-07-04 (`4dd64d650`). 4 days stale
relative to 620 commits of activity.

## Fork gate: `scripts/fork-touched-clippy.sh --fmt` exits 0 (186 fork-touched
.rs, fmt clean, in sync with github/main).
