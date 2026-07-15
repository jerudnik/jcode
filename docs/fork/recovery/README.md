# Fork recovery workspace

Status: scaffold complete, ready for recovery session, 2026-07-15

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
- [`RESPONSIBILITIES.md`](./RESPONSIBILITIES.md): at-a-glance responsibility index. The coordinator alone edits this file during parallel work.
- [`SEAM_LEDGER_TEMPLATE.md`](./SEAM_LEDGER_TEMPLATE.md): required structure for one authoritative seam record and its two independent reviews.
- [`PROGRESS.md`](./PROGRESS.md): phase gates, checkpoints, and blockers. The coordinator alone edits this file.
- [`ORCHESTRATOR_PROMPT.md`](./ORCHESTRATOR_PROMPT.md): self-contained launch prompt for the recovery session.
- [`seams/README.md`](./seams/README.md): seam-directory ownership and merge-back rules.
- `seams/<ID>-<slug>/`: one directory per reviewed seam. A full review contains `opus-review.md`, `grok-review.md`, and `ledger.md`.

## Baseline to revalidate at session start

The exact pre-scaffold snapshot and reproduction commands live in [`BASELINES.md`](./BASELINES.md). The last code commit before these recovery documents was `3d80eaf343e690aaa8b428d0b3ed6de64b7464d0`. At that point the fork had 286 fork-only commits, upstream had 246 upstream-only commits, and 406 files had changed on both sides since merge base `631935dd1`.

The recovery documentation commit makes HEAD and commit counts differ immediately. Refresh refs and append a new baseline before research. Never replace the older snapshot.

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
