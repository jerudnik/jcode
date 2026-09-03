---
title: "Optional model auto-fallback for unattended swarm member sessions"
status: open
priority: high
owner: maintainers
opened: 2026-09-03
related:
  - docs/issues/kimi-stream-transport-failures.md
  - docs/issues/swarm-observability-status-and-wake-gaps.md
  - .jcode/swarm-prompt.md
---

# Optional model auto-fallback for unattended swarm member sessions

When a provider route degrades mid-session, an unattended swarm worker just
dies quietly. On 2026-09-03 a k3 worker burned nine consecutive turns on
`stream_transport_error` over 26 minutes, and a k3 reviewer died on its first
turn without a verdict; in both cases progress stopped until a human looked at
a terminal. Nobody is watching these screens by design, so the session itself
must recover.

## Desired behavior

An opt-in fallback policy, most valuable for swarm-spawned members:

- After N terminal provider failures of the same class in a row (transport
  drops, auth expiry, rate-limit exhaustion), the session switches to the next
  route in a configured fallback chain and continues the same turn.
- The switch is recorded loudly: evidence event, session journal, swarm status
  chip, and a note in the member's next report, so post-hoc audit shows which
  model produced which work.
- Chain resolution follows the same catalog rules as spawn (`swarm
  list_models` names, fail-closed); a chain entry that no longer resolves is
  skipped, and exhausting the chain produces a structured failure the spawner
  is woken with, rather than silence.
- Coordinator-facing default: spawns can pass `fallback: [...]` alongside
  `model`, and the routing prompt can recommend standard chains per class of
  work. Interactive foreground sessions keep today's behavior unless the user
  opts in.

## Prior art

The shape is a solved problem in other harnesses: the oh-my-pi coding agent
(TypeScript) ships per-session model fallback chains with automatic
advance-on-failure. Review its policy surface (what counts as a fallback-worthy
failure, how it avoids flapping) before designing ours.

## Interactions to design around

- Failure classification: only terminal, provider-side failures should advance
  the chain. A tool error or a context-length rejection needs compaction, not a
  new provider.
- Capability drift: chains should stay within a capability class (the
  model-routing skill's tiers) so a verify node does not silently fall from an
  Opus-class reviewer to a flash-class one; a downgrade should be recorded as
  such.
- Flapping and cost: back off before re-trying the primary; cap chain
  traversals per session.
- Thinking-state carryover: reasoning blocks in the transcript may not be
  portable across providers; the switch point must rebuild the request in the
  target provider's dialect (this is the same seam as
  docs/issues/kimi-stream-transport-failures.md).
