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

## The one decision this needs

Everything else follows from it: **should a resolvable model ever be
refused?**

- **No.** Then close this issue. Record the decision and delete the file; the
  advisory prose is the design, not a gap.
- **Yes.** Then the smallest useful version is a deny list, not an allowlist.
  A deny list needs no per-role vocabulary, no capability classes, and no
  agreement on what "mechanical work" means. It expresses the concrete thing
  that has actually gone wrong: a model the operator does not want used at
  all, used anyway.

Start there and let the interface work decide whether anything richer earns
its place. An allowlist per role (ambient, swarm, subagent, coordinator) and
capability or cost classes are the richer options, but each needs a vocabulary
that does not exist yet.

## First slice, if the answer is yes

1. Add a config key holding denied model identities. Route-prefixed form, so
   `cursor:gpt-5.6-sol-high` and a bare `gpt-5.6-sol` are distinguishable.
2. Check it in `resolve_swarm_spawn_selection` beside the existing
   fail-closed branch, so both refusals share one error path and one message
   shape.
3. Refuse hard rather than warn. A warning that still spawns reproduces the
   `provider-confusion` failure: the operator believes one thing while
   another runs. That incident is the argument against soft failure.
4. Apply the same check to `subagent`, which reconstructs selection from
   inherited parent route state and is listed as a deferred follow-up in
   `provider-confusion.md`.

The test that must fail first: a spawn naming a denied model succeeds today.

## Relationship to the model picker work

This issue is the enforcement point for whatever routing strategy the model
selection interface ends up expressing. If a picker grows per-role routing
with fallbacks, this resolver is where that policy has to be honoured, or the
interface will describe a policy the system does not keep.

Sequencing: settle the routing model in the interface work, then implement
enforcement here. Implementing enforcement first risks pinning a vocabulary
the interface then has to work around.
