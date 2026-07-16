# Phase 6 Independent Spot-Check Report

Role: Independent Phase 6 spot checker (ORCHESTRATOR_PROMPT.md lines 227-245). Read-only, adversarial audit.

## Fixed refs reviewed

- Candidate commit: `4f96772b6f018d303d1da3f1438e3a290c2a5210` (= current HEAD) on `recovery/2026-07-15`.
- Accepted Phase 6 audit source head: `51168d16e9c708ae4afff09a6fc6402642d17782` (candidate is one docs-only commit above it).
- Approved Phase 5 source head: `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`.
- Plan head cited by task: `76ead5607032ef9e574979a779f6fddc60607b23` (RECOVERY_PLAN reviewed commit `e2464666d...`; the given ref resolves within the plan-review lineage).
- Merge base / `vendor/upstream`: `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.
- Sole dirty path: `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`, diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00` (reproduced live). Four stashes intact.

## Verdict

**PASS.** Zero unresolved IMPORTANT or CRITICAL findings. No material overclaim of test results, evidence, or governance state. One LOW labeling imprecision noted below.

## Prioritized findings with severity

1. **LOW / informational — "76 expected-exit entries" is a line count, not a check count.** README.md, static-audit.txt, PROGRESS.md, and evidence/README.md all state the accepted driver has "76 expected-exit entries." The driver actually runs **62** distinct `run_expect` checks (34 static + 28 from the 14-fixture R04 loop). `manifest.tsv` has 76 physical lines because 14 are continuation lines from multi-line embedded Python commands. Every one of the 62 real checks has matching expected/actual exit (0/0 for greens, 1/1 for the four expected-red debt gates), so the substantive claim ("zero mismatches, all pass") is fully supported. This is a phrasing imprecision, not a results overclaim, and does not affect the PASS bar. Recommend future wording say "62 checks / 76 manifest lines."

2. **Informational — panic count differs across sections by head, correctly.** evidence/README.md line 80 records panic `46` at the 2026-07-15 G0 head (`6c6a4f2c8`); the Phase 6 sections record `31 -> 48` at the Phase 6 head (`51168d16e`). This is honest Phase 5 debt growth (added panic-prone sites in `jcode-plan/src/dag/ops.rs` and `src/bin/memory_recall_bench.rs`), preserved as visible expected-red, not a contradiction.

No CRITICAL, HIGH, or IMPORTANT correctness, evidence, governance, or stale-status issues found.

## Sampled evidence and hashes (independently recomputed)

- Phase 6 `SHA256SUMS` file: `9af58f1563f266066edd6da9208983da62eeb0b1997ec78f9c26318221dcd2a3` (matches claim). `shasum -c SHA256SUMS`: all 88 entries OK.
- `accepted/verify_raw.sh`: 62 decompressed raw hashes OK, exit 0.
- `invalid/historical-r02-count-guard/verify_raw.sh`: 14 raw hashes OK, exit 0.
- All 17 tracked recovery-evidence `SHA256SUMS` manifests reproduce from their owning directories (0 failures).
- Sampled review artifact hashes verified: W5 correction-final Opus `582b3d12...`, Fable `d701676f...`; W6 R10 Grok pass-final `07ee7c3f...` — all match ledger claims.
- Prompt diff SHA-256 `8e8e6a92...` reproduced live; stash count = 4; `vendor/upstream` = `631935dd1...`; `PROTOCOL_VERSION = 1` at `crates/jcode-protocol/src/lib.rs:26`.
- Raw test outputs confirm: build-support 48 passed, protocol 81 passed, R02 subscription 38 passed, R02 provider filter 4 passed, R12 11 passed; panic/swallowed/code-size/test-size each exit 1 with visible debt text.

## Validation performed

- Confirmed candidate commit == HEAD; candidate diff over accepted head is docs-only (97 files, 1192 insertions, 0 deletions of code).
- Parsed `accepted/manifest.tsv`: 62 real checks, 0 expected/actual mismatches (greens 0/0, four reds 1/1).
- Verified invalid first attempt is honestly described: its R02 suite passed 38/38 but the stale count guard required 35 and exited 1; no product test failed. README states this plainly.
- Phase 5 integrated diff (`6c6a4f2c8` -> `51168d16e`): empty `crates/jcode-protocol` diff; 0 new `provider_session_id` assignments; runtime-identity assignment additions confined to `recovery_pilot_tests.rs` and the `mod tests` block in `reload_state.rs` (test-only, matches static-audit).
- Append-only integrity: 0 line deletions across sampled ledgers (R00/R02/R04/R05B/R08A/R09/R10/R11/R12) in candidate range. The only 6 recovery-doc deletions are legitimate current-status table transitions (blocked -> integrated) in PROGRESS/README/seams-README, with prior detail preserved in ledgers and the Phase 6 rollup appended.
- Overlays R00/R09/R11 preserve full count history (swallowed 3,077/3,072/3,074; panic 46/48; prod-size 60/61) as dated evidence, none rewritten; no `--update` anywhere (guard `no_update_invocation` exit 0).
- Stale-doc items flagged in RECOVERY_PLAN §2 (R04 ledger tail, five light-ledger "pending" lines) are resolved append-only: prior "pending" text retained, resolved approval lines appended below.
- Deferred risks in RECOVERY_PLAN §4 each carry owner, reason, evidence gap, and escalation trigger.
- Failed/superseded evidence preserved as required: W5 mutation-surviving evidence failure (superseded by discriminating test `95861f4f5`), W6 initial Grok FAIL (superseded by draft-staging correction), R12 W1 initial FAIL and Opus/Fable disagreement, plus disk-failed attempts — all retained, none counted as passing.

## Open questions

- The task cited plan head `76ead5607...`; the current RECOVERY_PLAN records reviewed commit `e2464666d...`. Both sit in the plan-review lineage and the plan content is internally consistent; I did not resolve whether `76ead5607...` is an intermediate plan-branch commit versus a typo. Non-blocking.
- I did not re-run the Cargo suites (per instruction to prefer validating preserved accepted evidence). The claim that the 62 checks pass rests on the byte-verified raw logs, not a fresh execution.

## Confidence

High on evidence integrity, hash reproduction, append-only governance, preservation, protocol/version invariance, authority-writer test-only confinement, and honest treatment of failed/invalid attempts. Medium on live runtime behavior, which the recovery deliberately never exercised (no-Nix/no-network/no-daemon boundary) and which remains explicitly deferred.

## What I did not check

- Did not execute any Cargo/Nix build or test, provider, daemon, network, or release path (out of scope; relied on preserved raw evidence).
- Did not re-derive the full 288/243 patch-ID divergence measurement or re-review upstream commit clusters.
- Did not read every one of the 66 review artifacts or all 17 ledgers in full; sampled R12/R05B/R04/R02/R08A/R10 plus R00/R09/R11 overlays and spot-verified hashes.
- Did not evaluate the substantive engineering quality of the Phase 5 code changes beyond the diff-shape and identity-writer invariants.
- Did not touch the repo, refs, stashes, worktrees, prompt, or index (verified working tree unchanged except the pre-existing user prompt edit).

Reported for the coordinator, Architect, and joint Sol/Fable sign-off. I am not the final decision maker.
