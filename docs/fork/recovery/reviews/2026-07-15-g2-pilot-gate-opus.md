# G2 Phase 3 Pilot-Gate Adversarial Review

- Reviewer: sole independent adversarial G2 pilot-gate reviewer (verify posture, read-only).
- Fixed coordinator commit reviewed: `16e52bf4bcdffb0e8aea46266488960673e8ee5f`.
- Repository: `/Users/jrudnik/labs/jcode`, branch `recovery/2026-07-15`.
- Integrated source checkpoint under the reviewed commit: `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`.
- Constraints honored: no repo/Git/stash/branch/worktree/daemon/channel/build mutation; no network, credentials, real providers, tools/MCP, memory, publication, release, install, external discovery; no pilot execution; no reading of any other G2 reviewer artifact.
- Method: read governing docs and named ledgers, recomputed evidence and review SHA-256 hashes, inspected source and test behavior directly at the fixed SHA, reconciled preserved logs. No Cargo/Nix build or test was run; every source-level question below was adjudicable from source plus the preserved exact logs, so no focused run was requested.

## VERDICT: PASS

I authorize one precisely bounded, fixture-backed, no-secret / no-network / no-tool / no-memory / no-publication / no-daemon pilot, under the exact boundary, observations, stop conditions, rollback, R00 budgets, and evidence limitations stated below. The previously named strict source-fix prerequisite nodes are genuinely closed at the fixed SHA on behavioral and evidentiary grounds, not on file presence. No new blocker node is warranted. Residual defects exist but are all outside the authorized boundary and are converted here into hard stop conditions rather than blockers.

Confidence: medium-high.

## What I verified (behavior and evidence, not file presence)

### 1. Reviewed commit is docs-only over the integrated source

- `git diff --name-only 6c6a4f2c8 16e52bf4b -- crates src` is empty; `git diff --stat 6c6a4f2c8 16e52bf4b -- ':!docs'` is empty. The reviewed commit adds only preserved evidence/ledger documentation over the integrated source checkpoint. Therefore source behavior read at HEAD is authoritative for the fixed SHA.

### 2. Evidence manifest integrity recomputed

Recomputed `SHA256SUMS` file hashes and verified every member with `shasum -a 256 -c`:

- Combined prerequisite validation `SHA256SUMS` = `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291` (matches documented). Members: ALL OK.
- Final R09 / infrastructure `SHA256SUMS` = `113817813b49815d00a10b716e66ab3ed094b28ff6d02fcc60c6d8584c70940a` (matches documented). Members: ALL OK.
- G0 R09 rerun `SHA256SUMS` = `eadb5441bfdf5aef353a2356b2f04454a33912924a07c8eb7e207146ba992614` (matches documented). Members: ALL OK.

### 3. Independent review hashes recomputed and matched

- R01/R03A correction re-reviews: Opus `f382998ca7fd56dbc302a43a7f234b3189e8d56979b58175fec342393fdd17f2` (PASS), Grok `9b265115ace7786b3698e4affeb006463a0b33903f266ccca73f031af77eafc6` (PASS). Match.
- R01/R03A preserved initial history: Opus PASS `b1eed52b...`, Fable provider-failure `d0f9b9ef...`, Grok FAIL `07349da7...`. Match. Disagreement preserved, not erased.
- R02 tier correction re-reviews: Opus `2e5e3c0e0acc63fd22bade8015fdafb003c7fcfb1d0884088345ed92b25388a2` (PASS), Fable `1a5ac839a8ea5a83fda1323427e6688210f7921a30f32dcfbbd8d3d6a513dcf3` (PASS). Match.
- R04 marker fix: Opus `7a8f24490806a6aa30bf4d16947a6e4ff2fee76c67589972fcadc0d96fb1a9de`, Fable `1ec0ceb5c333da18c814ba96a9392fd6fad398b6e3df9b00aafd0c1ee902f73d`. Match; both PASS-narrow with preserved IMPORTANT follow-ups.
- R12 evidence fix: Opus review/rereview and Fable review(FAIL)/rereview(PASS) hashes match documented values; initial disagreement preserved.

### 4. Preservation state intact (R00 contract)

- Four stashes present exactly (`fix-config-hotpath-spam` parts 3/2/1 and `wip before upstream sync`); not popped.
- All recovery worktrees and branches registered, including `orchestrator-s4/s5/s6`, parser branch, and per-seam branches.
- Immutable no-reload build `/Users/jrudnik/.jcode/builds/versions/6c6a4f2c8-dirty-7b4ec829c656/jcode`: size `227257456` bytes and SHA-256 `fd6297d9d9b135f7c8233dc27a6119bea767f74256e6dddccd1a0e5f557c6dd9`, both matching the recorded values.
- Sole working-tree modification is `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`; `git diff | shasum -a 256` reproduces `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`, matching the documented user-controlled edit. That edit removes numbered safety rule 11 (ask before irreversible/destructive/security-sensitive/publication actions) and adds two aspirational lines. It is correctly preserved and NOT adopted as an authority change. G3 must not rely on the removed rule being absent; the standing recovery safety rules still bind.

