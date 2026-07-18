# Ideal-base decisions

Append new decisions. Do not rewrite prior decisions to make the program appear
more linear than it was.

## D001. Archive recovery and normalization in place

**Decision:** `docs/fork/recovery/` and `docs/fork/normalization/` remain at their
existing paths as frozen historical namespaces.

**Reason:** the trees contain 600-plus evidence, review, and seam files with
relative links, checksum manifests, and hash-cited records. Moving them creates
integrity risk without improving execution. The active authority moves to
`docs/fork/ideal-base/`.

**Reopen trigger:** an explicitly authorized archive migration with a complete
link, checksum, and citation rewrite plan.

## D002. Preserve the historical orchestrator prompt byte-for-byte

**Decision:** do not edit `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`.

**Reason:** current records state it was restored to tracked baseline and retained
because many historical documents reference it. Archival warnings live in parent
indexes and the active baseline instead.

**Reopen trigger:** explicit user authorization to break the tracked-baseline
preservation guarantee.

## D003. Use graph structure for execution and repository state for restart

**Decision:** the live deep task graph schedules work and enforces artifacts and
gates. `WORK_GRAPH.json`, `STATE.json`, reachable commits, and evidence provide the
cross-session restart authority.

**Reason:** graph artifacts provide typed dataflow while repository checkpoints
survive coordinator or daemon loss.

**Reopen trigger:** a demonstrated task-graph persistence mechanism that makes the
repository state redundant without weakening recovery.

## D004. Separate implementation from acceptance

**Decision:** foundation-critical implementation requires a distinct verification
node or independent reviewer. A failed verifier injects a fix node and repeats the
same gate.

**Reason:** implementation self-assessment is insufficient for lifecycle,
persistence, packaging, and signoff claims.

**Reopen trigger:** none expected. Any exception requires a written risk decision.

## D005. Keep external gates honest and separate

**Decision:** provider, platform, Apple, credential, publication, and push work is
represented in the graph but cannot execute without the applicable authorization.
`authorization_blocked` is a valid explicit disposition and never means passing.

**Reason:** deterministic foundation work should proceed without silently spending,
publishing, or mutating external systems.

**Reopen trigger:** explicit authorization for the named gate and bounded scope.

## D006. Preserve the observed stale pending activation as F09 reproduction evidence

**Decision:** the stale selfdev `pending_activation` observed at session start on
2026-07-18 (requested 05:45:12Z by dead session
`session_peacock_1784221108198_12fe3e2e04160f62`, with `new_version` equal to
`previous_current_version` `923c6353e-dirty-5a0f07fa7495`) is left untouched.
No promotion, rollback, or reload is performed on it by the coordinator.

**Reason:** it is a live instance of the exact failure class node F09 must
reconcile. Clearing it by hand would destroy the best available real-world
fixture and would mutate runtime state outside the graph. The drift is
classified in `evidence/W0.1/drift.md`.

**Reopen trigger:** F09 implementation lands with reconciliation logic, or the
user explicitly asks for a manual manifest repair first.

## D007. Quarantine the stale persisted swarm plan before railway seeding

**Decision:** the persisted swarm plan for `/Users/jrudnik/labs/jcode/.git`
still contained the completed historical recovery program (P*, G*, w3-*
nodes). Seeding W0.2 with `task_graph` merged into that plan, and `run_plan`
resurrected five stale nodes (G4-pilot-execute, P3_gate_recheck, w3-cluster-b,
w3-cluster-c, w3-cluster-d) with fresh workers. Those workers were stopped
within minutes; one had added a partial test to
`crates/jcode-storage/src/active_pids.rs`, preserved as stash
`stale-plan worker (w3-cluster-d/blowfish) ...` rather than deleted. The full
pre-reseed plan snapshot is saved at
`docs/fork/ideal-base/evidence/W0.3/pre_reseed_plan_snapshot.json`. After the
in-flight W0.2 node completes, the stale plan will be cleared
(`swarm:clear_plan`) and the railway graph reseeded cleanly.

**Reason:** the recovery program is a frozen historical namespace; its plan
nodes must not execute again. Clearing the server-side plan does not rewrite
history because all recovery evidence lives in the repository, and the
snapshot preserves the final plan state.

**Reopen trigger:** none. If the stashed worker diff proves useful for F26 it
may be cherry-picked by the F26 owner.

## D008. Apply W0.2 census amendments GN-1, GN-2, GN-5 to the work graph

**Decision:** based on the accepted W0.2 source census
(`evidence/W0.2/source_census.md`, commit `fb00ab840`):

- F06 owned path `src/cli/commands/**/*mcp*` (matched zero files) is replaced
  with `src/cli/mcp_serve.rs` and `src/cli/dispatch.rs`.
- F09 gains owned path `crates/jcode-selfdev-types/src/**` because
  `PendingActivation` lives there.
- F04 gains the explicit acceptance gate "Status-serialization and write
  failures are surfaced, not swallowed".

GN-3 (reuse `OwnedChildPermit`, no second cap counter) and GN-4 (startup PID
sweep pre-exists; F26 starts with a verify of the existing sweep) are recorded
as binding scope guards for F12 and F26 owners rather than graph edits. GN-6
is an observation only.

**Reason:** implementation nodes cannot commit inside their ownership boundary
when the boundary names nonexistent paths, and gates must cover the confirmed
swallowed-error behavior at `background.rs:133`.

**Reopen trigger:** further source drift discovered by any F-node owner.

## D007a. Stale-worker stash resolved by commit

**Amendment to D007:** the preserved stash ("stale-plan worker
(w3-cluster-d/blowfish) ...") was applied and committed as `715d5fd21`
(test(r04): complete streaming-marker 2x2 replacement matrix) during the F01
window; the stash entry no longer exists. Coordinator verified
`cargo test -p jcode-storage --lib active_pids`: 10/10 pass at that commit.
The change is a bounded test addition consistent with F26's seam and is
retained on main.

## D009. Temporary review-model substitution: OpenAI for Opus-class

**Decision:** Anthropic usage is exhausted as of 2026-07-18T07:17Z (user
notice). Until further notice, "Opus-class" verification/critique nodes run on
the strongest available OpenAI route (`gpt-5.6-sol` at high effort, falling
back to `gpt-5.5`). Review artifacts must name the actual model used.

**Reason:** the railway must keep moving; the review-model requirement is
about independent adversarial capability, not vendor identity.

**Reopen trigger:** Anthropic usage restored; subsequent reviews may return to
Opus-class models. Already-accepted reviews are not re-run solely for vendor
identity.
