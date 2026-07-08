<!--
Swarm model routing policy. Global override at ~/.jcode/swarm-prompt.md.
Updated 2026-07-07 per operator corrective (hippo session).
-->

Model routing guidance for spawned swarm agents. Pass `model` (and optionally
`effort`) when spawning or assigning swarm work. Run `swarm list_models` first
when you need to confirm which models/routes are actually available.

- **Implementation / live testing / anything that runs processes:**
  `gpt-5.5`, effort `medium` or `high` (fast mode). GPT 5.5 is the best model
  for tasks requiring live testing; give it the instruction set (or computer
  use) it needs to run live verifications against a process in this harness.
- **Verification / review / critique lanes:** Opus (`claude-opus-4-8` route or
  current Opus), effort up to `high`. Verification work goes to Opus, not to
  the default worker model.
- **Design, investigation, debugging:** `claude-api:claude-fable-5`.
- **Context fetching / bulk reading / summarization:** `gpt-5.5`, effort `none`.
- If the requested route is unavailable, or the user asked for a specific
  model, or you are unsure, omit `model` so the worker inherits the
  coordinator's model.

Structure guidance for spawned swarm agents:

- Always pass `label` when spawning so the swarm UI shows what each agent is
  for.
- Any agent may spawn children; the spawner owns them. A manager is just an
  agent whose prompt tells it to decompose, delegate via spawn, and
  synthesize.
- A worker wanting to delegate more than 2-3 subtasks should spawn one
  manager agent to own that subtree rather than fanning out directly.
