# W0 Record-Consistency Closure — Independent Adversarial Mechanical Review

- **Reviewer role:** independent adversarial mechanical reviewer (verify), read-only.
- **Repo:** /Users/jrudnik/labs/jcode
- **Commit under review:** `11a78a858f14a2722f67efdaefc3025360dc19c6` ("docs: Close W0 record consistency.")
- **Base:** `53faa62ccc190c315df3e305fa3bccc9b0479727` ("docs: Preserve architecture gate approval.")
- **Relationship:** base is the direct parent of the reviewed commit (`git log` confirms `11a78a858` -> `53faa62cc`).
- **Requirements source:** `docs/fork/recovery/RECOVERY_PLAN.md` at base (W0 spec at §11 gate, W0 workstream at lines 81-84, inconsistency inventory at lines 68-73).

## Verdict: **PASS**

**Confidence: High.**

The commit is a pure append-only, docs-only closure that discharges exactly the nine ledger inconsistencies W0 was authorized to close, at their independently reviewed boundaries, citing byte-exact hashes, without erasing any historical prose, changing source/tests/baselines, or authorizing deferred/live/external behavior. The one plan omission it self-reports (section-2-vs-W3 R04 follow-up count) is a bounded record-consistency correction and does not require architecture reopening. W1/W2 are safe to start.

---

## Point-by-point findings

### 1. Append-only, docs-only, historical text intact — PASS
- `git diff --stat 53faa62..11a78a8`: 13 files changed, **83 insertions(+), 0 deletions**. All 13 paths are under `docs/fork/recovery/`.
- `git diff | grep '^-' (excluding '---')`: **empty** — zero content deletions. Every change is a dated `##`/`###` append below existing prose.
- Historical pending lines physically survive: `grep -rn 'approval:.*pending|pending independent'` still returns the original "Coordinator approval: pending" / "Fable review: pending" lines in R00, R03B, R05A, R07A, R08A, R09, R10, R11 (e.g. R03B:63-64, R05A:51-52). W0 supersedes them by dated amendment, not deletion, matching plan lines 68-70 ("append-only ledger amendment in W0, not a re-review").
- Nine ledgers touched (R00, R03B, R04, R05A, R07A, R08A, R09, R10, R11) plus four rollup docs (PROGRESS, README, RECOVERY_PLAN, RESPONSIBILITIES) — exactly the W0 owner/touch set named at plan line 83.

### 2. R04 superseding narrow source-review PASS — PASS
- R04 ledger amendment cites both reviews with hashes that reproduce byte-exact:
  - Opus PASS `7a8f24490806a6aa30bf4d16947a6e4ff2fee76c67589972fcadc0d96fb1a9de` ✓ (matches `reviews/2026-07-15-r04-marker-fix-opus-review.md`).
  - Fable "PASS for the narrow R04 pilot prerequisite, with IMPORTANT follow-ups" `1ec0ceb5c333da18c814ba96a9392fd6fad398b6e3df9b00aafd0c1ee902f73d` ✓ (matches `reviews/2026-07-15-r04-marker-fix-fable-review.md`).
- Scope is held narrow: "approved and integrated for the strict no-reload/no-resume/no-cancel/no-background path. R04 remains blocked before lifecycle widening." Reload/resume/cancellation/detached/background and wait-like semantics are explicitly "separately gated." No lifecycle widening is authorized.
- All **three** Fable IMPORTANT follow-ups are carried verbatim in substance and map exactly to the source review:
  1. `cleanup_client_connection` distinguishable partial-cleanup outcome == Fable IMPORTANT-1 (`Ok(())` ambiguity, review lines 22-33).
  2. replaced-active/unchanged-streaming, unchanged-active/replaced-streaming, both-replaced fixtures with exact removal booleans == Fable IMPORTANT-2 (lines 35-48, 93).
  3. blocking PID-marker lock liveness edge bounded before latency-sensitive widening == Fable IMPORTANT-3 (lines 48-59, 85).
- No scope weakening: none of the three is downgraded or dropped; each is stated as a carried, still-open obligation routed to W3.

