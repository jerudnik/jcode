---
status: open
priority: medium
owner: maintainers
opened: 2026-08-19
related:
  - crates/jcode-app-core/src/server/comm_session.rs
  - crates/jcode-app-core/src/tool/communicate.rs
  - docs/architecture/provider-confusion.md
---

# Swarm model-routing policy is advisory, so a per-spawn model always wins

Carried from a bug register during proposal triage. This is the **policy**
half of a problem whose **resolution** half is already documented and partly
fixed in `docs/architecture/provider-confusion.md`. Read that first; this
issue should not restate it.

## Claim

Routing guidance is prose, and nothing enforces it. Any concrete per-spawn
model overrides the configured `agents.swarm_model` pin, with no policy class,
no allowlist, and no refusal path.

## What is grounded

`resolve_swarm_spawn_selection` (`server/comm_session.rs:399-429`) lets an
arbitrary per-spawn concrete model take precedence over the configured pin.
The routing document loaded into the coordinator's context is advisory text;
`.jcode/swarm-prompt.md` states the intended policy in sentences, and the
resolver has no corresponding check.

The distinction that matters: `provider-confusion.md` Path A made the resolver
**fail closed on an unresolvable name**. That is a correctness gate. It is not
a policy gate. A name that resolves perfectly well but violates the operator's
routing policy still spawns.

## What is NOT established

- Whether enforcement is wanted at all, or whether advisory routing is the
  deliberate design. An agent choosing a cheaper model for mechanical work is
  the intended behaviour; the question is whether *any* choice should be
  refusable.
- What the policy vocabulary should be. Candidates: an allowlist per role
  (ambient, swarm, subagent, coordinator), a cost or capability class rather
  than named models, or a refusal only for models the operator has explicitly
  denied.
- Whether refusal should be hard, or a warning that still spawns and records
  the divergence.

## Relationship to the model picker work

This issue is the enforcement point for whatever routing strategy the model
selection interface ends up expressing. If a picker grows per-role routing
with fallbacks, this resolver is where that policy has to be honoured, or the
interface will describe a policy the system does not keep.

Sequencing: settle the routing model in the interface work, then implement
enforcement here. Implementing enforcement first risks pinning a vocabulary
the interface then has to work around.
