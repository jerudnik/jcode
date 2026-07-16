# Phase 1 adjudication final review

- Reviewer: independent `verify` agent, read-only. No repo files, refs, worktrees, or stashes changed.
- Repo: `/Users/jrudnik/labs/jcode`, branch `recovery/2026-07-15`.
- Verified refs: HEAD `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream/master `802f6909825809e882d9c2d575b7e478dce57d3b`, merge-base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`. All three match the values stated in `RESPONSIBILITIES.md:5` and the three reports.
- Scope reviewed: uncommitted diffs to `RESPONSIBILITIES.md`, `PROGRESS.md`, `README.md`, `ORCHESTRATOR_PROMPT.md`; the three preserved reports; targeted source spot-checks of the newly minted R12/R13 claims.

## Verdict: APPROVE (with two IMPORTANT items to fix before Phase 2 dispatch)

The adjudication is internally coherent, faithfully reflects the three reports, correctly resolves the R00/R09 category dispute, keeps the six-seam cap, and correctly promotes the two responsibilities (R12 agent turn, R13 compaction) that the focused Opus review discovered and that both initial maps missed. Evidence claims I spot-checked hold up at the code level. The two IMPORTANT items are a documentation-integrity regression in `ORCHESTRATOR_PROMPT.md` and an overstated cross-seam invariant, neither of which invalidates the map.

## Integrity checks that passed

- **Report hashes match.** `shasum -a 256` of all three `reviews/2026-07-15-responsibility-*.md` files exactly equals the SHA-256 values in `RESPONSIBILITIES.md:9-11`. Links resolve to real files.
- **Refs consistent.** `RESPONSIBILITIES.md:5`, `PROGRESS.md`, and `README.md:33` agree with actual git state. `PROGRESS.md:26` correctly replaces the placeholder "this Phase 0 docs checkpoint" with the real commit `7ff4fc6be`.
- **Row count and cap.** 22 rows (R00-R13 with A/B/C/D splits), exactly 6 rows marked `` `full` `` (R01, R02, R03A, R04, R05B, R12). Matches the six-full-seam table and the stated cap.
- **Every material responsibility is anchored to observable invariants, not file buckets.** The seed R00-R11 keyword rows are gone; the "Owns and protects" column and the cross-seam invariants section (`RESPONSIBILITIES.md:59-68`) express testable contracts (one runtime identity, one provider outcome, one terminal turn record, ordered reload recovery). Index editing rule and README both restate "Files are not responsibilities."
- **R12/R13 evidence is real.** `crates/jcode-app-core/src/agent.rs:3,5,9,15,16,17` declares `mod compaction/evidence/prompting/turn_execution/turn_loops/turn_streaming_mpsc` as first-class fork modules. `report_herdr_session` and `append_session_evidence_with_correlation` exist (`agent/turn_streaming_mpsc.rs:194,223`). Compaction resets both provider-session copies (`agent/compaction.rs:7-8`, plus 163/224/268). `crates/jcode-compaction-core/src/lib.rs` exists as a fork-only crate. The gap review's claims are accurate.
- **R12 not starved or improperly absorbed.** R12 is a full seam (rank 3, 15/16), a mandatory pilot prerequisite (`prereq #2`), and the pilot question explicitly requires it to "emit one correlated request/result record" (`RESPONSIBILITIES.md:74`). Invariant #2 and #4 correctly separate R02's route selection from R12's recorded identity and R06A's persistence. This directly implements the gap review's warning not to let R02 silently own emission. Good.
- **R04/R05B and R01/R03A boundaries are coherent.** R04 owns process/detached-task lifecycle and terminal state; R05B owns assignment/spawn-mode/reclaim/retry. Invariant #5 ("one liveness authority per layer") and the adjudicated-disagreements entry keep them distinct while requiring joint incident validation. R01 owns build/reload identity meaning; R03A owns wire carriage and compatibility verdict; invariant #1 and #6 bind them without collapsing either. These resolutions correctly follow the critic's finding-4/finding-9 concerns while rejecting the critic's over-merge (merging R03 into R01), which is defensible and documented.
- **R00/R09 to mandatory light overlays is safe.** The critic's category objection (reports section 1-3) is explicitly accepted. Both remain `required` pilot prerequisites (`prereq #1`, `#4`) and appear in invariant #8 (debt follows behavioral ownership) and the pilot stop conditions. Downgrading review depth does not remove their gating force; it only frees full-review slots, which is exactly the critic's intent. Safe.
- **Pilot prerequisites are sufficient and bounded.** The seven prerequisites cover every full-seam prerequisite that the pilot question actually exercises (R01/R02/R03A/R12 full ledgers, R06A round-trip, R09 gates, R07C reporting-off, R13 compaction-writer enumeration). The stop-conditions paragraph (`RESPONSIBILITIES.md:86`) bounds scope against real credentials, payment, publication, live daemon, tools, memory, UI, baseline updates, and unowned identity writers. This is materially tighter and better-scoped than either the Mapper's or Critic's pilot question.

## IMPORTANT findings (fix before Phase 2 dispatch)

### I1. `ORCHESTRATOR_PROMPT.md` dropped a safety rule and left a malformed list

`ORCHESTRATOR_PROMPT.md:55-57` (diff): rule 11 was deleted, leaving item 10 followed by two blank lines and no item 11. The removed rule was:

> "Ask the user only before irreversible, destructive, security-sensitive, or external publication actions. Local branches, worktrees, commits, tests, and reversible changes are authorized."

