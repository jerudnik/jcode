# R11 Documentation, incidents, and backlog governance: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4` (Phase 1 adjudication baseline; ledger authored at `8848f2d54f67f9a5a1de76bace9666c78036e116`); upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light overlay` (mandatory governance overlay per `RESPONSIBILITIES.md`; required record) |
| Research budget | 8 decisive checkpoints for the R00/R09/R11 overlay batch; 1 consumed here beyond preserved Phase 0/1 evidence |
| Recommended disposition | `retain-fork` |
| Confidence | high |

R11 owns active recovery truth, incident hashes, maintenance state, stale-instruction retirement, and append-only decisions. It excludes runtime authority and gate verdicts. `docs/fork/recovery/` is the durable source of truth, ordered by the authority list in `README.md`: current code/tests/refs first, then reproducible ledger evidence, then approved decisions here, then older documents. Upstream documentation is not an authority for fork recovery truth in any tier of that order.

## Findings

| Finding | Evidence | Consequence |
|---|---|---|
| Recovery truth is append-only by rule and by practice | `README.md` record rules 9-10 ("Never erase disagreement", "Do not overwrite old baselines... Append a dated amendment"); `BASELINES.md` carries three dated 2026-07-15 sections without rewriting earlier ones; `PROGRESS.md` appends checkpoints | Amendment, not replacement, is the only legal edit to recorded decisions; superseding evidence is linked, not substituted |
| The Phase 1 review debate is preserved with hashes that reproduce | `RESPONSIBILITIES.md` lists five review files with SHA-256; rerun `shasum -a 256 docs/fork/recovery/reviews/*.md` on 2026-07-15 reproduced all five (mapper `2c73d75a...`, critic `bc215eca...`, gap `dfdfba30...`, final `21fd96c4...`, rereview `3b6bb2d1...`) plus the three gate-parser reviews | Preserved reviews are tamper-evident; a hash mismatch is itself an incident to record, not to fix silently |
| The user's `ORCHESTRATOR_PROMPT.md` edit is preserved, unadopted, and user-controlled | Coordinator worktree `git diff -- docs/fork/recovery/ORCHESTRATOR_PROMPT.md \| shasum -a 256` = `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`, reproduced 2026-07-15, matching `BASELINES.md`, `PROGRESS.md`, and `RESPONSIBILITIES.md` ("Preserved prompt edit"); the final Opus review noted the diff removes a numbered safety rule | The edit is neither reverted nor treated as Phase 1 authority; the committed prompt remains authoritative text while the working-tree edit stays untouched pending an explicit user decision |
| Stale maintenance instructions were retired against ancestry, not deleted | `PRESCREEN.md` maintenance reconciliation: `fix-config-hotpath-spam` and `fix-marker-sweep` entries are closed because `a69ef9710`, `97529fd6c`, etc. are ancestors; `agent/marker-hardening` preserved at `6fc5623a5`; external notes carry absolute path plus SHA-256 (spawn-storm `7fdd9040...`, stale-daemon `80012e2c...`, fork-health audit `a672073e...`) | Retirement is evidence-backed reconciliation; old instructions become historical records, and external evidence stays citable by hash even though the notes are untracked |
| Fork documentation volume dwarfs upstream's with modest overlap | `PRESCREEN.md` R11 row: files F/U/O = 116/25/21, commits 111/33, hunks 0/0; older docs (`docs/fork/SYNC_MODEL.md`, architecture notes) remain useful evidence with non-current status labels per `README.md` authority order | The fork's governance corpus is the recovery substrate; upstream docs changes are candidate evidence for other seams, not replacement recovery truth |

## Mandatory overlay obligations on every seam

