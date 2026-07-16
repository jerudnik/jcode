# Fable architecture and completeness review: post-recovery normalization drafts

- Date: 2026-07-16
- Reviewer: Fable-class read-only architecture review (verify agent)
- Scope: working-tree drafts `docs/fork/normalization/{README,BASELINE,DEFINITION_OF_DONE,NEXT_SESSION_PROMPT}.md`, plus `docs/fork/recovery/reviews/2026-07-16-w7-review.md` and `docs/fork/SYNC_MODEL.md`
- Repo state at review: branch `recovery/2026-07-15`, HEAD `cdc2cc2b4cea51c185de330c8e15e08615acc46c`
- No file, ref, worktree, stash, service, or host state was modified.

## Verdict: FAIL (bounded corrections required before the coordinator prompt is safe to run)

The program design is strong: milestones N0-N6 map cleanly onto gates D0-D9, the
archive-then-curate history strategy avoids rewriting the recovery record, the
mutation rule separates inventory from deletion, and the "core-runtime validated"
vs "fully runtime validated" labeling honestly bounds the provider gate. The W7
split (W7a-W7d) faithfully tracks the committed W7 review, including the 11 R12
fixtures, the typed-predicate-before-status-normalization ordering, and the
explicit dormancy adjudication for R03A/R02. Most baseline facts reproduce
exactly (HEAD, `main`, `vendor/upstream`, 0/166 ahead-behind, 4 stashes, 29
worktrees, prompt diff SHA-256 `8e8e6a92da...`).

However, one self-consistency defect will fire the drift-stop rule at the very
first command of the next session, and several gates are underdefined enough
that D9 could be "passed" in materially different end states.

---

## CRITICAL

### C1. The program's authority files are untracked, unhashed, and contradict their own baseline

- Verified live: `git status --short` shows **two** entries:
  `M docs/fork/recovery/ORCHESTRATOR_PROMPT.md` **and** `?? docs/fork/normalization/`.
- `BASELINE.md:19` claims "Working-tree status | Only `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` modified".
- `NEXT_SESSION_PROMPT.md:42` claims "Sole dirty path: docs/fork/recovery/ORCHESTRATOR_PROMPT.md".
- `NEXT_SESSION_PROMPT.md:52-53` orders: "If any starting fact differs, stop mutation, preserve the observation, explain the drift..."

Consequences:
1. The coordinator's mandatory first reproduction (`BASELINE.md:81-89`) immediately
   contradicts two recorded starting facts (working-tree status, and after any
   commit of these docs, the recorded HEAD `cdc2cc2b4`). The session opens in
   declared-drift mode caused solely by the program's own files.
2. `DEFINITION_OF_DONE.md` D0 demands immutable hashes and rollback for
   everything, yet the binding definition itself exists only as untracked
   working-tree bytes with no commit, no hash, and no rollback anchor. A crash,
   accidental `git clean`, or worktree confusion silently destroys the program's
   authority.

**Bounded correction:** before the next session, commit the four normalization
docs as a single docs-only commit on `recovery/2026-07-15` (this is within the
README's autonomous "read-only inventory and reversible" envelope since it is
additive and does not touch source, refs, or the preserved prompt edit). In the
same change, update `BASELINE.md` and `NEXT_SESSION_PROMPT.md` starting facts to
name the new head and restore "sole dirty path = ORCHESTRATOR_PROMPT.md" as a
then-true statement. Alternatively, if the docs must remain uncommitted, add an
explicit line to both files: "the untracked `docs/fork/normalization/` directory
is expected and is not drift," plus recorded SHA-256 hashes of all four files.
The commit option is strictly better.

---

## IMPORTANT

### I1. N1/N2 ordering circularity around `main` promotion and W7

- `DEFINITION_OF_DONE.md` D2 requires tree equivalence "between the approved
  recovery-plus-W7 tree and canonical `main`", and `main` moves "only by reviewed
  fast-forward to the curated integration line."
- `NEXT_SESSION_PROMPT.md` N1.2 says the curated branch carries the recovered
  tree "then add W7 and normalization changes", but N1.5 gates `main` promotion
  inside N1, while W7 is N2.

As written, N1 cannot close (promotion requires the recovery-plus-W7 tree) until
N2 completes, yet the milestone table (`README.md`) presents N1 before N2 as if
sequential. A coordinator could defensibly promote `main` at the pre-W7 tree and
still claim D2, or defensibly refuse to close N1 for days.

**Bounded correction:** one sentence in both DoD D2 and the prompt: "`main`
promotion is the exit criterion of N2, not N1. N1 ends when the curated branch
carries the tree-equivalent recovered product tree; W7a-W7d land as reviewed
commits on that branch before promotion." Or, if two-stage promotion is
intended, state explicitly that D2 equivalence is measured twice (recovery tree
at first promotion, recovery-plus-W7 tree at second).

### I2. Remote end state is undefined, so D9 can pass in two different worlds

- D2/N4.4 correctly forbid pushes without separate explicit instruction.
- But no D1/D9 gate states what the remote (`origin`/`github` =
  `jerudnik/jcode`, verified live) must look like at sign-off. The fork can be
  declared "done" with `origin/main` 166+ commits stale and the recovery archive
  existing only locally, which undermines both the rollback story (single-host
  archive) and "well-organized."

**Bounded correction:** add one D9 checklist line: "The final handoff states the
exact remote state and disposition: either an explicitly authorized push of
`main` and the archive ref, or a labeled `local-canonical, remote pending`
status with the risk recorded." No new scope, only forced explicitness.

### I3. Evidence placement contradicts the clean-history gate

- D2 forbids "evidence-only churn that obscures product history" on canonical
  `main`.