### 5. Source behavior at the fixed SHA (the substance)

R01/R03A fail-before-mutation preflight (closes C1):
- `crates/jcode-app-core/src/server/client_lifecycle.rs:97-145` defines `preflight_initial_incompatible_advertised_subscribe`. It returns `Ok(false)` when `protocol_version.is_none()` (legacy) and when the evaluated handshake `is_compatible()`. Only for an advertised-and-incompatible initial `Subscribe` does it log, emit the `HandshakeVerdict`, then emit a terminal `ServerEvent::Error`, and return `Ok(true)`.
- The caller at `:494-496` invokes this preflight and `return Ok(())` before any per-client state (processing flags, channels, task) is constructed at `:498+`. This is genuine fail-closed ordering, not mere event ordering. Grok's correction re-review independently traced return before `provider_template.fork_for_new_session` / `Agent::new_with_initial_working_dir`.
- Combined log `6.log` shows the regression `incompatible_initial_subscribe_preflights_before_full_session_initialization` passing 1/1 at the integrated head.

R02 fail-closed tier/auth (closes the stale-cache over-entitlement blocker):
- `crates/jcode-base/src/subscription_api.rs` implements `tier_truth()`, `deny_live_tier_truth(...)` on authoritative denial, HTTP `401`/`403` treated as authoritative denial (`:97`), malformed JSON denial (`:115`), and `UnknownDenied` freshness recording. `LIVE_TIER_DENIED` process marker override and cache-clear-failure fail-closed behavior are present and tested.
- Combined log `3.log` shows 35/35 selected subscription catalog/API tests passing, including `denied_live_tier_overrides_stale_cache_when_durable_clear_fails`, `local_me_fixture_http_401_403_clear_stale_flagship_and_downgrade_auth_readiness`, `ambient_process_tier_env_does_not_grant_entitlement`, and `live_unknown_denied_tier_clears_stale_flagship_cache`. The unrelated zero-test binary lines are correctly not counted as passing evidence.

R12 terminal-evidence emission (closes the strict-fixture and transport-error under-emission nodes):
- `crates/jcode-app-core/src/agent/evidence.rs:142` defines `append_provider_error_response`, invoked from both engines (`turn_loops.rs:141,237,641`; `turn_streaming_mpsc.rs:281,455,916`).
- Strict fixtures exist in `crates/jcode-app-core/src/agent_tests.rs`: `r12_no_tool_turn_emits_and_persists_exactly_one_terminal_provider_response` (`:338`), `r12_blocking_raw_transport_error_persists_terminal_provider_response` (`:485`), `r12_mpsc_stream_event_error_persists_terminal_provider_response` (`:543`), plus the blocking/MPSC parity fixtures named in the ledger. This closes the original "fixture absent" blocker behaviorally.

R04 marker durability (narrow prerequisite):
- `client_disconnect_cleanup.rs:628` and `swarm_persistence_tests.rs` show terminal-marker persistence-before-cleanup and identity/content-conditional removal coverage. Both correction reviews PASS-narrow with preserved non-blocking IMPORTANTs.

### 6. R09 quality gates honest at the fixed SHA (no `--update`)

G0 `run.meta` encoded expected exits before invocation; `manifest.tsv` expected == actual for every gate. Reproduced from preserved logs:
- Classifier 17/17 OK; warning `current=0 baseline=0` green; wildcard `total=16` green.
- Panic red `31 -> 46`; swallowed red `2987 -> 3074`; production-size red `61` oversized findings; test-size red `31`.
- No command used `--update`. Trusted greens green, real red debt visible and attributed.

## Honest treatment of discrepancies (none blocking)

- Swallowed-error count generations: Phase 0 `3,077`, intermediate `3,072`, fixed-HEAD `3,074`. All three preserved; current truth `3,074` reproduced at G0. Honest, not erased.
- Production-size count: several older seam ledger bodies cite `60` violations; the fixed-HEAD G0 rerun and evidence index record `61`. Both are preserved and visible. Current truth is `61`. This is a stale-count-in-older-prose artifact, not a gate failure; it does not block, but G3 must treat `61` as the current production-size baseline truth (evidence limitation below).
- R12 amendment records a full-suite run that failed on two non-R12 tests (`comm_session ... prepare_visible_spawn_session_cleans_session_when_launch_errors`; `tool::selfdev::tests::build_lock_is_removed_on_drop_and_can_be_reacquired`) which passed 2/2 on targeted rerun. Honestly recorded as concurrency/full-suite flakiness. Both concern R05B visible-spawn and selfdev build-lock, which are outside the authorized pilot; they become stop conditions, not blockers.
- The raw coordinated-build transcript did not survive; only task metadata, immutable binary identity (independently verified above), channel state, and check logs remain. Honest evidence limitation; the claim is appropriately narrowed.
- Combined validation logs predate the improved driver and lack internal command-line/timestamp/expected-exit metadata. Preserved as-is and manifest-verified; I compensated by reading source behavior directly rather than trusting logs alone.
- Preserved review disagreements (R01/R03A Grok FAIL then correction PASS; R02 Fable FAIL then PASS; R12 Fable FAIL then PASS; Fable provider failure with no verdict) are all intact. No disagreement was converted into approval.

