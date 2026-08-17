---
title: "Rate-limit notifications flood agent context as repeated user turns"
status: open
priority: medium
owner: unassigned
opened: 2026-08-17
---

# Rate-limit notifications flood agent context as repeated user turns

When a session is rate-limited while the operator keeps a message queued (or the
operator message itself is throttled), the harness delivers the full user
message once per retry instead of surfacing a compact backoff notice. The agent
then wakes to a wall of identical turns.

## Observed

1. **42 identical copies of one user message** were delivered to a single
   session between `09:39:52Z` and `09:51:29Z`, at a near-constant ~15-16s
   cadence. Each copy was a full user turn (~40 tokens); the same body with a
   longer prompt would inject proportionally more duplicate content.
2. **The cadence did not back off.** Roughly constant retry spacing over 11+
   minutes suggests no exponential backoff on the delivery path, or a backoff
   that resets per attempt.
3. **Delivery targeted a dead agent.** The session was mid-tool-call for the
   entire window; the queue accumulated against a consumer that could not
   drain it, then emptied all 42 turns into context the moment the agent woke.

## Preferred behavior

- Exponential backoff with jitter on redelivery attempts.
- Do not enqueue per-attempt copies of the same undelivered message. Deliver it
  once, and if the agent was unavailable, prepend a single compact notice
  (e.g. "message delayed N times over M minutes by rate limiting") rather than
  replaying the body per attempt.
- Consider coalescing: identical queued bodies collapse to one turn plus a
  count.

## Related

- `docs/issues/cross-session-content-leakage.md` — same morning, same fleet of
  concurrent sessions; item 5 (todo content crossing repositories) was also
  reported by the operator during this window. In the affected session the
  todo tool returned an empty list when queried, so whatever the operator saw
  rendered did not match that session's stored todos — consistent with a
  UI-surface rendering another session's state rather than storage scoping.
