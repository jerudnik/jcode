# G4 bounded pilot result

> **Final amendment.** Independent G5 review later returned **PASS** at fixed
> commit `da7c155b9`; see
> [`reviews/2026-07-15-g5-g4-evidence-opus.md`](reviews/2026-07-15-g5-g4-evidence-opus.md).
> The original coordinator-only status below is retained as the state at the
> time this result was first recorded.

Status: coordinator validation **PASS**, pending independent G5 review.

The authorized fixture-backed pilot ran at source HEAD `505cd86726f86dc0eedaf3998afae6ed83290d5d` on branch `recovery/2026-07-15`. It used the checked-in plan [`pilot/2026-07-15-g4-validation-plan.json`](./pilot/2026-07-15-g4-validation-plan.json) and dedicated driver `scripts/recovery_validation_driver.py` without `--update`, network access, live providers, a daemon, reload, tools/MCP, memory, publication, installation, cancellation, retry, or compaction.

## Implemented slices

| Category | Commit |
|---|---|
| Offline subscription fixture adapter | `f6ca30c1a6c8c9d65a3fc585c12b2385fb618157` |
| Focused adapter tests | `f796ace46263aedcdd9b936cddee2f1a2a69078a` |
| Exact composite pilot fixture | `86d3e3214a2bafdb2251d69ceb12ad89fa3022c4` |
| Driver vendor-pin correction | `b1f5d187d` |
| Standalone observation framing | `505cd86726f86dc0eedaf3998afae6ed83290d5d` |

The exact current-thread test is `agent_tests::recovery_pilot_one_fixture_route_subscribe_turn_evidence`. It composes one accepted Plus subscription fixture, one symbolic `jcode-subscription` route selecting `gpt-5.5`, one compatible Subscribe carrying a distinct runtime projection, one `Agent`, one no-tool/no-memory turn, and one four-event evidence stream with deterministic replay.

## Successful evidence

The byte-exact driver output is [`evidence/2026-07-15-g4-bounded-pilot/`](./evidence/2026-07-15-g4-bounded-pilot/).

- `SHA256SUMS` SHA-256: `b4692dc023075d89fcbe94065d089234fa59bbc5777215082870eb00c3842343`
- `manifest.tsv` SHA-256: `321c43f51d5cd6e9d953896117d90873adf17b5b4f594ea7fb4f1cb2341eb4e5`
- `run.meta.json` SHA-256: `b85e34c61e434e956c2a8cdfc51785ddf3b99d111bf59f5cbb7600bdae9140bb`
- Pilot log SHA-256: `fdc47ac6cb27cad0dec492990075f98c8a248341fa7de13db00c953c3ae484bf`
- Result: `passed`, ten steps run of ten planned
- Pilot observation lines: exactly one
- Forbidden output hits: zero

All expected and actual exits matched:

| Step | Expected | Actual | Meaning |
|---|---:|---:|---|
| Pilot fixture | 0 | 0 | One bounded composition passed |
| Classifier | 0 | 0 | 17/17 classifier tests passed |
| Panic budget | 1 | 1 | Expected-red debt remained 46 |
| Swallowed-error budget | 1 | 1 | Expected-red debt remained 3,074 |
| Production-size budget | 1 | 1 | Expected-red debt remained 61 |
| Test-size budget | 1 | 1 | Expected-red debt remained 31 |
| Wildcard budget | 0 | 0 | Total remained 16 |
| Warning budget | 0 | 0 | Total remained 0 |
| Shell syntax | 0 | 0 | Passed |
| Diff check | 0 | 0 | Passed |

The pilot observation records account `acct_fixture`, live Plus truth, auth transition from credential-present to request-valid, OAuth credential classification, provider `jcode`, model `gpt-5.5`, route `jcode-subscription`, compatible handshake, distinct runtime projections, tools `0`, memory disabled, telemetry disabled, deterministic token usage `7/3/10`, four evidence events, four replay events, and terminal counts `1/1/1`.

## Preservation proof

Preflight and postflight both recorded:

- Sole dirty path: `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`
- Prompt diff SHA-256: `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`
- Stash count: four
- `vendor/upstream`: `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`
- Active build processes: none

Nix printed saved Cachix substituter and trusted-key notices even with offline and substitution-disabled settings. Those notices are preserved in the launch transcript. No fetch, credential use, or network-backed provider occurred.

## Preserved failed attempts

The complete attempt history is [`evidence/2026-07-15-g4-attempt-history/`](./evidence/2026-07-15-g4-attempt-history/), whose `SHA256SUMS` SHA-256 is `f1fa86fdbffca927d0128fda92bdb3ff3cdfa85d2561d02b683cd275941f4944`.

1. Two launch attempts failed before driver preflight because the cached dev shell did not expose a usable `python3`.
2. The corrected Python launch reached preflight and exposed a driver defect that assumed a physical `vendor/upstream` directory. The partial output contained only the copied plan. The tooling fix now resolves `refs/heads/vendor/upstream^{commit}` and has a focused regression test.
3. The next driver run passed the fixture assertions but rejected its log because Cargo prefixed the observation on the test-status line, so the required standalone observation count was zero. The rejected driver output and the separate corrected framing verification remain preserved.
4. The third full driver attempt passed all ten steps and is the successful evidence set above.

These failures are infrastructure and evidence-framing history. They are not converted into source PASS results or erased by the successful attempt.

## Claim limit

This result proves only the exact G2/G3 bounded question. It does not authorize a live subscription backend, real credentials, network egress, generic-client identity claims, a running server or daemon, reload, tools/MCP, memory, publication, installation/update, cancellation, retry, compaction, disconnect/takeover, or quality-baseline changes. Phase advancement remains blocked pending an independent review of this fixed evidence/status commit.

## G5 independent review amendment

Independent Anthropic Opus review of fixed evidence/status commit `da7c155b9d34ff719e065c855338eea3574d62a9` returned **PASS** with high confidence and no blocking findings. The byte-exact review is [`reviews/2026-07-15-g5-g4-evidence-opus.md`](./reviews/2026-07-15-g5-g4-evidence-opus.md), size `15,077` bytes, SHA-256 `37f094d26b196612f2171de98d52238abb72bb8b69d59b149e7bb00999db86d3`.

The reviewer independently recomputed both G4 evidence sets, checked exact membership and named hashes, verified the ten-step expected/actual sequence, compared preflight/postflight preservation projections, audited fixture behavior and driver fail-closed logic, ran the driver's ten offline unit tests and plan check, inspected commit separation, and confirmed the current prompt hash and four stashes. It ran no Cargo/Nix build or live pilot.

The preserved nonblocking limitations are: the observation JSON uses literal values backed by preceding assertions rather than serializing variables directly; offline Nix still printed saved Cachix configuration notices; and the evidence proves only the exact bounded question. These limitations do not widen the claim. G4/G5 now establish a reviewed PASS for the bounded pilot only.
