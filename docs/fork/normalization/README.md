# Post-recovery fork normalization

The six-phase recovery is complete. This directory defines the separate program
that turns the signed recovery branch into the normal operating state of the
fork: one canonical repository, clean history and task state, completed W7
maintenance, live runtime validation, and a normalized local host.

This program is tracked by durable initiative
`post-recovery-fork-normalization`.

## Authoritative files

- [`COMPLETION_STANDARD.md`](COMPLETION_STANDARD.md): normative, binary completion criteria.
- [`BASELINE.md`](BASELINE.md): repository and host-topology facts to revalidate
  before mutation.
- [`COORDINATOR_BRIEF.md`](COORDINATOR_BRIEF.md): copy-ready coordinator brief
  for the next session.
- [`RUNTIME_AND_NIX_RUNBOOK.md`](RUNTIME_AND_NIX_RUNBOOK.md): clean build,
  immutable channel, live handoff, verification, and rollback procedure.
- [`../recovery/reviews/2026-07-16-w7-review.md`](../recovery/reviews/2026-07-16-w7-review.md):
  reviewed W7 scope and implementation order.
- [`../SYNC_MODEL.md`](../SYNC_MODEL.md): current monitored-curation policy,
  which normalization must refresh without silently returning to broad replay.

Normalization evidence belongs under `evidence/` and independent reviews under
`reviews/`. Evidence is committed in a bounded number of dedicated documentation
commits, not interleaved through the curated product stack.

## Infrastructure review history

The first independent draft reviews both returned FAIL and are preserved rather
than replaced:

- [Fable architecture/completeness review](reviews/2026-07-16-infrastructure-draft-fable.md),
  SHA-256 `ac708e64ab94177452cf8209ae701d568eda7d99d44aa7edd327b7ff3c95706e`;
- [Opus operational/safety review](reviews/2026-07-16-infrastructure-draft-opus.md),
  SHA-256 `a9fe8180b8e335a0872e0565b45a2ec514fa6e9ac4b9ab8148152dab2f624fa2`.

Their blocking findings drove the tracked-authority requirement, explicit stash-
object and all-ref archives, N1/N2 promotion ordering, exact host/runtime
classifications, live-binary rollback anchors, evidence placement, remote-state
labeling, and independent final-review rules. Corrected-candidate re-reviews must
be preserved beside them before this infrastructure is called approved.

The first commit attempt was also rejected by the repository PM-surface hook:
the checkbox-heavy `DEFINITION_OF_DONE.md` and tracking-named
`NEXT_SESSION_PROMPT.md` belonged in project tracking rather than repository
documentation. No commit or history movement occurred. The durable initiative
retains task state; these repository files were corrected into normative
`COMPLETION_STANDARD.md` and operational `COORDINATOR_BRIEF.md` documents.

The first committed corrected candidate, `1f938b7e537a20aaad133ec300d0cfdc6368bca0`,
received independent Fable and Opus PASS verdicts with zero CRITICAL or IMPORTANT
findings. Their reports are preserved as
`reviews/2026-07-16-infrastructure-candidate-{fable,opus}-pass.md`. The candidate's
three cosmetic/defensive MINOR observations are corrected in the following
documentation commit and receive a final diff re-review.

## Program milestones

| ID | Milestone | Required outcome |
|---|---|---|
| N0 | Definition and safety | Fixed end state, before-state inventory, all-ref and stash-object rollback archives, and approval boundaries |
| N1 | Curated integration line | Preserved recovery/all-ref archives, tree-equivalent curated product history, and reconciled refs/worktrees/stashes; `main` does not move yet |
| N2 | W7, quality, and promotion | W7a-W7d completed, optional candidates adjudicated, quality debt honestly normalized, and reviewed fast-forward promotion of `main` |
| N3 | Docs and tasks | Current architecture/runbooks, archived recovery-only instructions, and no ambiguous recovery task remains |
| N4 | Runtime validation | Clean hermetic gates plus isolated live daemon/session/swarm/tool/provider validation |
| N5 | Host normalization | One canonical checkout/runtime, no stale worktrees/binaries/services/sockets/config paths, and verified identity agreement |
| N6 | Final sign-off | Independent architecture and operations PASS plus reproducible final evidence and rollback anchors |

Milestones are ordered, but work may overlap when authority and mutation surfaces
do not. `main` promotion is the exit criterion of N2, after the curated recovery
tree and all reviewed W7 commits are present. It is not an N1 completion action.

## Core distinction

Recovery completion proved a bounded offline implementation branch. Normalization
is responsible for promotion, live operation, cleanup, and normal maintenance.
The recovery record remains immutable forensic evidence and is not rewritten to
make the normalized state look simpler than it was.

## Mutation rule

Read-only inventory, metadata-only host inspection, reversible branch creation,
and a fetch from the configured upstream remote may proceed autonomously. An
upstream fetch is read-only only: it may update remote-tracking refs but may not
delete refs, replay commits, merge, rebase, or push.

Moving `main`, deleting branches, removing worktrees, dropping stashes, changing
the live runtime or user-facing integrations, changing declarative Nix/home-
manager state, using real credentials, installing/updating, or publishing
requires the explicit gate stated in `COMPLETION_STANDARD.md` and a recorded
rollback path. Inventory and deletion are never combined in one step.
