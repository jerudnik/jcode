---
title: "Kimi k3 sessions die on repeated stream_transport_error under long context"
status: open
priority: high
owner: maintainers
opened: 2026-09-03
related:
  - crates/jcode-app-core/src/agent/evidence.rs
  - docs/issues/swarm-model-auto-fallback.md
  - docs/issues/swarm-observability-status-and-wake-gaps.md
---

# Kimi k3 sessions die on repeated stream_transport_error under long context

Streaming, thinking, and connection handling on OpenAI-compatible routes needs a
closer look. The Kimi `k3` route is the concrete reproducer, but the failure
class (`stream_transport_error`, `crates/jcode-app-core/src/agent/evidence.rs`)
is generic to the compat streaming path.

## Evidence (2026-09-03)

Session `tiger` (`session_tiger_1788415171575_140452b05ae51d14`, provider
`openai-compatible:kimi`, model `k3`):

- Nine consecutive turns failed with `stream_transport_error` between 07:56:51Z
  and 08:17:29Z, each after roughly 135 to 185 seconds. The session was dead in
  practice; the user had to abandon it.
- The final request was large: 382,199 bytes of prompt JSON, 276 messages, 173
  tools. Earlier, smaller turns on the same route succeeded (a 06:52:19Z
  failure recovered; the terminal run of nine did not).

Session `whale` (same route, spawned as an independent reviewer) died on its
first turn with the same `stream_transport_error` after 61 seconds and produced
no review verdict. Nothing surfaced the death to the spawner.

## Suspected mechanisms, unverified

- Interplay with reasoning output: k3 streams thinking deltas, and the
  transport error timing (mid-stream, after a long prefill) suggests the drop
  happens while the reasoning channel is open. Compare how the compat layer
  negotiates and parses thinking/reasoning fields against Kimi's current API.
- Long prefill timeouts: the failing requests were at the large end. Establish
  whether the failure correlates with prompt size, and whether read/idle
  timeout budgets in the compat streaming client fit slow-prefill providers.
- No retry or degradation: nine identical failures in a row argue the turn
  loop retried blindly with the same oversized context instead of compacting,
  switching route, or surfacing a structured failure (see
  docs/issues/swarm-model-auto-fallback.md for the routing half).

## Diagnosis work

- Capture the underlying transport error detail (HTTP status, io error kind,
  bytes streamed before drop) in the evidence event; today the class alone is
  recorded, which forced this issue to be reconstructed from timing.
- Reproduce against the Kimi endpoint with a large synthetic prompt, with and
  without thinking enabled, and record where the stream drops.
- Decide the turn-loop response to a repeated terminal transport error:
  compaction, route fallback, or a hard structured failure the coordinator and
  spawner can see.

## Related observation

`~/.jcode/cache/grok-direct_models.json` contains a models listing whose
`source_api_base` is `https://api.z.ai/api/coding/paas/v4`: the per-profile
models cache can be written from another profile's listing. Filed here as a
pointer because it is the same compat-provider plumbing; split it out if it
survives triage.
