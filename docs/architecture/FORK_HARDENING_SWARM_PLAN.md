# Fork Sustainability — Adversarial Swarm Hardening Analysis (Plan)

Date: 2026-07-08
Status: Executing.
Goal: Stress-test the *current* state of the fork-sustainability approach against
the committed `FORK_SUSTAINABILITY_MODEL.md`, where practice has moved fast and
may have pragmatically diverged from the written plan. Produce an analysis plus
concrete hardening recommendations.

## Why a swarm (adversarial groups)

A single reviewer converges too easily on the model's own framing. We instead run
**three groups with opposing mandates against each other**, then adjudicate. Each
group must cite ground truth (commits, files, scripts, measurements), not opinion.

## Groups

### Group RED — "The model has drifted and is under-hardened"
Mandate: assume the pragmatic velocity since 2026-06-29 has silently violated or
outgrown the committed model. Find where practice contradicts the doc, where the
"two cheap changes" have quietly grown, where new invasive edits crept in, and
where the fork is now more fragile than the doc claims. Produce a ranked list of
the strongest failure modes with evidence.

### Group BLUE — "The drift was correct; the model held"
Mandate: assume the divergence from the written plan was the right pragmatic call.
Show where reality validated the cut-down model, where deferred machinery was
correctly never needed, and argue the minimal hardening that is actually justified.
Rebut RED's strongest points with evidence.

### Group GREEN — "Ground truth referee"
Mandate: no priors. Measure the real current divergence surface (files with
deletions vs additions, new invasive edits since 2026-06-29, rerere cache health,
doctor/NS coverage, CI rail health, patch-ledger freshness). Adjudicate RED vs
BLUE claim-by-claim with numbers. Owns the final scoreboard.

## Rounds

1. **Position** (parallel): RED, BLUE, GREEN each produce an evidence-backed brief.
2. **Rebuttal** (parallel): RED and BLUE each answer the other's brief + GREEN's
   measurements.
3. **Synthesis** (coordinator): fold into a single ranked hardening backlog with
   severity, cost (cargo/script/nix ladder), and a keep/cut verdict per item.

## Verification / done-state

- Every claim references a commit hash, file path, script, or measured number.
- GREEN's scoreboard is reproducible (commands recorded).
- Output: `docs/architecture/FORK_HARDENING_FINDINGS.md` with a ranked, costed
  hardening backlog and an explicit "model still holds / needs amendment" verdict.

## Ground rules for workers

- Read-only investigation. No code edits, no commits. Report findings only.
- Cite evidence: `git log`, file paths, line counts, script output.
- Frugal: `git`, `rg`, `cargo check` at most; no heavy nix/release builds.
- Scope: the fork-sustainability system only (model doc, rerere, doctor/NS,
  seam audit, patch-ledger, CI rails, sync scripts).