This is a substantive safety/authority boundary, not cosmetic. Its removal is not mentioned or justified anywhere in `PROGRESS.md`, `RESPONSIBILITIES.md`, or the adjudicated-disagreements section, and it is unrelated to Phase 1 responsibility mapping. This is out of scope for a Phase 1 docs checkpoint and silently weakens the recorded operating contract. It also leaves a broken numbered list (10 then blank). Recommend restoring rule 11 (or, if intentional, recording the rationale in an append-only note). The two additive edits in the same file (the "third way" clause at line 11 and the "more performant... than either upstream or fork" closing line) are stylistic mission edits, also out of the stated Phase 1 scope, but harmless; the deletion is the concern.

### I2. Cross-seam invariant #3 asserts exactly two provider-session co-writers; the tree has many reset sites

`RESPONSIBILITIES.md:63` states model changes in R02 and compaction completion in R13 "are co-writers of provider-session identity ... with no undiscovered third writer left unreviewed." A `grep` for `provider_session_id = None` in non-test code returns 13 distinct files, including `crates/jcode-app-core/src/agent/turn_execution.rs:189,197`, `overnight.rs:188`, `server/client_actions.rs:698`, and numerous TUI sites (`model_context.rs`, `conversation_state.rs`, `commands_review.rs`, `commands.rs`, `inline_interactive.rs`). Many are plausibly the same model-switch/new-session logical writer surfaced through R02, but `turn_execution.rs` and `overnight.rs` (a background task, i.e. R04) are additional reset points inside the agent/background layer, not obviously R02 or R13.

This does not break the map: pilot prerequisite #6 (`RESPONSIBILITIES.md:83`) already requires R13 to "enumerate every writer of provider-session identity," which is the correct mitigation. But invariant #3's flat "two co-writers" wording is stronger than the evidence and mildly contradicts prereq #6's own hedge. Recommend softening invariant #3 to "at least two known co-writers (R02 model switch, R13 compaction); R13's ledger must enumerate all writers including agent-turn and background-task reset sites" so the invariant and the prerequisite agree. The gap review itself flagged (report section "Explicit confidence and gaps") that "a third writer is possible," so the doc currently overstates relative to its own cited source.

## minor findings

- **m1. Score attribution for R03A silently changed from the sources.** `RESPONSIBILITIES.md:54` gives R03A 14/16; the Mapper gave R03A 13/16 (report line 445, "13/16") and Opus did not rescore it. The adjudication does not note the +1 revision or its basis. Not wrong to re-score during adjudication, but per record rule 9 ("never erase disagreement") a one-line rationale for the delta would be cleaner. Similarly R04 is 13/16 here vs the Mapper's R04 12/16; undocumented delta.
- **m2. R06A pilot label uses "smoke prerequisite" in the table but prereq #3 requires a full round-trip.** `RESPONSIBILITIES.md:27` says "smoke prerequisite"; `prereq #3` (line 80) requires R06A to "round-trip the minimal evidence fixture." A round-trip is stronger than a smoke check. Minor wording tension, not a contradiction.
- **m3. `PROGRESS.md` blocker list names only R12 as the newly discovered responsibility** (line 40-43) and omits R13, though R13 was discovered by the same gap review and carries the more subtle multi-writer invalidation risk (invariant #3 / I2 above). Consider adding R13 to that blocker note.
- **m4. The state-vocabulary line "Every row is `mapped`" (`RESPONSIBILITIES.md:42`)** is consistent with README's `seed -> mapped -> ...` lifecycle, good. No issue; noted as verified.

## What I did NOT check (explicit gaps)

- No fork-vs-upstream symbol-level semantic diff. I confirmed the R12/R13 fork modules and reset sites exist, not that they are behaviorally superior or that upstream lacks equivalents.
- I did not re-derive any 16-point score; I only checked internal consistency of scores against the three reports.
- I did not read `BASELINES.md`, `PRESCREEN.md`, `QUALITY_GATES.md`, or `SEAM_LEDGER_TEMPLATE.md` in full, so line-cited evidence in the reports pointing into those files was not independently re-validated beyond the refs and hashes.
- I did not run builds or tests (not required, and the pilot is deferred to Phase 3).
- I did not exhaustively classify all 13 `provider_session_id` reset sites into R02 vs R04 vs R13; I confirmed the count exceeds two and that at least the agent-turn and background-task sites are non-obvious, which is enough to support I2.

## Confidence

- High: hash/ref/link integrity, six-seam cap arithmetic, R12/R13 existence and module structure, R00/R09 overlay safety, coherence of the four contested boundaries, the ORCHESTRATOR_PROMPT rule-11 deletion (I1).
- Medium-high: sufficiency and boundedness of pilot prerequisites; the invariant-#3 overstatement (I2) is high-confidence as a factual mismatch, medium on whether the extra sites are logically distinct writers.
- Medium: score-delta findings (m1) since adjudication may legitimately re-score.

## Bottom line

Approve the Phase 1 adjudication. The map is evidence-anchored, correctly caps six full seams, correctly promotes the two omitted responsibilities, and bounds the pilot well. Before dispatching Phase 2 seam teams, restore or justify the deleted safety rule in `ORCHESTRATOR_PROMPT.md` (I1) and reconcile invariant #3's "two co-writers" wording with prerequisite #6's enumeration requirement (I2). Neither blocks approval; both are cheap doc fixes.
