# Fork recovery workspace

> **Archived forensic record.** Recovery completed on 2026-07-16. The accepted
> implementation head is `51168d16e9c708ae4afff09a6fc6402642d17782`, and the
> joint-signoff head is `17586246afb11cd54e1db12a0beec05fd29a0612`. The
> recovery record was imported into the curated history by `c786be6c3`; those
> recovery commits are preserved as historical branch identity rather than as
> ancestors of current `main`. The dated status paragraphs below remain
> append-only history. Use [`../normalization/STATUS.md`](../normalization/STATUS.md)
> for the current source, runtime, soak, and cleanup checkpoint.

Previous status checkpoint: Phase 2 evidence ledger complete; Phase 3 bounded pilot remains blocked with two prerequisite nodes after independently verified R04 and R12 integration, 2026-07-15.

Current status amendment, 2026-07-15: the previously named strict prerequisite-node count is zero after sequential R02 and R01/R03A integration and combined validation. This is not a pilot PASS. Pilot authorization remains **OPEN, pending independent G2 adversarial adjudication**, and G2 may inject new blockers.

This directory is the durable source of truth for the fork recovery. It records responsibility boundaries, fork/upstream evidence, decisions, implementation slices, validation, and unresolved questions.

## Authority order

When sources disagree, use this order:

1. Current source code, tests, runtime observations, and refreshed Git refs.
2. Reproducible commands and evidence recorded in a seam ledger.
3. Approved decisions in this directory.
4. Older fork, architecture, maintenance, and strategy documents.

Older documents remain useful evidence, but their status labels and measurements are not automatically current.

## Files

- [`BASELINES.md`](./BASELINES.md): append-only divergence and runtime baselines with reproduction commands.
- [`PRESCREEN.md`](./PRESCREEN.md): Phase 0 mechanical divergence/risk ranking and explicit unknowns.
- [`QUALITY_GATES.md`](./QUALITY_GATES.md): repaired gate semantics, exact red debt, historical attribution, and independent review links.
- [`RESPONSIBILITIES.md`](./RESPONSIBILITIES.md): adjudicated behavior/governance boundaries, six full-review seams, cross-seam invariants, and bounded pilot prerequisites. The coordinator alone edits this file during parallel work.
- [`SEAM_LEDGER_TEMPLATE.md`](./SEAM_LEDGER_TEMPLATE.md): required structure for one authoritative seam record and its two independent reviews.
- [`PROGRESS.md`](./PROGRESS.md): phase gates, checkpoints, and blockers. The coordinator alone edits this file.
- [`ORCHESTRATOR_PROMPT.md`](./ORCHESTRATOR_PROMPT.md): historical launch prompt
  for the completed recovery session. Its preserved local user edit is not
  current repository authority and must not be reused without revalidation.
- [`seams/README.md`](./seams/README.md): seam-directory ownership and merge-back rules.
- [`reviews/`](./reviews/): preserved independent reviews for cross-cutting recovery changes.
- [`evidence/`](./evidence/): byte-exact validation, R09, build-identity, and infrastructure evidence with SHA-256 manifests.
- `seams/<ID>-<slug>/`: one directory per reviewed seam. A full review contains `opus-review.md`, `grok-review.md`, and `ledger.md`.

## Baseline to revalidate at session start

The exact pre-scaffold snapshot and reproduction commands live in [`BASELINES.md`](./BASELINES.md). The last code commit before these recovery documents was `3d80eaf343e690aaa8b428d0b3ed6de64b7464d0`. At that point the fork had 286 fork-only commits, upstream had 246 upstream-only commits, and 406 files had changed on both sides since merge base `631935dd1`.

The recovery documentation commit makes HEAD and commit counts differ immediately. The refreshed Phase 0 measurements, parser correction, stale-ratchet tightening, branch topology, gate verdicts, and Phase 1 responsibility adjudication are recorded in [`BASELINES.md`](./BASELINES.md), [`QUALITY_GATES.md`](./QUALITY_GATES.md), [`RESPONSIBILITIES.md`](./RESPONSIBILITIES.md), and [`PROGRESS.md`](./PROGRESS.md). Never replace the older snapshot.

## Record rules

1. The responsibility index stays brief. Detailed evidence belongs in seam directories.
2. Every material claim cites a commit, path and symbol, test, issue, incident, or reproducible command.
3. Independent reviewers work without reading each other's conclusions. Discussion begins only after both reviews are filed.
4. Triage every seam mechanically before assigning review depth. Use full review for at most six high-risk or highly contested seams, a lightweight ledger for low-risk seams, and `defer` for work without current recovery value.
5. Set and record a research budget for every full seam. Exceeding it blocks or narrows the seam instead of silently expanding the program.
6. A decision must distinguish behavioral ownership from code location. Files are not responsibilities.
7. Use one disposition: `adopt-upstream`, `retain-fork`, `compose`, `upstream-patch`, `delete`, or `defer`.
8. Keep synchronization, behavior fixes, refactors, and quality-of-life changes in separate implementation slices and commits.
9. Never erase disagreement. Record the competing claims, the adjudication, and the deciding evidence.
10. Do not overwrite old baselines or decisions. Append a dated amendment and link the superseding evidence.
11. Repository records are authoritative. Agent memory, task state, and chat are secondary caches.

