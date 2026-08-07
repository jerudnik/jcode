## Routing

- Run `swarm list_models` before pinning a route. Use an exact listed model; if
  unavailable or ambiguous, omit `model` and inherit. User choices win.
- One `run_plan` applies one `model` and `effort` to all workers it creates.
  Keep specialized runs profile-homogeneous; otherwise use a general route.
- Defaults: Fable 5 medium for design, investigation, debugging, review, and
  verification; GPT-5.5 low for implementation and focused tests; GPT-5.5 with
  no reasoning for bulk retrieval; coordinator synthesis, or Fable 5 medium for
  delegated synthesis. Reserve high effort for genuinely risky work.
- Add another provider only for an independent opinion, rate limiting, or route
  failure.

## Structure and ownership

- Every spawn needs a concrete prompt, short `label`, and `subagent_type`. When
  a worker needs more than three children, give one manager that subtree and its
  synthesis.
- Concurrency is a ceiling, not a target. Fan out distinct outputs with disjoint
  scopes. Use light mode for flat fan-out and deep mode only when recursive
  discovery, critique gates, and typed artifacts justify it.
- In deep mode, use the smallest sufficient child set, normally two to six. Do
  not split a cohesive reviewed leaf merely to keep slots busy.
- Prefer `implement -> verify`. Turn failures into focused fix and re-verification
  paths. Serialize overlapping mutations or use worktrees.
- Do not reserve a watchdog. Heartbeats, churn guards, checkpoints, and
  `await_members` cover normal monitoring.

## Monitoring and recovery

- Observe sub-orchestrated work through its owner and artifacts; direct
  cross-swarm status calls fail by design.
- Prefer bounded commits and typed artifacts over polling or frequent DMs.
- On resume, compare the checkpoint with the repository and goal. Replace stale
  queued work and clean up owned workers after reports land.
