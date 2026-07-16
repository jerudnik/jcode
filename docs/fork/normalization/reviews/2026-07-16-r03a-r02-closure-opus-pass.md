# Independent review: R03A and R02 closure

Date: 2026-07-16

Reviewer: independent Opus verification lane
`session_penguin_1784230788304_503f8efa803b70a1`

## Verdict

**PASS. Close both candidates as unwarranted, with named re-open triggers.**
This satisfies Completion Standard D3: each dormant candidate is implemented
with evidence or explicitly closed rather than left under an ambiguous W7 label.

## Strongest evidence

1. `docs/fork/recovery/reviews/2026-07-16-w7-review.md:24-35,45-47`
   explicitly recommends keeping R03A verdict centralization and R02 file
   splitting dormant because neither has a new correctness trigger.
2. `crates/jcode-app-core/src/server/handshake.rs:22-68` already centralizes
   compatibility evaluation and verdict construction. The remaining call sites
   are adapters at different lifecycle phases: pre-session direct transport
   rejection and post-initialization event-channel notification.
3. `crates/jcode-base/src/provider/mod.rs` and `crates/jcode-base/src/sidecar.rs`
   are large, but they sit on configuration, credential, entitlement, routing,
   and failover boundaries. LOC does not supply a safe extraction authority.
   R09 specifically rejects blanket cleanup without a correctness trigger.

## Regression and benefit analysis

### R03A

Further consolidation offers little benefit because the compatibility decision
is already single-sourced. Combining the two transport adapters would couple
handshake semantics to both pre-session writer and initialized event-channel
lifetimes. That increases lifecycle and fail-closed regression risk while
removing no duplicate policy.

### R02

A blanket split may make files visually smaller, but it would move private
state and visibility across high-risk provider and sidecar boundaries without
fixing a defect. The safer disposition is to keep exact size debt in the normal
quality register and require future bounded extraction to have a named
responsibility and focused behavioral tests.

## Re-open triggers

Re-open R03A only if compatibility evaluation or verdict construction escapes
the current authority, a third transport adapter appears, or equal inputs yield
different verdict contents across lifecycle paths.

Re-open R02 only alongside a real feature or correctness change that exposes a
narrow authority, has focused before/after tests, reduces tracked size without
creating another oversized file, and receives independent review for auth,
entitlement, routing, and failover safety.

## Findings

- CRITICAL: 0
- IMPORTANT: 0
- MINOR: 0

Confidence: high.

Not checked: full runtime/provider behavior and the final N2 validation matrix.
Those are separate fixed-head gates and are not implied by this closure review.
