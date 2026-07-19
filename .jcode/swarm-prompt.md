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

Monitoring sub-orchestrator subtrees (learned 2026-07-10, avoid rediscovering):

- `swarm status`/`summary` on a member of ANOTHER swarm fails with "not in the
  same swarm as requester". A sub-orchestrator's workers form its own swarm;
  you cannot introspect them directly. Do not retry variations of the call.
- Cheap non-disruptive progress signals, in order of preference:
  1. Expected artifacts: the ledger/progress file the prompt told workers to
     write, and `git log --since` on every repo the work touches.
  2. Session journal freshness: `~/.jcode/sessions/session_<name>_*.journal.jsonl`
     mtime (seconds-old = alive and working).
  3. Recently-modified sibling journals reveal spawned workers by name.
- Only DM the sub-orchestrator for a progress report if artifacts and journals
  have BOTH been silent for ~20+ minutes; a DM interrupts its turn.
- Design prompts so progress is observable: require a shared ledger file with
  per-worker sections and commit-as-you-go, then monitoring is `ls` + `git log`.

## Coordinator discipline (from the 2026-07-18 ideal-base incidents)

- **Do not mutate the tree while workers own paths in it.** While a plan is
  running, the coordinator writes only coordinator-owned files (state
  checkpoints, decision logs). Imperative edits or commands against
  worker-owned paths mid-run caused the frozen-tree mutation and stash
  collisions in the F01-era meltdown.
- **Seed small graphs; encode the review loop in the graph.** Prefer
  `implement -> review` chains with `depends_on` over stopping between
  nodes. A FAIL review should become an `inject_gap` fix node plus a re-run
  of the same review node, not a coordinator takeover.
- **Expansion is capped in the engine** (depth 2, 24 children per
  expansion). If a subtask shares most of its context with its siblings,
  do it directly in one pass instead of decomposing; fan out only
  context-disjoint work.
- **Check for stale plan items before seeding.** Accepted work can be
  resurrected as queued seed copies after reloads; snapshot then
  `swarm:clear_plan` when the plan does not match the durable checkpoint.
- **Never drive `jcode debug` swarm commands at your own session** (the
  debug command waits on the agent loop that is busy running it). Use the
  native swarm tool (available via `.jcode/mcp.json` -> `jcode mcp-serve`),
  or a dedicated headless driver session.
