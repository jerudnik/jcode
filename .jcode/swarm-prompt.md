## Routing

- Human model choices win. `swarm list_models` is the authority: pass names
  exactly as printed (fail-closed). Never guess.
- Never use an OpenAI model below GPT-5.6. Prefer `gpt-5.6-sol` at high effort
  for capable/general work and `gpt-5.6-luna` at high for trivial or mechanical
  work. Prefer fast variants.
- `claude-fable-5` is not a default worker. Use it only by human request, as the
  human-facing orchestrator, or when an explicit swarm/workflow condition names
  it.
- Use `grok-4.6` at xhigh as the Sol fallback. `k3` at xhigh is a second
  choice for Opus/Fable-class depth; it drops long sessions
  (docs/issues/kimi-stream-transport-failures.md). Prefer `glm-5.3` first.
- Spread simple or mechanical work across `deepseek-v4-pro`, `MiniMax-M3`,
  and `glm-5.2`.
- One `run_plan` applies one model/effort to all workers it creates. Keep such
  runs profile-homogeneous; otherwise use explicit spawns.

## Execution

- Every spawn needs a concrete prompt, short `label`, and `subagent_type`. Give
  one manager ownership and synthesis when a worker needs more than three children.
- Concurrency is a ceiling. Fan out disjoint outputs; use light mode for flat
  work and deep mode only for justified recursive discovery and critique.
- Deep nodes use the smallest sufficient child set, two to six; do not split a
  cohesive leaf to fill slots.
- Prefer `implement -> verify`; make failures focused fix/re-verification paths.
  Serialize overlapping mutations or use worktrees.
- Do not reserve a watchdog. Use heartbeats, churn guards, checkpoints, and
  `await_members`.
- A dead reviewer with no verdict is a failed review: await with a timeout
  and respawn on another route.
- Observe subtrees through their owner and artifacts. Prefer bounded commits and
  typed artifacts over polling. On resume, replace stale work and clean up owned
  workers after reports land.
