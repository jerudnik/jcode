## Routing

- Human model choices win. Run `swarm list_models` and use an exact live route.
  On failure use the fallback below; never guess or inherit a policy violation.
- Never use an OpenAI model below GPT-5.6. Prefer `gpt-5.6-sol` at high effort
  for capable/general work and `gpt-5.6-luna` at high for trivial or mechanical
  work. Prefer fast variants.
- `claude-fable-5` is not a default worker. Use it only by human request, as the
  human-facing orchestrator, or when an explicit swarm/workflow condition names
  it.
- Use `xai/grok-4.5` at xhigh as the Sol fallback. Use `k3` (Kimi K3) at xhigh
  where Opus or Fable would otherwise be used; prefer fast routes when applicable.
- Rotate suitable independent/bulk lanes across `deepseek-v4-pro`, `MiniMax-M3`,
  and `glm-5.2` to keep rotations fresh and spread provider load without lowering
  task capability.
- One `run_plan` applies one model/effort to all workers it creates. Keep such
  runs profile-homogeneous; otherwise use explicit spawns.

## Execution

- Every spawn needs a concrete prompt, short `label`, and `subagent_type`. Give
  one manager ownership and synthesis when a worker needs more than three children.
- Concurrency is a ceiling. Fan out disjoint outputs; use light mode for flat
  work and deep mode only for justified recursive discovery and critique.
- Deep nodes use the smallest sufficient child set, normally two to six. Do not
  split a cohesive reviewed leaf merely to fill slots.
- Prefer `implement -> verify`; make failures focused fix/re-verification paths.
  Serialize overlapping mutations or use worktrees.
- Do not reserve a watchdog. Use heartbeats, churn guards, checkpoints, and
  `await_members`.
- Observe subtrees through their owner and artifacts. Prefer bounded commits and
  typed artifacts over polling. On resume, replace stale work and clean up owned
  workers after reports land.