## Pilot prerequisite adjudication (RESPONSIBILITIES.md rules 1-7)

1. R00 fixes refs, preservation, budgets, rollback, stop conditions: SATISFIED (R00 ledger + reproduced preservation).
2. R01, R02, R03A, R12 full ledgers pass with strict source-fix nodes closed and independently re-reviewed PASS: SATISFIED (behavioral verification above).
3. R06A round-trips the minimal evidence fixture: ledger APPROVED; schema round-trip 11/11 and journal replay tests pass; the minimal pilot round-trip is the pilot EXIT check, defined below.
4. R09 classifier + trusted greens pass without `--update`, red debt visible/attributed: SATISFIED (verified).
5. R07C reporting provably disabled for the fixture, no secret/session leak: ledger APPROVED; kill-switch `is_enabled()` re-checked at every send site; enforced by 17/17 disabled-path tests in a fresh `JCODE_HOME`.
6. R13 enumerates and classifies every `provider_session_id` writer/reset across R02/R04/R12/R13 and proves the one-turn pilot cannot compact: SATISFIED; census complete (three benign single-copy sites identified), arithmetic no-compaction proof holds (needs >10 active turns and ~160k tokens; pilot is one small turn), independently re-reviewed PASS.
7. R04/R05B/R06B/R07A-B/R08 not prerequisites unless the pilot exercises reload/resume/cancel/detached/tool/memory/UI/discovery: the authorized pilot exercises none, so they are excluded and become stop conditions. The narrow R04 marker fix is nonetheless integrated and independently verified.

## Authorized pilot boundary (G3 must honor exactly)

Question permitted: on disposable fixed refs, can one fixture-backed non-secret provider/model route resolve from declared configuration and entitlement provenance, execute exactly one no-tool agent turn, carry canonical build/protocol identity through subscribe, and emit exactly one correlated request/result record while trusted gate verdicts stay unchanged?

Allowed:
- One process, one session, one non-interactive no-tool agent turn.
- Disposable `JCODE_HOME=<tempdir>` and isolated `JCODE_SOCKET`/runtime dir; `JCODE_NO_TELEMETRY=1` exported for every pilot process; symbolic credentials only (for example `fixture-key`); in-process or localhost-mocked `/v1/me` returning a product-owned accepted tier (`plus` or `flagship`).
- The R12 strict happy-path emission (first matrix row): exactly `TurnStarted`, `ProviderRequest`, `ProviderResponse{Ok}`, `TurnFinished{Ok}` at sequences `0..=3`, shared `turn_id`, correlated request/response ID.
- R03A: advertised compatible subscribe path only; `build_hash` treated strictly as a compatibility token; `runtime_identity` additive and optional.

Forbidden inside the pilot: real credentials, payment, network egress, live user daemon, any reload/activation/publication/install/update, tools/MCP, memory/recall, discovery, swarm/visible spawn, cancellation, retry, context-limit or compaction paths, disconnect/takeover, generic-client identity advertisement, and any `--update` of any gate baseline.

## Required observations (pilot passes only if all hold)

1. Route identity R02->R12 equality: the persisted request/result record's account/provider/model/entitlement/route/API method equal the R02-selected identity; no ambient substitution. `JCODE_TIER` in the environment grants nothing.
2. Auth readiness derives from the fixture `/v1/me` accepted response, not key presence; no credential value appears in any observable or evidence file.
3. Subscribe carries the R01 runtime projection additively; the compatible verdict is never represented as canonical-identity equality.
4. Exactly one correlated terminal `ProviderResponse{Ok}` for the one `ProviderRequest`; exactly one `TurnStarted`/`TurnFinished`.
5. R06A exit check: readback from the disposable `JCODE_HOME` yields exactly the four events in order with a truncation-tolerance probe, no loss/duplication/fabrication.
6. R07C: no telemetry session line in `$JCODE_HOME/logs`; nothing written outside the disposable `JCODE_HOME` and pilot worktree.
7. R09: after the pilot, classifier 17/17, warning and wildcard green, and the four red ratchets remain red at their current fixed-HEAD truth with no `--update`.

