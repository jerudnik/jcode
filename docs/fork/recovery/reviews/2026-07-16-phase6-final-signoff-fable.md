# Phase 6 Joint Sign-Off: Fable Half (Final Signer)

Role: fresh final signer for Phase 6 under `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` lines 227-247. This is the Fable half of the joint Sol/Fable sign-off on the completed seam ledgers and recovery plan together. It is a separate session from, and independent of, the prior Architect (Fable) architecture review. Read-only throughout; no repo, ref, stash, worktree, prompt, index, or report mutation.

## Fixed refs

- Sign-off head: `17586246afb11cd54e1db12a0beec05fd29a0612` == live `HEAD` on `recovery/2026-07-15` (verified).
- Accepted Phase 6 source head: `51168d16e9c708ae4afff09a6fc6402642d17782`. `git diff --name-only 51168d16e..17586246a` = 99 paths, zero outside `docs/` (verified live): the sign-off head is docs-only above the accepted source head.
- Phase 5 base: `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`. Integrated range 528 files, 15,219 insertions / 257 deletions (recomputed).
- Merge base / `vendor/upstream`: `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` (verified live).
- Preservation: sole dirty path `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`, diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00` (reproduced live); exactly 4 stashes; 29 registered worktrees untouched.
- `PROTOCOL_VERSION = 1` at `crates/jcode-protocol/src/lib.rs:26`; empty `jcode-protocol` and `jcode-session-types` diffs across `6c6a4f2c8..51168d16e` (recomputed).

## Materials signed together

Signed as one completed set, all read at the sign-off head:

1. `RECOVERY_PLAN.md` in full, including sections 15 (Phase 5 execution and coordinator-audit rollup), 16 (spot-check amendment), and 17 (architecture-review amendment with the W7 defer table).
2. `RESPONSIBILITIES.md` including the 2026-07-16 Phase 6 responsibility-status reconciliation and spot-check correction.
3. `PROGRESS.md` including the three Phase 6 checkpoints (coordinator audit, spot-check PASS, architecture PASS).
4. `seams/README.md` including the Phase 6 active-status rollup and spot-check metadata correction.
5. All 17 non-deferred seam ledgers (R00, R01, R02, R03A, R03B, R04, R05A, R05B, R06A, R07A, R07C, R08A, R09, R10, R11, R12, R13), tails read directly; R00 read in full.
6. Final audit evidence package `evidence/2026-07-16-phase6-final-audit/` (accepted + preserved invalid attempt + static audit + all-manifests reconciliation).
7. Prior required reviews, hash-verified at both working tree and the `17586246a` blob:
   - Opus spot check `reviews/2026-07-16-phase6-spot-check-opus.md`, SHA-256 `092dbf4ec862b23b8d778f029772b46b434202e816622bd1f71c4bfa1f759dcc` (PASS).
   - Independent Architect Fable `reviews/2026-07-16-phase6-architecture-fable.md`, SHA-256 `3fa06d1109c5fc56c9cf1bc73dcea540cff084b5ef4fcc1a0a8dcd48e3910865` (PASS).
   - Corrected final audit package `SHA256SUMS`, SHA-256 `ca8ff5b9f3b6c09dc0ff05de9b3c1c426fc2373706eeeca26cad87126f2e14d8` (matches both tracked file and committed blob).

## Verdict

**PASS.**

Zero unresolved IMPORTANT or CRITICAL findings across the completed set. No material overclaim found in any active document, ledger, or evidence README. Both required prior reviews are committed, hash-anchored, and PASS. Every Phase 6 completion criterion is satisfied at the fixed refs.

## Phase 6 criteria matrix (ORCHESTRATOR_PROMPT lines 233-247)

| Criterion | Verdict | Decisive evidence (independently checked) |
|---|---|---|
| Responsibility boundaries and authorities explicit | PASS | `RESPONSIBILITIES.md` approved map + cross-seam invariants; `RECOVERY_PLAN.md` §2 authority table; disposition arithmetic 14 retain-fork / 2 compose / 1 defer reconfirmed across plan, index, and seams README |
| Active seams have evidence-backed dispositions | PASS | All 17 ledgers exist with disposition, conditions, and hash-anchored review closures; sampled tails of all 17 agree with the Phase 6 rollup |
| Pilot selected an economically justified sync posture | PASS | Curated composition chosen with explicit claim limits (plan §1): structural inference (empty patch-ID intersection over 288/243 commits, squash ancestry, retain-fork dominance) honestly labeled as inference, not measured replay comparison, with a named trigger for per-seam replay experiments |
| Remediation slices implemented, tested, documented, committed | PASS | W0-W6 integrated with per-workstream evidence dirs (all 17 manifests verify: 14 `SHA256SUMS` + 3 `MANIFEST.sha256`, 0 failures, rerun live); W1-W6 code spot-checked in source (see below) |
| Trusted quality gates do not regress | PASS | Accepted manifest: 62 real checks, 0 expected/actual mismatches (reparsed myself); greens 0/0; exactly four expected-red debt gates (`panic`, `swallowed`, `code_size`, `test_size`) at 1/1 with real debt text in raw logs; `no_update_invocation` guard exit 0; no `--update` anywhere |
| Code, tests, docs, ledgers describe the same behavior | PASS | Source spot checks at head match ledger claims: single liveness authority `swarm_verbs::member_status_is_dead` (delegated from `server/swarm.rs:373`, `comm_control.rs:734`); W5 timeout routes to `onboarding_handle_login_failed(None)` (`onboarding_flow_control.rs:1581`); `verify_asset_checksum_required` (`update.rs:281,999`); opt-in reload gates in all three installers; R04 restart-identity fixture in `reload_state.rs:728` inside `mod tests` (line 617) |
| Obsolete mechanisms / stale instructions archived | PASS | W0 closed all named stale ledger lines append-only; Phase 6 rollup supersedes plan-time status cells explicitly while preserving them as history; 0 code deletions in the docs-only candidate range |
| Deferred work has owner, reason, evidence gap, trigger | PASS | Plan §4 register complete (checked every row); §17 adds the four architecture LOW items with per-item owner/reason/gap/trigger; R08A remains the single broader defer with the W5 defect closed |
| `PROGRESS.md` reproducible final validation summary | PASS | Phase 6 checkpoints cite exact refs, counts, hashes, and reproduction commands; I reproduced the package (`shasum -c` 88/88 OK), raw logs (`verify_raw.sh` all OK), manifest parse, and all 17 evidence manifests from those instructions alone |

Authority-writer invariants independently reconfirmed: zero new `provider_session_id` assignments in `6c6a4f2c8..51168d16e`; new `runtime_identity` assignment additions confined to `recovery_pilot_tests.rs` and the `reload_state.rs` test module; protocol diff empty; `PROTOCOL_VERSION` unchanged.

## Prioritized findings

No CRITICAL, HIGH, or IMPORTANT findings.

1. **LOW / informational — carried, not new.** The five architecture LOW items (duplicated W1 emission blocks, string-only interruption identification, `failed` vs `stopped` display race, missing `ClassifiedEvidenceError::source`, unbounded `append_progress_provenance`) are correctly registered in plan §17 with owners and triggers. Nothing to add; see adjudication below.
2. **LOW / informational — evidence-manifest naming is heterogeneous.** W1/W2/W2-scope-repair use `MANIFEST.sha256` while later packages use `SHA256SUMS`. All 17 verify (0 failures, rerun live) and the final-audit `all-evidence-manifests.txt` reconciles them, so this is cosmetic only.
3. **Informational — the spot audit's unresolved plan-head lineage note (`76ead5607` vs `e2464666d`) remains open and remains non-blocking.** Neither hash participates in any fixed ref of this sign-off; every ref I signed resolves and reproduces.

## W7 / deferred-risk adjudication

**The observed W7 trigger and the architecture LOW findings are validly deferred and do not block recovery completion.** Grounds:

- The plan's own W7 trigger ("growth of duplicated emission/liveness logic in any new fix") fired via W1's deliberate duplication across both turn engines. The record does not hide this: §17 explicitly marks the trigger "observed and ripe" rather than dormant, which is the honest treatment.
- Every one of the four §17 defer rows carries a named owner, a substantive reason (behavior pinned by 11 `r12_` fixtures; reopening source after the accepted final audit would invalidate the reviewed chain for a non-correctness cleanup), a concrete evidence gap, and a hard trigger (next R12/R04/R05A/R05B-adjacent source change, or specific conditions like a third emission copy or a 2 KiB `checkpoint_summary`).
- None of the five items is a correctness, security, or consent defect: interruption/`failed` labeling outcomes are all dead statuses for the single reclaim authority; the retry-after path remains unwrapped; the debt is visible in expected-red gates with per-file attribution.
- The orchestrator's own completion bar requires deferred work to have owner/reason/gap/trigger, not to be zero. Performing new source work now, inside the final-audit window, would violate the bounded audit cadence the same prompt mandates. The deferral is therefore the compliant choice, with the caveat (which I endorse and restate as binding) that the next R12-adjacent source change MUST execute the W7 consolidation first or the deferral loses credibility.

**No-live/no-network claim limits remain honest.** Every PASS in the chain is explicitly bounded to the offline/fixture surface: plan §1 claim limits, §15 per-workstream "remains separately gated" language, seams README rollup, R10/R08A/R05B/R04 ledger tails, both prior reviews' "Medium confidence on live runtime behavior," and the evidence driver itself (process before/after equality, no-active-build and no-remote-builder guards, `CARGO_NET_OFFLINE=true`, cached toolchain only). I found no sentence claiming live daemon, provider, network, release, installer, updater, or swarm behavior was validated. The one place live machinery is even adjacent, the W6 optional-validation attempt where `nix shell --offline` contacted the network, is preserved as an invalid incident and explicitly not counted as evidence. That is the standard working.

## Overlay retirement recommendation

**Retire R00, R09, and R11 upon preservation of this joint sign-off**, their ledgers' stated conditions being met:

- **R00:** all dispositions were made against the fixed refs with recorded provenance; preservation passes at final state (branch, sole dirty prompt with hash `8e8e6a92...`, 4 stashes, `vendor/upstream` pinned at `631935dd1`, worktrees intact); no unlogged ref/stash/worktree mutation appears anywhere in the record (the one index-lock incident is logged, moved no history, and is preserved as a process failure).
- **R09:** all four red ratchets are explicitly re-accepted as attributed, visible debt (coordinator amendment in the R09 ledger); zero `--update` events (guard exit 0); classifier 17/17 green throughout; per-file attribution present in raw logs.
- **R11:** the record is internally consistent (every hash I recomputed reproduces, at working tree and committed blob), append-only discipline held (correction chains, failed reviews, and invalid attempts preserved rather than rewritten), and touched code/tests/docs/ledgers describe the same system per the criteria matrix above.

Retirement should be recorded as a dated append to each overlay ledger citing this report and the Sol half. Retirement does not relax the still-binding live/external gates (B3-class), which survive recovery completion as ordinary repository policy.

## Validation performed (read-only)

- Verified `HEAD` == `17586246a`, branch, docs-only candidate range (99 paths, 0 non-docs), 4 stashes, `vendor/upstream` pin, prompt-diff hash reproduced live.
- Recomputed SHA-256 of both prior reviews and the corrected package `SHA256SUMS` at the working tree and at the `17586246a` blobs; all three match the task-fixed values exactly.
- `shasum -a 256 -c SHA256SUMS` in the final-audit package: 88/88 OK. `accepted/verify_raw.sh`: all raw hashes OK, exit 0.
- Independently parsed `accepted/manifest.tsv`: 76 physical lines, 62 real expected-exit checks, 0 mismatches, exactly four expected-red gates, matching the corrected wording.
- Verified all 17 recovery evidence manifests from their owning directories (14 `SHA256SUMS` + 3 `MANIFEST.sha256`): 0 failures.
- Recomputed the Phase 5 protocol/session-types diff (empty), `PROTOCOL_VERSION = 1`, zero new `provider_session_id` assignments, and test-only confinement of new runtime-identity assignments (read the `reload_state.rs` diff hunk directly).
- Spot-checked W1/W2/W5/W6 ledger claims against live source: `TurnInterruptedError` private to `agent/evidence.rs`; single `member_status_is_dead` authority; `onboarding_handle_login_failed(None)` timeout route; `verify_asset_checksum_required`; `JCODE_RELOAD_SERVER`/`JCODE_SKIP_SERVER_RELOAD` gates in `install.sh`, `install_release.sh`, `install.ps1`.
- Read raw accepted logs for suite counts (48/81/38/11 passed) and expected-red debt text (panic, swallowed, code-size, test-size all exit 1 with per-file growth attribution).
- Confirmed the invalid first audit attempt is preserved and honestly described (R02 38/38 passed; stale count guard of 35 stopped it; driver correction disclosed as not byte-preserved).
- Spot-verified additional review hashes cited by ledgers: W5 correction-final Opus/Fable, W6 Fable pass-final, W2 Grok/Fable rereviews, remaining-light-ledgers review, Phase 4 plan review — all reproduce.
- Reviewed plan §4 and §17 deferred-risk rows field-by-field for owner/reason/gap/trigger completeness.

## Confidence

High on: evidence integrity and hash reproduction, docs/ledger/code agreement at the sampled surfaces, gate non-regression, authority-writer invariants, honest claim limits, deferred-risk completeness, and the append-only record. Medium on: live runtime behavior (daemon, reload, providers, network, releases, swarm), which the recovery deliberately never exercised and which every signed document bounds out explicitly. That boundary is a feature of the record, not a gap in it.

## What I did not check

- Did not run any Cargo/Nix build or test suite; per instruction I validated the byte-verified preserved raw logs instead of re-executing them.
- Did not exercise any live daemon, reload, provider, network, credential, installer, updater, release, signing, or swarm path.
- Did not read all 69 review artifacts end-to-end; I read both Phase 6 reviews in full, all 17 ledger tails (R00 in full), and hash-verified the review artifacts each Phase 6-relevant closure cites.
- Did not re-derive the 288/243 patch-ID divergence measurement, audit `.rerere-cache`, the `b3ed82a6b` squash content, the desktop crate, or deferred seams R06B/R07B/R08B-D.
- Did not resolve the non-blocking `76ead5607` vs `e2464666d` plan-head lineage question inherited from the spot audit.
- Did not verify the immutable build artifact `fd6297d9...` on disk (R01 ledger claim; not load-bearing for this sign-off).
- Did not modify the repository, refs, stashes, worktrees, prompt, index, or any committed report.

## Signature

I, **Fable (final signer, Phase 6 joint sign-off)**, sign **PASS** on the completed seam ledgers and `RECOVERY_PLAN.md` together, as one set, at fixed sign-off head **`17586246afb11cd54e1db12a0beec05fd29a0612`** on `recovery/2026-07-15` (accepted source head `51168d16e9c708ae4afff09a6fc6402642d17782`), with zero unresolved IMPORTANT or CRITICAL findings and no material overclaim. This is the Fable half of the joint Sol/Fable sign-off; recovery completion additionally requires the Sol half and preservation of both reports. This signature authorizes no live, networked, credentialed, release, installer, updater, signing, publication, swarm, or baseline-update action.

Signed: Fable — 2026-07-16T09:18Z
