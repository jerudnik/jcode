# Proposal: MCP callback tools for external agents

Status: decisions recorded 2026-08-14 after maintainer review. Implementation
not yet scheduled except where noted.

Source research (all reviewed, evidence-checked):
- /tmp/jcode-mcp-serve-scoping.md (scoping + maintainer inline comments)
- /tmp/jcode-mcp-transport-research.md (transport + MCP protocol currency)
- docs/proposals/structured-agent-dialogue.md (escalation UI primitive)

## Decisions (maintainer-approved)

### Transport
No tool-calling on the debug socket. Add a narrow, token-authenticated
callback request family (CallbackToolsList / CallbackToolCall /
CallbackCancel) on the daemon's MAIN socket, routed to a parent-linked
callback child session (report_back_to_session_id = owner, never
coordinator). Lease token bound to owner session, callback session,
provider/ACP session origin, frozen tool policy, process lease, expiry.
Injection is per-session via ACP session/new mcpServers — never a config
file (see docs/architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md).

### Default tool profile
`coordination-readonly` plus repo inspection:
- swarm read/coordination actions (list, status, summary, plan_status,
  read_context, await_members, report, dm/message, complete_node)
- session_search, conversation_search (read-only)
- todo (session-local progress)
- memory reads (recall, search, list, related)
- agentgrep (repo text/outline/relationship search)
- nix toolbox shells (the `nix` tool's ephemeral command surface)

Excluded by default: bash, file mutation (write/edit/patch), browser,
computer control, gmail, schedule, selfdev, memory.forget, destructive
swarm actions, recursive mcp__* tools.

### Swarm spawn authority is ROLE-based, not runtime-based
The axis is the owning session's role, not which model or runtime it is.
- Driver-seat sessions (user-initiated, top-level) get swarm spawn/drive
  authority by default — a Grok/Kimi/Reasonix driver is trusted like a
  Claude/OpenAI driver.
- Worker-owned callback sessions get coordination-reads only; no spawn.
  Note: this is TIGHTER than native swarm members (which may spawn
  recursively today, bounded by member caps). Deliberate for v1: the
  callback bridge is a new surface. Revisit after the read tier has
  soaked.

### Approval autoclassifier (new, from maintainer)
Three-tier gate for actions outside the static allowlist:
1. Policy allow (static profile) -> execute.
2. Classifier scores the request (blast radius, reversibility, scope
   match to the session's task). High-confidence-safe -> allow with audit;
   otherwise ->
3. Escalate to the human via the structured-dialogue UI
   (docs/proposals/structured-agent-dialogue.md).
Fail-closed invariants: classifier unavailable, timeout, or
low-confidence => escalate or deny, never auto-allow. Design as a
separate component; it can later gate native tool approvals too.

### MCP protocol currency: dual-era NOW
Maintainer wants the 2026-07-28 standard sooner rather than later.
Implement dual-era per the research change list (section 6.2): era-aware
dispatcher, server/discover, per-request _meta validation, -32022,
resultType, cache fields, concurrent cancellation. Legacy initialize
retained for the pinned external agents (Grok 2025-11-25, Kimi
2025-11-25, Reasonix 2024-11-05). Legacy-era fixes already landed in
d03518e19.

## Out of scope, tracked separately
- Mini-harnesses: purpose-built tuned harnesses (prompt + tools + budgets)
  for common task shapes — a swarm-member packaging concept, larger than
  tool allowlists. Needs its own proposal.
- Structured-dialogue UI: separate proposal, is the escalation surface
  for the classifier tier.

## Sequencing
1. Dual-era MCP in mcp-serve (started 2026-08-14).
2. Daemon callback protocol + linked child + lease (needs daemon work).
3. Profile enforcement + role-based spawn tier.
4. Classifier + dialogue-UI escalation (after dialogue UI exists).