### 3. Five light ledgers approved only at reviewed boundaries — PASS
- `reviews/2026-07-15-remaining-light-ledgers-opus-review.md` reproduces to `b537bc5674fdb9385e60c2dd18a44db5e61ba4f57146cd57fbf91f7a58a8a55d` ✓ (all five amendments cite this exact hash).
- Verdicts in that artifact match each amendment's disposition and boundary:
  - R03B: PASS, `retain-fork` conditional; amendment approves "only the narrow Unix attach/cleanup contract," WebSocket/mobile + every escalation trigger deferred. The 20/20 + 1/1 claim is corroborated by `PROGRESS.md:32` and `:43` and `seams/README.md:19` ("isolated Unix socket 20/20 and live-attachment 1/1 fixtures pass"). No overclaim.
  - R05A: PASS `retain-fork`; amendment authorizes no source/swarm exercise; two app-core integration tests preserved as W2 entry criteria (consistent with review's checkpoint-6 verification-gap note).
  - R07A: PASS `retain-fork` fail-closed; amendment approves no tool/MCP/discovery/network/credential/daemon exercise; no-server fixture + escalations deferred.
  - R08A: PASS `defer` only, no-UI boundary; amendment explicitly withholds dangerous onboarding consent, keeps timeout/Escape fail-closed, requires the mandated full R08A/R02 joint review before W5 source/credential work.
  - R10: PASS `compose` no-network; amendment authorizes no workflow edit/tag/release/download/install/updater/profile/daemon/publication action.
- No accidental authorization of deferred/live/external behavior in any of the five.

### 4. R00/R09/R11 stale Fable-pending discharge — PASS
- All three cite the corrected Fable plan hash `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` ✓ (matches `reviews/2026-07-15-phase4-fable-corrected-plan.md`) and the independent Opus plan-review hash `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92` ✓ (matches `reviews/2026-07-15-phase4-plan-opus-review.md`).
- Discharge basis is legitimate: these ledgers were Fable-authored and could not self-approve; the named independent review is the Phase 4 architecture gate (plan line 70), which returned PASS. This is the exact discharge mechanism the plan reserved for W0.
- Overlay obligations remain active: R00 "remains active until Phase 6 ... does not retire fixed-ref, provenance, prompt, stash, worktree, or no-broad-sync obligations"; R09 "retain-fork quality/debt overlay" binds; R11 "remains active through Phase 6." Correct.

### 5. 60-vs-61 production-size reconciliation — PASS
- Neither count erased. `QUALITY_GATES.md:26` still records the historical `60 violations` at the Phase 0 head. R09 ledger line 69 records `61 findings` with **Expected exit 1 / Actual exit 1** at the fixed G0 HEAD; line 76 states "No command used `--update`."
- The W0 R09 amendment states both truths, ties `60` to the Phase 0 snapshot and `61` to fixed G0/G4 heads with unchanged `1/1` exit, and explicitly says neither is "rewritten or treated as green" and "no command used `--update`." Matches plan inconsistency item at line 71 ("both preserved append-only ... any seam citing 60 must be read as historical").
- Expected-red status preserved: R09 debt remains visibly red; §4 deferred table (base plan) lists prod-size 61 as attributed non-blocking debt.

### 6. RECOVERY_PLAN §12 amendment fixing the section-2-vs-W3 omission — PASS (bounded correction, no reopening)
- The discovered omission is real and reproduced: base plan **section 2** (line 68) enumerates **three** R04 Fable IMPORTANT follow-ups (disconnect `Ok(())` ambiguity; marker-lock liveness; streaming-marker partial-outcome coverage). Base **W3** (line 108) names only **two** ("give `cleanup_client_connection` a distinguishable outcome ... and document/bound the marker-lock liveness edge"). The third (streaming-marker partial-outcome fixtures) was dropped from W3's enumeration.
- The §12 amendment adds exactly that third item to W3 acceptance with precise fixture cases and `SessionPidMarkerRemoval` booleans, and cites the Fable review hash `1ec0ceb5...` for verification (reproduces byte-exact).
- **Decision: this is a bounded record-consistency correction, not architecture reopening.** Justification: (a) the fixture was already promised by section 2 and by the byte-exact Fable review, so no new obligation is invented; (b) W3 is already a fixture-dominant fixture-ladder workstream, so the item lands in its native home with no new workstream, dependency edge, or concurrency change; (c) the amendment explicitly states it "does not widen the strict pilot, alter R04 ownership, or authorize a live lifecycle test," and the fixtures are deterministic/offline. No architecture invariant, disposition arithmetic, or gate posture changes.

### 7. Rollups agree with ledger state; no premature W0-complete claim — PASS
- `grep -niE 'W0 complete|W0 done|W0 closed|W0 finished'` across README/PROGRESS/RESPONSIBILITIES: **none.**
- PROGRESS amendment explicitly ends "W0 remains pending independent mechanical review." README amendment: "independent mechanical review is pending." RESPONSIBILITIES amendment scopes to "docs/evidence closure only." All consistent with ledger amendments and with `seams/README.md` integrated-ledger dispositions (R03B/R05A/R07A `retain-fork`, R08A `defer`, R10 `compose`).

### 8. Preservation state — PASS
- Sole dirty path: `git status` / `git diff --stat` shows only `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` modified (3 insertions, 2 deletions).
- Prompt diff SHA-256: `git diff ORCHESTRATOR_PROMPT.md | shasum -a 256` = `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00` ✓ (exact match).
- Four stashes present: `stash@{0}`..`stash@{3}` (scorpion config hot-path parts + wip before upstream sync).
- Commit is docs-only: all 13 changed files under `docs/fork/recovery/`; no `crates/`, `scripts/`, or config paths touched. HEAD == reviewed commit.

### 9. Missed stale approval / W0-scope contradiction that would make W1/W2 unsafe — none found — PASS
- The only remaining "OPEN/pending" line inside a touched ledger not converted by W0 is `R00:68` "Pilot authorization remains OPEN pending independent G2 adjudication." This is a **pilot gate**, not an approval/review-pending record in W0's scope (plan line 83 lists only the R04 tail, five light approval lines, R00/R09/R11 Fable-pending lines, and the 60/61 note). Leaving it open is correct, not a miss.
- W1/W2 prerequisites are satisfied and unblocked: plan line 225 ("No blocker prevents starting W0 immediately, then W1/W2"); W2 entry criteria (R05A app-core tests) preserved unchanged by the R05A amendment; R04 marker fix (W2 prerequisite) integrated and independently PASS-reviewed. No contradiction introduced.

---

## Nonblocking observations
1. Because the closure is strictly append-only, each superseded "pending" line still physically precedes its amendment. A casual reader scanning only the historical block could misread current status; the dated amendments resolve this, and append-only discipline is itself a required W0 acceptance property, so this is correct-by-design, not a defect.
2. R05A's amendment inherits the reviewer's checkpoint-6 verification gap (jcode-plan 79/79, control_log 2/2 not independently re-run). The amendment correctly authorizes no swarm exercise and preserves the app-core tests as W2 entry criteria, so the gap is carried, not hidden. Worth surfacing before any W2 R05A-touching change.
3. The §12 W3 acceptance now enumerates three follow-ups while W3's prose at line 108 still literally says "the two carried Fable IMPORTANT follow-ups." The §12 append reconciles this by supersession (same append-only pattern as the ledgers); a future editor should read line 108 as amended by §12. Nonblocking.

---

## Commands / evidence
```
git log --oneline -1 11a78a858            # docs: Close W0 record consistency.
git log --oneline -1 53faa62cc            # parent/base
git diff --stat 53faa62..11a78a8          # 13 files, 83 insertions(+), 0 deletions
git diff 53faa62..11a78a8 | grep '^-' | grep -v '^---'   # empty (append-only)
git diff --stat                           # only ORCHESTRATOR_PROMPT.md dirty
git diff docs/.../ORCHESTRATOR_PROMPT.md | shasum -a 256  # 8e8e6a92...c00
git stash list                            # 4 stashes
shasum -a 256 reviews/2026-07-15-r04-marker-fix-opus-review.md          # 7a8f2449...
shasum -a 256 reviews/2026-07-15-r04-marker-fix-fable-review.md         # 1ec0ceb5...
shasum -a 256 reviews/2026-07-15-remaining-light-ledgers-opus-review.md # b537bc56...
shasum -a 256 reviews/2026-07-15-phase4-plan-opus-review.md             # 3f2d31cb...
shasum -a 256 reviews/2026-07-15-phase4-fable-corrected-plan.md         # b0bae980...
grep -n 'Expected exit' seams/R09-quality-gates/ledger.md               # prod-size 1/1, 61 findings
grep -niE 'W0 complete|W0 done|W0 closed' README.md PROGRESS.md RESPONSIBILITIES.md  # none
```
All five cited artifact hashes reproduced byte-exact. Base plan section 2 (line 68) names three R04 follow-ups; base W3 (line 108) named two; §12 adds the third.

---

## Scope limits
- Read-only. No repository file modified. No source build, Cargo/Nix, or test executed. No network, credentials, daemons, MCP, releases, or external actions used.
- Verification is documentary/mechanical: hash reproduction, git diff/status facts, and cross-reference of ledger/plan/review text. I did not execute the R09 gate scripts, the R03B/R04 fixtures, or any Cargo test, so the underlying 20/20, 1/1, 61-finding, and 3,074/2,987 numbers are verified as *consistently recorded and internally reconciled*, not independently re-measured.
- I did not audit source code behavior of the R04 marker fix beyond confirming the cited review artifacts exist, hash-match, and support the ledger's narrow-scope claims.
- This review covers only W0 record-consistency scope. It grants no source, pilot, or external-action authorization; every later workstream remains subject to its own gates.
