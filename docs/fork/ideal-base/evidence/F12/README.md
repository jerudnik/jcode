# F12 implementation: global caps and observability

Commits: 814a6f707 (base), 0a93900bc (review round 1 fixes), a1c9075af
(review round 2 fixes). Review cycle: FAIL -> FAIL -> PASS
(reviews/F12-implementation-review.md).

## What landed

- Pooled MCP children cap (JCODE_MCP_MAX_CHILDREN, default 32): enforced
  in the ensure_connected leader path with slot reservation via the
  connecting map and race-safe connecting-before-clients counting. Only
  new spawns gated; reuse never refused. Refusal is explicit, names
  cap/count/env var, is visible to concurrent waiters, and never
  triggers the 30s failure cooldown (pre-expired last_errors record).
- Owned-children cap made env-configurable (JCODE_MCP_MAX_OWNED_CHILDREN,
  default 64) on the existing OwnedChildPermit.
- Background live-task cap (JCODE_MAX_BACKGROUND_TASKS, default 64):
  RAII SpawnSlot reservation from before the check until live-map
  insert; refusal writes a terminal Failed status file AND sets
  BackgroundTaskInfo.refused, which bash and swarm run_plan surface as
  explicit tool errors (build_queue logs; its status-file follow-up
  observes the terminal refusal). write_initial persistence failure also
  marks refused. Terminal pruning and F07 eviction release capacity.
- Observability: capacity_snapshot() on pool and background manager
  ({current, cap}).

## Gates

1. Low test caps bound counts: cap-1 fixtures for both subsystems.
2. Refusal explicit: error strings name cap, count, and env var;
   surfaced at the tool layer.
3. Capacity releases after terminal cleanup: disconnect/evict (pool) and
   prune/cancel (background) tests.

## Validation

mcp 48; background 44; tool::bash 21 (x3); selfdev 36; communicate 89;
jcode-base full lib 1197 at base commit.

## Follow-ups (non-blocking, from round 3)

Background cancellation window between tokio::spawn and live-map insert
(transient overshoot, pre-existing); finish_connect handle/client gap
under cancellation (pre-existing); config-file surface for the caps
(env-only v1).