## State vocabulary

`seed` -> `mapped` -> `independent-review` -> `adjudicated` -> `approved` -> `implementing` -> `verified`

A row may instead be `blocked` with a concrete evidence gap and a named next action.

## Recovery gates

1. **Truth and triage gate:** refreshed refs, clean baseline, trustworthy quality gates, and a mechanical divergence/risk pre-screen.
2. **Mapping gate:** reviewed responsibility boundaries with no more than six seams assigned full review; all others use light review or explicit defer.
3. **Ledger gate:** each full seam has two independent reviews, Terra adjudication, reproduced decisive evidence, confidence, and a disposition. Light seams have a concise evidence-backed ledger.
4. **Pilot gate:** one bounded ancestry or integration pilot runs as soon as its prerequisite ledgers pass, before the final cross-seam plan.
5. **Architecture gate:** Fable reviews the full ledger in light of pilot results and approves or amends the recovery plan.
6. **Implementation gate:** each slice has isolated scope, tests, rollback criteria, and no mixed sync/refactor/fix/QoL concerns.
7. **Sign-off gate:** an independent Grok audit informs a final Sol and Fable decision that touched code, tests, docs, and ledgers describe the same system.

## 2026-07-15 G2 pilot-gate authorization amendment

Independent Opus review of fixed coordinator commit `16e52bf4bcdffb0e8aea46266488960673e8ee5f` returned **PASS** for exactly one bounded Phase 3 fixture pilot. The byte-exact review is [`reviews/2026-07-15-g2-pilot-gate-opus.md`](./reviews/2026-07-15-g2-pilot-gate-opus.md), SHA-256 `abb7b2694abccb0c32385fc552dcc29bf0eba854d439c5c43dc82ba4f3991e4f`.

This supersedes only the earlier `OPEN pending G2` status. It does not authorize live credentials, network egress, a daemon, tools/MCP, memory, discovery, reload, publication, installation, update, cancellation, retry, compaction, or gate-baseline changes. G3 must encode the review's exact seven observations, stop conditions, rollback, R00 budgets, current R09 truth, and dedicated-driver requirement before G4 can execute.

## 2026-07-15 G4 bounded-pilot amendment

The authorized G4 fixture pilot completed all ten checked-in validation steps with expected and actual exits equal at source HEAD `505cd86726f86dc0eedaf3998afae6ed83290d5d`. The durable result is [`G4_RESULT.md`](./G4_RESULT.md); successful byte-exact evidence and the complete failed-attempt history are indexed in [`evidence/README.md`](./evidence/README.md).

This is a coordinator validation PASS, not unrestricted Phase 3 approval. The exact composition remains bounded to one offline account fixture, one symbolic subscription route, one compatible Subscribe, one no-tool/no-memory agent turn, and one evidence/replay stream. Advancement is pending independent review of the fixed G4 evidence/status commit. All exclusions in the G2 amendment remain binding.

## 2026-07-15 G5 pilot-evidence review amendment

Independent Anthropic Opus review of fixed commit `da7c155b9d34ff719e065c855338eea3574d62a9` returned **PASS** with no blocking findings. The byte-exact artifact is [`reviews/2026-07-15-g5-g4-evidence-opus.md`](./reviews/2026-07-15-g5-g4-evidence-opus.md), SHA-256 `37f094d26b196612f2171de98d52238abb72bb8b69d59b149e7bb00999db86d3`. This closes independent review of the exact bounded pilot; it does not authorize any excluded adjacent behavior.

## 2026-07-15 Phase 4 architecture-plan amendment

The corrected Fable cross-seam synthesis has been coordinator-audited into [`RECOVERY_PLAN.md`](./RECOVERY_PLAN.md). It selects curated composition, orders W0-W7, accounts for all seventeen non-deferred responsibilities, separates slice classes and commit boundaries, and preserves all G4/G5 exclusions. The byte-exact initial and corrected Fable artifacts remain under [`reviews/`](./reviews/). Plan execution is blocked pending independent review of the fixed plan commit.

## 2026-07-15 architecture-gate PASS amendment

Independent Anthropic Opus review of fixed plan commit `76ead5607032ef9e574979a779f6fddc60607b23` returned **PASS** with no blocking findings. The artifact is [`reviews/2026-07-15-phase4-plan-opus-review.md`](./reviews/2026-07-15-phase4-plan-opus-review.md), SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92`. The architecture gate is closed; only W0 record-consistency closure is next.

## 2026-07-15 W0 record-consistency amendment

W0 append-only closure now reconciles the R04 source-review tail, five light-ledger coordinator/Fable approvals, R00/R09/R11 Phase 4 review status, and historical production-size count `60` versus fixed G0/G4 count `61`. It also carries the previously omitted R04 streaming-marker partial-outcome fixture into W3. No source or behavior changed; independent mechanical review is pending.

## 2026-07-15 W0 gate PASS amendment

Independent Opus review of W0 commit `11a78a858` returned **PASS** with high confidence and no blockers. Preserved review SHA-256: `bd662db1792edcfed7276aed3203fd173f047daa58747ca8bcbabca290999fd3`. W0 is complete; no later source or external-action slice is authorized by this record.
