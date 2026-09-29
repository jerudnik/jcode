---
title: "Forge event ingress: wake sessions on webhooks or subscription polls instead of held turns"
status: open
priority: medium
owner: maintainers
opened: 2026-09-04
related:
  - docs/issues/scheduled-wake-and-background-resume-broken.md
  - crates/jcode-base/src/inbox/delivery.rs
  - docs/issues/swarm-model-auto-fallback.md
---

# Forge event ingress: wake sessions on webhooks or subscription polls instead of held turns

Waiting on CI is the dominant long wait in this repository's workflow (30-50
minutes per merge under the up-to-date rule). Today an agent either holds a
turn open watching `gh pr checks` or polls on wake cycles. The durable
primitives for something better now exist and are verified: the session inbox
delivery engine (w23-c6) and wake-on-completion (live-tested 2026-09-04,
0.4s wake latency after a yielded turn). What is missing is an ingress that
converts forge events into those wakes.

## Shape

A subscription registry plus two interchangeable event sources:

- A session (or swarm coordinator) registers a subscription: repository,
  event kind (check suite concluded, PR merged, push to branch), optional
  narrowing (PR number, ref), and a delivery target (wake this session with
  this context). Subscriptions persist in the durable store, so they survive
  reloads and are cancelled when the session ends.
- **Webhook source.** An opt-in localhost/tailnet HTTP listener
  (`[webhooks]` config: bind address, per-forge HMAC secret). Forgejo and
  Gitea deliver push/PR/status webhooks natively and can reach a listener on
  the same LAN or tailnet directly, so for a self-hosted forge this is the
  zero-latency path. Validate HMAC, map payload to subscriptions, append to
  the target session's inbox with wake. Never expose the listener beyond
  loopback/tailnet interfaces.
- **Polling source.** github.com cannot reach a laptop listener without a
  relay (smee.io, a tunnel), which adds exposure and infrastructure for
  little gain. For GitHub-hosted repositories the same subscription registry
  is served by a server-side poller: conditional requests (ETag) against the
  checks/PR API on a modest interval, diffing conclusions, and firing the
  identical inbox wake. No agent turn is held anywhere; latency of a minute
  is fine for CI.

The subscription surface must not care which source fires it. That keeps the
Forgejo webhook path and the GitHub poll path the same feature, and lets a
repository migrate forges without touching agent behavior.

## Non-goals

- No public webhook endpoint, tunnels, or relays by default.
- Not a general HTTP server: one route, HMAC-validated, JSON-body-size
  capped, event kinds allowlisted.
- Not a replacement for `bg` wake or `ScheduleWakeup`; it is a third event
  source feeding the same inbox delivery engine.

## Acceptance sketch

- A subscription registered by a session survives a self-dev reload and
  fires exactly once per matching event.
- A Forgejo webhook delivery (HMAC-signed fixture) wakes the target session
  through the inbox engine without any client attached.
- A GitHub poll subscription on a PR's check suite wakes the session when
  the rollup concludes, with zero held turns in between; the poller respects
  ETags and backs off on API errors.
- A dead target session cleans up its subscriptions.
