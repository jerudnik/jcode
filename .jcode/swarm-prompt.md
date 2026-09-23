## Routing

- Human choices win. Use exact `swarm list_models` names; fail closed rather than guess.
- Never use OpenAI below GPT-5.6. Prefer fast `gpt-5.6-sol` at high for general work; use `gpt-5.6-luna` at high for trivial or mechanical work.
- `claude-fable-5` is not a default worker; use it only by human request, as human-facing orchestrator, or when an explicit workflow condition names it.
- Use `grok-4.6` at xhigh as the Sol fallback. Prefer `glm-5.3`, then `k3` at xhigh for Opus/Fable-class depth; `k3` drops long sessions (docs/issues/kimi-stream-transport-failures.md).
- Spread simple or mechanical work across `deepseek-v4-pro`, `MiniMax-M3`,
  and `glm-5.2`.
- One `run_plan` fixes one model/effort for every worker; use explicit spawns for mixed profiles.

## Execution

- Give every spawn a concrete prompt, short `label`, and `subagent_type`; assign
  one manager when a worker needs more than three children.
- Fan out only disjoint outputs. Use light mode for flat work and deep mode only
  for justified recursive discovery or critique.
- Deep nodes use the smallest sufficient child set, two to six; do not split a
  cohesive leaf to fill slots.
- Prefer `implement -> verify`; make failures focused fix/re-verification paths.
  Serialize overlapping mutations or use worktrees.
- Serena diagnostics apply to a worker's worktree only after its session started there.
- Do not reserve a watchdog; use heartbeats, churn guards, checkpoints, and `await_members`.
- A reviewer with no verdict failed; time out and respawn on another route.
- Observe subtrees through their owner and artifacts. Prefer bounded commits and
  typed artifacts over polling. On resume, replace stale work and clean up owned
  workers after reports land.