## Stop conditions (halt and revert only the active slice)

Missing/duplicate terminal response; correlation or sequence mismatch; wrong provider/model/route/tool-count; any `Ok` status after a cancellation signal; any compaction/retry/context-limit event; telemetry enabled or any telemetry log line; evidence written outside the disposable home; a credential value in any observable; a need for real credentials/payment/network/live daemon/reload/tools/MCP/memory/discovery/swarm; any gate `--update`; any move of `main`/refs/stashes/worktrees; or any change to the preserved prompt diff hash. Additionally stop if the run touches disconnect cleanup (R04 IMPORTANT-1: `cleanup_client_connection` returns `Ok(())` even when terminal persistence fails) or visible-spawn/build-lock paths (the two non-R12 flaky tests), both outside boundary.

## Rollback

The pilot is additive and disposable: delete the temporary `JCODE_HOME`/socket, and revert only the isolated pilot fixture/implementation slice commit. No daemon, channel, publication, or baseline is touched, so rollback is a working-tree/commit revert plus tempdir removal. Preservation invariants (four stashes, worktrees, immutable build, prompt diff hash) must reproduce unchanged after rollback.

## R00 budgets G3 must honor

- No new curated sync, replay, merge, rebase, or ratchet `--update`.
- Fixed refs: fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`; source checkpoint `6c6a4f2c8`.
- One bounded pilot only; exceeding the conflict/time/semantic-rewrite budget blocks or narrows rather than expands. Any needed identity writer must already be owned; no unowned writer may be introduced.
- Re-run the R00 preservation reproduction (stashes, worktrees, `vendor/upstream` still `631935dd1...`, prompt diff hash) at pilot start and end.

## Evidence limitations G3 must honor

- Current gate truth is: swallowed `3,074`, production-size `61`, test-size `31`, panic `46`. Older ledger prose citing `60`/`3,077`/`3,072` is preserved history, not current truth.
- No raw build transcript exists; rely on the verified immutable binary hash `fd6297d9...`, not on a reconstructed transcript.
- Combined validation logs lack internal driver metadata; the recovery continuation's dedicated driver is required for the next long validation, and the pilot must generate its own manifest with encoded expected exits, disk/tool snapshots, and hashes.
- R06A schema round-trip is proven; the end-to-end emit->persist->replay is proven only by component fixtures, so the pilot's own readback is the decisive R06A exit evidence.
- Non-blocking residuals remain live for any future widening: R02 in-memory denial marker does not survive process restart after a failed durable clear, and some status-panel surfaces can show stale tier text while admission stays fail-closed; generic remote `/reload` still carries `runtime_identity: None`; R04 disconnect cleanup masks terminal-persistence failure as `Ok(())`. None is exercised by the authorized boundary; each is a stop condition if scope grows.

## Why PASS rather than FAIL

Every named strict source-fix prerequisite is closed on reproduced behavior at the fixed SHA (preflight fail-before-mutation, fail-closed tier/auth, correlated terminal evidence with transport-error emission, marker durability), independently re-reviewed as PASS with disagreements preserved. Evidence manifests, review hashes, the preserved prompt diff, and the immutable build hash all recompute exactly. Preservation is intact. Trusted gates are green and real debt is visibly red with no `--update`. The R06A/R07C/R13 pilot prerequisites are satisfied at ledger and source level, and R04/R05B and all external-effect seams are out of the authorized boundary. The remaining defects are real but confined outside the bounded no-tool/no-secret/no-network/no-daemon pilot and are converted into explicit stop conditions. No reproducible source, test, evidence, or infrastructure gap rises to a new blocker for this narrowly bounded pilot.

## Open questions

- Should generic remote `/reload` be required to carry `Some(runtime_identity)`, or is `Some`-preservation across `Starting -> SocketReady` sufficient? (Low; outside pilot.)
- Per-file assignment of the 61 production-size and 31 test-size violations to owning behavior seams remains unenumerated; owed before any remediation slice, not before this pilot.
- The R02 process-restart escape window after a failed durable clear is untested by design; needs a fixture before any restart-inclusive widening.

## What I did NOT check

- I did not run any Cargo/Nix build or test; all pass counts beyond source inspection are taken from preserved manifest-verified logs.
- I did not execute the pilot (not authorized) and did not exercise any live daemon, network, provider, tool, or telemetry endpoint.
- I did not read any other G2 reviewer artifact or verdict; none was supplied.
- I did not line-audit every consumer of tier gating, every `.rerere-cache` entry, the desktop crate, the telemetry-worker server code, or full replay/crash-recovery surfaces (all outside the bounded pilot).
- I did not independently re-derive upstream product-tier truth; upstream constants remain evidence-only per the ledgers.
