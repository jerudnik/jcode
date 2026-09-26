## Routing

- Human model choices win. `swarm list_models` is the authority: pass names
  exactly as printed (fail-closed). Never guess.
- Research (`explore`): rotate `k3`, `glm-5.3-flash`, `grok-4.6` at high
  across parallel roots.
- Hard implementation: alternate `gpt-6-astra` high and `claude-fable-5-1`
  medium. Small or mechanical: `grok-4.6` or `glm-5.3-flash`.
- Hard review (`verify`): opposite provider family from the author. OpenAI
  work -> `claude-opus-5` or `claude-fable-5-1`; Anthropic work ->
  `gpt-6-astra` or `gpt-5.6-terra`. Low-stakes review: `k3`.
- Synthesis and decisions: `claude-fable-5-1`.
- No OpenAI model below GPT-5.6. Saturated OpenAI -> `grok-4.6` xhigh;
  saturated Anthropic -> `glm-5.3`. `k3` drops long sessions; keep it bounded.
- One `run_plan` applies one model/effort to all workers it creates. Keep such
  runs profile-homogeneous; otherwise use explicit spawns.

## Execution

- Every spawn needs a concrete prompt, short `label`, and `subagent_type`. Give
  one manager ownership and synthesis when a worker needs more than three children.
- Concurrency is a ceiling. Fan out disjoint outputs; use light mode for flat
  work and deep mode only for justified recursive discovery and critique.
- Deep nodes use two to six children; do not split a leaf to fill slots.
- Prefer `implement -> verify`; make failures focused fix/re-verification paths.
  Serialize overlapping mutations or use worktrees. Workers run crate-scoped
  tests; one serialized full-workspace run per integration.
- No watchdog. Use heartbeats, churn guards, checkpoints, `await_members`.
- A dead reviewer with no verdict is a failed review: await with a timeout
  and respawn on another route.
- Observe subtrees through their owner and artifacts. Prefer bounded commits and
  typed artifacts over polling. On resume, replace stale work and clean up owned
  workers after reports land.