- D9 requires a reproducible "final evidence package"; the prompt (N6.1) requires
  byte-hashed evidence; the safety rules require preserving every failed attempt
  append-only.

Nothing says where normalization evidence lives. A coordinator could commit
hundreds of evidence files to `main` (violating D2's spirit) or leave them
uncommitted (violating D0's immutability), and argue compliance either way.

**Bounded correction:** one sentence in D2 or D9: "Normalization evidence lives
under `docs/fork/normalization/evidence/` in a bounded number of dedicated
`docs(evidence)` commits at the top of the curated stack (or on the archive
ref), and this is the enumerated exception to the evidence-churn prohibition."

### I4. Baseline omits ref inventories that D1 later requires decisions on

Verified live: **40 local branches** and **138 tags** exist. D1 requires "every
local branch and tag has a documented keep/archive/delete decision", and the
baseline warns that non-ancestor tips are not disposable, but `BASELINE.md`
records neither branch tips, branch count, tag count, nor the identity of the
four stashes (identities exist elsewhere, e.g.
`reviews/2026-07-15-phase4-plan-opus-review.md:84`). Drift in these surfaces
between now and session start is therefore undetectable against this baseline.

**Bounded correction:** append to `BASELINE.md`: branch count (40), tag count
(138), the four stash subjects, and add `git for-each-ref` to the required
reproduction block. Purely additive.

### I5. The D4 "full trusted matrix" is not concretely defined

D4 names thirteen gate categories but cites no command set. The recovery record
has an exact definition (`docs/fork/recovery/QUALITY_GATES.md`, the Phase 6
accepted driver with 62 checks and four expected-red rows). Without a pointer,
"full matrix" is unmeasurable and can be narrowed silently.

**Bounded correction:** one line in D4: "The matrix is the command set defined in
`docs/fork/recovery/QUALITY_GATES.md` and the Phase 6 accepted driver, minus
recovery-scoped checks that are explicitly enumerated as retired." This also
anchors the expected-red baseline (panic 46/31, swallowed 3,077/2,987, prod-size
60, test-size 31) that D4's debt-register gate needs exact counts for.

---

## LOW

### L1. Sandbox/live daemon collision is not gated
D6 requires disposable `JCODE_HOME`, but no check requires proving the sandbox
daemon's socket/port/marker paths are disjoint from any currently live user
daemon before start. Add one D6 bullet: "pre-start check proves no shared
socket, port, pid, or marker path with any live daemon."

### L2. Reviewer independence for D9 is compromised by the staffing rule
The prompt assigns "Fable for architecture/investigation" during execution and
D9 requires an "independent architecture reviewer." Add: "D9 reviewers must not
have authored or steered any normalization implementation lane."

### L3. `vendor/upstream` refresh implies a network fetch of unstated authority
N3.4 says eliminate the stale-`vendor/upstream` footgun; nothing classifies
`git fetch upstream` as autonomous-read-only or approval-gated. Classify it
explicitly (recommend: allowed read-only, no ref deletion, no replay).

### L4. W7d retention policy owner is unstated
The W7 review says the bound is "a policy decision, not a casual truncation
patch" and proposes 2 KiB. DoD/prompt neither adopt the number nor say who
approves. State: coordinator proposes the exact bound, user or independent
reviewer approves before W7d merges.

### L5. Multi-session resumption is implicit
The prompt says "Persist... Do not stop at a plan" but the program plainly spans
sessions. Add: "Every new session re-runs the BASELINE reproduction block and
appends the observation before resuming mutation."

### L6. Curated-stack commit buildability unspecified
D2's "small logical stack" has no per-commit build requirement. Either require
each curated commit to build, or explicitly waive bisectability with rationale.

---

## What already passes review (no change needed)

- Archive-before-curate, no-rewrite-of-recovery-line strategy (D2, N1.1-1.3) is
  the correct low-risk history design given 166 linear commits and `main` as
  ancestor (verified live).
- Fast-forward-only `main` movement plus tree-equivalence proof is a sound,
  measurable promotion gate.
- W7a-W7d scope, ordering, fixture requirements, and R03A/R02 adjudication match
  the committed W7 review exactly, including the 11 R12 fixtures and the
  "typed predicate before status normalization" dependency.
- The mutation rule (inventory never combined with deletion; approval packets
  only when destructive actions are ready) is consistent between README, DoD D0,
  and the prompt.
- SYNC_MODEL alignment: D1's "stale vendor/upstream cannot masquerade" and
  N3.4's "without broad replay" correctly preserve monitored curation.
- Honest-labeling rule ("core-runtime validated" fallback) prevents the most
  likely overclaim.
- Preserved prompt edit handling (hash-pinned, user-controlled disposition)
  matches the standing R11/R00 ledger contract.

## Summary

| ID | Severity | Finding | Correction size |
|---|---|---|---|
| C1 | CRITICAL | Authority docs untracked; baseline contradicts live status; drift rule self-fires | 1 commit + 2 fact updates |
| I1 | IMPORTANT | N1/N2 circularity on `main` promotion vs W7 | 1-2 sentences |
| I2 | IMPORTANT | Remote end state undefined at D9 | 1 checklist line |
| I3 | IMPORTANT | Evidence placement vs clean-history contradiction | 1 sentence |
| I4 | IMPORTANT | No branch/tag/stash-identity inventory in baseline (40 branches, 138 tags live) | additive baseline rows |
| I5 | IMPORTANT | D4 matrix unmeasurable without a command-set pointer | 1 line |
| L1-L6 | LOW | isolation check, reviewer independence, fetch authority, W7d owner, resumption rule, commit buildability | 1 line each |

Re-review after C1 and I1-I5 are applied should be quick and is expected to PASS.
