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

Possible refinement (later, not v1): adaptive idle — a stream that has
produced *no bytes at all* gets a shorter budget than one that was mid-flow
and paused, since the former is the broken-model signature and the latter is
the slow-network signature. Keep v1 dumb and role-based.

## Acceptance criteria (v1)

- [ ] Two optional keys, resolution order as above, config docs updated.
- [ ] Subagent spawn path threads the role into stream setup.
- [ ] Unit test per resolution branch.
- [ ] No behavior change for configs that set only the global key.