1. **Append-only recovery truth.** Seam ledgers, `BASELINES.md`, and `PROGRESS.md` accumulate dated amendments. No seam rewrites an earlier decision, baseline, or review; superseding evidence is appended and linked. Disagreement is recorded, never collapsed.
2. **Incident and external-evidence hashing.** Any incident, external note, or untracked artifact a seam relies on is summarized in the ledger with its absolute path and SHA-256 (`SEAM_LEDGER_TEMPLATE.md` evidence standard). Cited hashes must reproduce; mismatches are recorded as incidents.
3. **Preserved prompt edit stays user-controlled.** The `ORCHESTRATOR_PROMPT.md` working-tree diff, SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`, is neither adopted as authority nor reverted by any agent. Only the user decides its fate; any change to that hash without a recorded user decision is an incident.
4. **Ownership boundaries on shared files.** The coordinator alone edits `RESPONSIBILITIES.md` and `PROGRESS.md` during parallel work; reviewers write only their assigned files (`seams/README.md`). Stale-instruction retirement requires ancestry or hash evidence, not judgment alone.

## Reproduction

```bash
shasum -a 256 docs/fork/recovery/reviews/*.md
# compare against the hashes listed in RESPONSIBILITIES.md and QUALITY_GATES.md links
git -C /Users/jrudnik/labs/jcode diff -- docs/fork/recovery/ORCHESTRATOR_PROMPT.md | shasum -a 256
# expect 8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00
git log --follow --oneline docs/fork/recovery/BASELINES.md   # appends only, no rewrites
```

## Explicit gaps

- The three external maintenance notes under `/Users/jrudnik/notes/projects/jcode/maintenance/` were not re-hashed in this session; the hashes cited are the preserved `PRESCREEN.md` values, and any seam relying on a note must re-verify its hash at use time.
- The 25 upstream-changed documentation paths were not individually reviewed; none is adopted, and any seam citing one must treat it as evidence about upstream, not about the fork.
- Older fork docs (`docs/fork/SYNC_MODEL.md`, `docs/fork/patch-ledger.md`, architecture records) were not re-audited for stale status labels beyond the `README.md` blanket caveat; seams citing their measurements must revalidate them.

## Disposition and conditions

- Recommended disposition: `retain-fork`. The recovery documentation system is fork-only, self-consistent, hash-anchored, and load-bearing for every later phase. Upstream has no counterpart to adopt, and deleting or deferring governance records would orphan the evidence chain.
- Acceptance or retirement condition: this overlay retires when Phase 6 sign-off confirms the recovery record is internally consistent (hashes reproduce, appends only, ownership boundaries respected) and that touched code, tests, docs, and ledgers describe the same system. Until then it binds every seam once the coordinator approves this ledger.
- Escalate to full review if: a preserved review or baseline is edited rather than amended; a cited hash fails to reproduce; the preserved prompt-diff hash changes without a recorded user decision; a seam ledger relies on an unhashed external note; or documentation claims diverge from ledger evidence in a way that changes a disposition.
- Coordinator approval: pass, 2026-07-15. Review links and hashes, append-only checkpoints, exact changed paths, and the preserved prompt hash were checked.
- Fable review: pending independent Phase 4 architecture review; this ledger was authored by Fable and cannot self-approve.

## 2026-07-15 W0 Phase 4 review amendment

The stale Fable-pending line is discharged. Corrected cross-seam Fable plan SHA-256 `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` retained R11 as the binding append-only governance overlay; independent fixed-plan Opus review SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92` returned **PASS**.

R11 remains active through Phase 6. W0 closes stale approval prose only by dated append; no earlier status, disagreement, count, failure, or review text is deleted.

## 2026-07-16 Phase 6 coordinator-audit amendment

The active recovery status documents now carry append-only Phase 6 rollups that
explicitly supersede stale plan-time meanings without deleting the historical
text. W0 through W6 completion, W7 defer, the 17-responsibility disposition
arithmetic, current R09 exits, remaining external-action gates, and the
historical R02-count-guard failure are all recorded consistently in
`RECOVERY_PLAN.md`, `RESPONSIBILITIES.md`, `seams/README.md`, `PROGRESS.md`, and
the final evidence index.

All 17 recovery evidence manifests verify. The final coordinator package is
[`../../evidence/2026-07-16-phase6-final-audit/`](../../evidence/2026-07-16-phase6-final-audit/),
`SHA256SUMS` SHA-256
`9af58f1563f266066edd6da9208983da62eeb0b1997ec78f9c26318221dcd2a3`.
The coordinator judges the R11 retirement conditions mechanically satisfied.
R11 remains active only until the required independent reviews and joint
Sol/Fable sign-off are preserved.

## 2026-07-16 independent spot-check and wording-correction amendment

Independent Opus spot review returned **PASS** with zero IMPORTANT or CRITICAL
findings; report SHA-256
`092dbf4ec862b23b8d778f029772b46b434202e816622bd1f71c4bfa1f759dcc`.
Its sole LOW finding distinguished 62 real expected-exit checks from 76 TSV
physical lines. The active records and package metadata now state both values.
No command, exit, raw transcript, or conclusion changed. The corrected package
manifest hash is
`ca8ff5b9f3b6c09dc0ff05de9b3c1c426fc2373706eeeca26cad87126f2e14d8`.

The local index-lock race during the first candidate commit attempt is recorded
in `RECOVERY_PLAN.md` and `PROGRESS.md`; no history moved and only the stale lock
was removed after a no-live-Git-process check. R11 remains active through the
architecture review and joint sign-off.

## 2026-07-16 architecture-review amendment

Independent Fable architecture and maintainability review returned PASS with
zero IMPORTANT or CRITICAL findings; report SHA-256
`3fa06d1109c5fc56c9cf1bc73dcea540cff084b5ef4fcc1a0a8dcd48e3910865`.
Its five LOW findings are not hidden as a generic W7 label: the recovery plan
now records per-item owners, reasons, evidence gaps, and triggers, and marks the
W7 growth trigger observed and ripe. R11 remains active only for joint
Sol/Fable sign-off and final report preservation.

## 2026-07-16 final retirement amendment

**Status: retired as a special Phase 6 overlay.** The byte-exact joint sign-off
reports are preserved with SHA-256 values
`228f5937dd7eafa6570ed857b3a8db43a1ed43c0a3c9ad6dcaf6e2d29ef8ebe4` (Sol)
and `7da9ca6810bde9db1035b68e1d2a46f3c0966c6610db7c19553acc96cacc13d3`
(Fable). Both sign the completed ledgers and recovery plan PASS at fixed head
`17586246a` with zero unresolved IMPORTANT or CRITICAL findings.

Append-only corrections, invalid/superseded-attempt preservation, exact report
hashes, code/test/docs/ledger agreement, deferred-risk ownership, and final
reproduction instructions are complete. R11's evidence-integrity and
active-document-consistency rules continue under normal documentation
governance; only the special recovery overlay gate is closed.

### Final-closure tooling note

The first retirement patch missed three append targets because its expected
tail context was stale; successful hunks remained append-only and the missing
R00/responsibility/index appends were added after reading the current tails. The
first closure manifest parser also assumed a nonexistent header/five-column
shape and therefore reported zero rows. The corrected headerless four-column
parser reproduced 62 checks, zero mismatches, and four expected-red rows. These
process failures did not alter evidence or product results.
