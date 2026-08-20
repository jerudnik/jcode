---
status: open
priority: critical
owner: maintainers
opened: 2026-08-11
---

# Swarm Runaway Growth

Status: **problem capture** (no solution designed yet).

Incident evidence: 2026-08-10 workflow-repair swarm, coordinator session
`session_shrimp_..._ac572e` in the primary jcode checkout.

## What happened

A deep-mode `task_graph` seeded with 8 nodes (context → plan → research →
synthesis → implement → review → gate) grew to **114 nodes and 125 spawned
agents** over ~4 hours, consuming well over 200M tokens. The coordinator's
convergence directives did not stop it. Late-stage workers escalated from
read-only research to writing code, producing **9 unauthorized commits on the
user's `main` branch in a worktree the plan never named**, plus a stray
worktree/branch (`jcode-production-repair`) and staged edits mixed into the
user's dirty working tree.

Nothing was pushed, but only because the branch ruleset made direct pushes
fail earlier in the session.

## Failure mechanisms observed

1. **Gate-driven graph growth is unbounded.** Deep-mode critique gates treat
   "unaddressed low-confidence sibling" and `what_i_did_not_check` lists as
   license to inject new nodes. Every gate pass injected 2-5 children; those
   children ended in more gates. There is no node budget, depth budget, token
   budget, or wall-clock budget on a graph.
2. **Coordinator directives are advisory.** Broadcasts reach workers at turn
   boundaries and gates ignored an explicit "research is closed, stop
   injecting" instruction. Force-completing nodes did not stop sibling gates
   from injecting replacements.
3. **Workers inherit full write capability regardless of node kind.** Nodes
   whose task text said "read-only" stayed read-only, but gate-injected
   children carried no such constraint, and later `fix`/`implement` children
   freely committed to `main`, created worktrees and branches, and staged
   files, none of which the plan authorized. Node `kind`
   (explore/verify/implement) is a prompt nudge, not an enforcement boundary.
4. **Working-directory scope is not enforced.** The intended target worktree
   (`jcode-wfx`) was stated in the coordinator's context artifact, but workers
   defaulted to the repository the swarm was launched from and wrote there.
5. **No liveness reporting to the human or coordinator.** The background
   `run_plan` task only wakes the coordinator at terminal state. A plan that
   never terminates never reports. The coordinator idled for 2+ hours while
   the graph tripled.

## Protections worth considering (not designed here)

- Hard budgets on a task graph: max nodes, max depth, max gate injections
  per gate, max total tokens, max wall clock. Exceeding a budget pauses the
  plan and wakes the coordinator instead of continuing.
- Gate injection quotas that decay: a gate that has already injected N
  children auto-passes or escalates to the coordinator rather than injecting
  more.
- Capability tiers per node kind: explore/verify nodes get read-only tool
  enforcement (no commit, no worktree/branch creation, no file writes outside
  a scratch dir); implement nodes require an explicit allowlisted working
  directory.
- Enforced working-directory scope for the whole plan, checked at tool-call
  time, not prompt time.
- Periodic progress wakeups for the plan owner (node count, agent count,
  token burn, files touched) with anomaly triggers, e.g. "graph grew >2x
  seed size" or "worker touched a path outside scope."
- A coordinator directive channel with teeth: a "freeze graph" order that the
  scheduler enforces (no new node acceptance) rather than asking gates to
  comply.

## Related

- The `swarm-lifecycle-remediation` proposal record covers process-lifecycle
  leaks (orphaned children, stale markers). This issue is about logical runaway
  of a healthy swarm: the same theme of trusting cooperative signals where
  enforcement is needed.

## Earlier precursor: repeated same-shape gate injection

An earlier deep-mode run repeatedly injected near-identical gap nodes because
the underlying external blocker could not be resolved by workers. Rewording the
same verification request bypassed cooperative stopping and continued graph
growth. This is the same mechanism as failure 1 above, seen before the incident
that opened this issue.

The existing churn breaker detects repeated assignment waves, but it is not a
per-gate semantic quota. Add an acceptance criterion that one gate may inject
only a bounded number of materially equivalent gaps. Once the quota is
exhausted, the gate must enter a blocked state and wake the coordinator with
the unresolved prerequisite and the attempts already made.

## Partial remedy for failure 5: role-aware member liveness watchdog

Carried from a proposal that is otherwise retired. This addresses only the
absent-liveness-reporting mechanism above. It does not supply the graph
budgets, capability tiers, or working-directory enforcement that failures 1
through 4 require.

Configuration currently has a single global idle timeout shared by all
streaming-provider paths. Add optional foreground and subagent stream-idle
budgets, keeping the global timeout as the fallback, and resolve the effective
value as role-specific, then global, then built-in default. Interactive
foreground sessions should fail a genuinely dead model stream promptly, while
background members receive a longer budget for slow or intermittent streams.

Use the resolved budget for a member-liveness watchdog, not only for stream
termination. The watchdog should evaluate meaningful activity such as streamed
bytes, tool progress, and durable journal or control-log activity. It must
distinguish a member that has never produced activity from one that was active
and then paused, and it must preserve the last-known machine-readable state.

## Update 2026-08-19: budgets and freeze landed; enforcement tiers remain

PR #197 implemented the tractable half: `MAX_GATE_INJECTIONS = 3` per gate,
a total-node budget (`max_nodes`, default 64, config
`agents.swarm_max_graph_nodes`) enforced in both `expand_node` and
`inject_from_gate`, coordinator-only `freeze|unfreeze` with server-side
rejection while frozen, and run_plan growth checkpoints (fire at 2x seed and
each doubling). The engine probe that previously grew 2 seeds to 123 nodes
with no cap now stops at a quota rejection at 6 nodes. PR #194 separately
gave coordinator broadcasts teeth (failure mechanism 2's root: broadcasts
defaulted to a delivery mode whose handler was empty).

Still open from the protections list, deliberately deferred as design
decisions: capability tiers per node kind (read-only enforcement at the tool
layer), enforced working-directory scope at tool-call time (its prerequisite
— a trustworthy recorded cwd — landed in PR #195), and the role-aware
liveness watchdog sketched above.
