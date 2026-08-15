# Stream-idle policy by role

Status: Proposal seed

## Problem

`provider.stream_idle_timeout_secs` is one global scalar. Two conflicting
needs:

- **Interactive foreground sessions** want fast detection of a dead/broken
  model stream (~3 min or less) so failover countdown starts quickly.
- **Background swarm workers / subagents** on spotty networks want patience
  (~10 min): a premature idle-kill wastes a long turn's work and repeats it.

Today the operator picks one number and eats the other cost (currently 600s
globally, which makes broken-model detection slow in the foreground).

## Direction

Role-aware idle timeouts with the global key as fallback:

```toml
[provider]
stream_idle_timeout_secs = 600            # fallback (unchanged semantics)
stream_idle_timeout_foreground_secs = 180 # optional; interactive sessions
stream_idle_timeout_subagent_secs = 900   # optional; swarm/subagent sessions
```

Session role is already known at spawn time (subagent/swarm spawns are
distinguishable from operator-attached sessions). Resolution order:
role-specific key → global key → built-in default.

Refinement (later, not v1): adaptive idle — a stream that has
produced *no bytes at all* gets a shorter budget than one that was mid-flow
and paused, since the former is the broken-model signature and the latter is
the slow-network signature. Keep v1 dumb and role-based.

## Watchdog refinement (promoted from "later": operator-reported, evidence attached)

The adaptive-idle refinement matters sooner than v1 planning assumed, because
stream idleness is only one instance of a wider gap: **nothing monitors swarm
member liveness and completion on the coordinator's behalf.** Operator-observed
failure modes, all reproduced in the 2026-08-12/13 test-audit sessions:

- Coordinators do not use `swarm await` consistently, and their behavior when
  a member finishes is inconsistent — some react, some never notice.
- Members finish real work but never file a completion report, or file one and
  are still marked `failed` (observed repeatedly: workers whose full output was
  on disk carried `failed` lifecycle status after provider errors on the final
  turn).
- Members wedge: one `xai/grok-4.5` worker produced no output for ~45 minutes
  before being manually stopped; the same task completed in minutes after a
  respawn on a different route.
- Swarms can spin out of control entirely (see `swarm-runaway-growth.md`).

Proposed direction: a **watchdog** that monitors member activity (stream
bytes, tool calls, journal writes) against role-aware idle budgets — the same
budgets this proposal introduces — and alerts the coordinator instead of only
killing streams. Escalation ladder: notify coordinator → mark member suspect
in `swarm list` → offer stop/respawn. A member that dies abnormally should
still yield a machine-readable last-known state for the coordinator.

This is the runtime-signal complement to `swarm-lifecycle-remediation.md`
(which covers PID-liveness and marker cleanup); the two should share the
liveness probe.

## Acceptance criteria (v1)

- [ ] Two optional keys, resolution order as above, config docs updated.
- [ ] Subagent spawn path threads the role into stream setup.
- [ ] Unit test per resolution branch.
- [ ] No behavior change for configs that set only the global key.
