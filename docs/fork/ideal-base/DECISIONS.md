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
