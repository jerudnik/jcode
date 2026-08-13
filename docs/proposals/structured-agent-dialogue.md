# Proposal: structured agent-to-user dialogue (multi-choice, plan review, Q&A)

Status: idea, captured 2026-08-13. Not scheduled.

## Observation

Jcode has no first-class way for an agent to ask the user a structured
question mid-turn. This shows up in two places:

1. Native providers (Claude, OpenAI, all direct routes): an agent that wants
   a decision ("which of these three approaches?", "review this plan before I
   continue") can only emit prose and end its turn. There is no option list,
   no plan-review affordance, no structured answer.
2. ACP providers (see /tmp/acp-provider-runtime-design.md §4, §7.4): the ACP
   `session/request_permission` reverse RPC carries arbitrary option lists,
   and Kimi/Reasonix use it for plan approval and questions, not just
   yes/no tool permission. Jcode's current permission surface is binary
   approve/deny and async-queued, so it cannot answer these without a new
   synchronous multi-choice channel.

## Direction

The ACP work forces a minimal active-turn approval channel that can display
content plus an arbitrary option list and return the selected option id.
Design that channel so it is provider-agnostic:

- One UI primitive: "agent asks, user picks (or types)" — options,
  free-text fallback, plan/diff content rendering.
- ACP consumes it for permission/plan/question reverse RPCs.
- Native providers could consume it via a tool (e.g. an `ask_user` tool the
  agent can call) once the primitive exists.
- Room for a richer UI later (side panel, dedicated overlay) — the user is
  interested in designing this as its own experience.

## Constraints

- Fail closed: timeout/dismiss must map to reject/cancel, never auto-select.
- Remote/headless sessions need a story (queue the question, notify, or fail
  fast) before the tool is exposed by default.

## Origin

User request during ACP provider onboarding work; spider's ACP runtime design
identified the same gap as a hard dependency (§4.3, §7.4).
