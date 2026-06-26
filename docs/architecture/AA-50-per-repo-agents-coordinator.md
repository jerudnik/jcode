# AA-50: Per-Repo Tool-Rich Agents + Lightweight Coordinator Routing

Date: 2026-06-26
Status: design (decompose into child items; name the smallest safe prototype)
Pad: AA-50 (milestone). Builds on AA-35 (profiles), swarm-core, tool registry.

## The shape (user direction, not over-built)

Not rigid "SME roles." The useful shape is: **an agent bound to each repo, carrying a strong, bounded tool set for working inside that project.** Ten agents that each know ten surfaces beat one agent juggling a hundred. The motivating workflow: enter a repo, state the problem, and ask the in-repo tool-rich agent to **generate a starting prompt/plan for a fresh session**, rather than plan-then-implement inline.

## What already exists (verified, so we compose not invent)

- **Assistant profiles (AA-35):** named, persisted, cwd-pinned sessions. This is the binding for "the agent for repo X." Profiles already load repo-local context (AGENTS.md, `.jcode/` overlays) per dir. Persona/mode shipped in AA-45/AA-46.
- **Tool scoping already exists:** `Registry::definitions(allowed_tools: Option<&HashSet<String>>)` (crates/jcode-app-core/src/tool/mod.rs:328) filters the tool set per call; `ToolPolicy.allowed_tools` gates execution (mod.rs:557). So a curated per-agent tool surface is a config + wiring task, not new machinery.
- **swarm-core report-back exists:** `SwarmRole` (Agent/Coordinator/WorktreeManager/`Other(String)`), `ChannelIndex` pub/sub, `format_structured_completion_report` capped at `MAX_SWARM_COMPLETION_REPORT_CHARS=4000`. A coordinator collecting structured reports is already a primitive.

The gap is small: swarms are task-scoped/ephemeral today; this wants **persistent, repo-bound agents** a coordinator can repeatedly hand problems to, plus "generate a starting prompt" as a first-class output.

## Design

### 1. Per-repo agent = profile + curated tool set + repo context

Extend `AssistantProfile` with an optional `tools` allow-list (names). At launch, thread it into the session so `Registry::definitions`/`ToolPolicy` scope to that set. Absent = all tools (today's behavior). This is the bounded-surface lever; it reuses the existing `allowed_tools` plumbing end to end.

### 2. "Generate a starting prompt/plan" as a first-class repo-agent output

A repo agent's highest-value output is a fresh-session prompt, not inline execution. Model this as a tool/playbook (`plan_for_fresh_session` or a playbook) that takes a goal and emits a structured plan + a ready-to-paste starting prompt, grounded in the repo's context. This matches the observed workflow and is a contained, testable unit.

### 3. Lightweight coordinator routing

A coordinator profile (`global`, seeded in AA-46) that, given a goal, picks the right repo agent, hands off, and collects the structured report. Reuse `format_structured_completion_report` + `ChannelIndex`; do **not** build a new framework. `SwarmRole::Other("repo-agent")` is enough; no role ontology.

### 4. Knowledge consolidation

Persist repo-agent completion reports as typed records (ties to AA-42 TaskOutcome / AA-48 evidence rollups) so a repo accrues working knowledge across sessions instead of re-deriving it. This is the link to the self-improvement spine, not a separate store.

## Non-goals

- A rigid taxonomy of SME role names (`Other(String)` is fine).
- A new orchestration framework (reuse swarm-core reports/channels).
- Auto-execution by the coordinator (it routes and integrates; the repo agent does the work, or just plans).

## Smallest safe prototype

**Profile tool-scoping + a `plan_for_fresh_session` output, single repo, no coordinator yet.** Concretely: add `AssistantProfile.tools` (allow-list), wire it through `Registry::definitions`, and add a tool/playbook that turns a stated goal into a structured plan + starting prompt using repo-local context. This proves the two load-bearing claims (bounded tool surface works; repo agent produces a plan/prompt) with zero new orchestration risk. The coordinator + knowledge-consolidation layers follow as separate child items once the repo-agent unit is real.

## Child items (to create under AA-50)

1. **AA-50a Profile tool-scoping**: `AssistantProfile.tools` allow-list threaded through `Registry::definitions`/`ToolPolicy`; tests prove a profile sees only its set. (Smallest prototype, ship first.)
2. **AA-50b `plan_for_fresh_session` output**: tool/playbook turning a goal into a structured plan + starting prompt from repo context; render + serde test.
3. **AA-50c Lightweight coordinator routing**: a coordinator profile routes a goal to a repo agent and collects a structured report, reusing swarm reports/channels; route test.
4. **AA-50d Repo-knowledge consolidation**: persist repo-agent reports as typed records feeding AA-42/AA-48; round-trip + trend-query test.

## Validation

- Evidence: per child item — tool-scoping test (a profile's `definitions` excludes non-listed tools), plan-output render/serde test, a route-a-goal-get-a-report integration test reusing `format_structured_completion_report`.
  Proves: bounded tool surface per repo agent; repo agent yields a structured plan/prompt; coordinator routes and collects without a new framework.
  Limit: does not prove multi-repo coordination quality at scale, nor that bounded surfaces measurably reduce error (that needs the AA-49/AA-41 measurement harness over real sessions).
