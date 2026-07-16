# Phase 6 Independent Architecture and Maintainability Review (Fable)

Role: independent Architect under `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` lines 227-247. Read-only. Informed by, and evidence-checked against, the committed independent spot audit.

## Fixed refs

- Review head (Phase 6 docs): `6cbed3a95450a2b22637c63145b31fb5aeda0d87` == current HEAD on `recovery/2026-07-15` (verified live).
- Accepted Phase 6 source head: `51168d16e9c708ae4afff09a6fc6402642d17782` (head is two docs-only commits above it; `git diff --name-only 51168d16e..6cbed3a95` contains zero non-`docs/` paths, verified).
- Phase 5 base: `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`. Integrated range: 99 commits, 528 files, 15,219 insertions / 257 deletions; production/test Rust+script surface is 45 files, 4,563 insertions / 238 deletions (recomputed).
- Working tree: sole dirty path `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`, diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00` (reproduced live); exactly 4 stashes (verified).
- `PROTOCOL_VERSION = 1` at `crates/jcode-protocol/src/lib.rs:26`; `crates/jcode-protocol` and `crates/jcode-session-types` diffs across the Phase 5 range are empty (verified; `SessionLogStatus::Interrupted` pre-existed at `evidence.rs:175`).

## Spot-audit inputs

- Report: `docs/fork/recovery/reviews/2026-07-16-phase6-spot-check-opus.md`, SHA-256 `092dbf4ec862b23b8d778f029772b46b434202e816622bd1f71c4bfa1f759dcc` (recomputed live, matches). Verdict PASS, sole LOW finding: "76 expected-exit entries" was a physical-line count, real checks are 62.
- I independently re-derived the LOW finding: parsed `evidence/2026-07-16-phase6-final-audit/accepted/manifest.tsv` myself: 62 real checks, 0 expected/actual mismatches, exactly four expected-red gates (`panic`, `swallowed`, `code_size`, `test_size`).
- The correction is applied append-only: package `README.md:13-15` now states "62 distinct expected-exit checks / 76 physical lines", mirrored in `PROGRESS.md:452-453` and `evidence/README.md:160`, with the corrected package `SHA256SUMS` SHA-256 `ca8ff5b9f3b6c09dc0ff05de9b3c1c426fc2373706eeeca26cad87126f2e14d8` (recomputed live, matches; both the tracked file and the `6cbed3a95` blob hash to it). `shasum -c SHA256SUMS`: 88/88 OK, `accepted/verify_raw.sh` all OK (rerun live).
- The audit's second informational item (panic 46 at the G0 head versus 31 -> 48 at the Phase 6 head) is honest head-relative debt reporting, not a contradiction; I reran `scripts/check_panic_budget.py` at the working tree and its finding list byte-matches the preserved `accepted/raw/panic.txt.gz` (31 -> 48, same per-file attribution).

## Verdict

**PASS.**

Zero unresolved IMPORTANT or CRITICAL architecture or maintainability findings. Code, tests, active docs, ledgers, and claim limits describe the same system at the fixed refs. All findings below are LOW and are either already owned by a named deferred item with a trigger, or are recommendations that do not gate sign-off.

## Prioritized architecture/maintainability findings

1. **LOW - W7 emission-consolidation trigger is effectively already met by W1.** W1 duplicated near-identical provider-error-evidence blocks (context-limit retry, provider-open, stream-transport) across both turn engines: `crates/jcode-app-core/src/agent/turn_loops.rs` (1,251 -> 1,314 LOC) and `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs` (1,774 -> 1,840 LOC), both already on the R09 production-size red list. The RECOVERY_PLAN §4 deferred-risk trigger for W7 is "growth of duplicated emission/liveness logic in any new fix", which is exactly what W1 did, deliberately and with pinned behavior (11 `r12_` fixtures). Recommendation: treat the W7 R12 helper consolidation as ripe, not dormant, at the next opportunity. Not blocking: behavior is pinned, debt is visible expected-red, and duplication was the reviewed lower-risk choice for a recovery slice.

2. **LOW - the turn-interruption signal crosses the agent -> server seam as an opaque string.** `TurnInterruptedError` (`crates/jcode-app-core/src/agent/evidence.rs:227`) is private to the agent module; the server-side test `client_lifecycle_tests.rs:481` asserts `error.to_string() == "turn interrupted"`. Server error consumers cannot downcast the marker type. Today no server consumer needs to distinguish interruption (both `"stopped"` and `"failed"` are in `member_status_is_dead`, so liveness/salvage behavior is identical), but if one ever does, the only handle is a magic display string. Recommendation for W7: export a typed predicate (e.g. `Agent::error_is_turn_interruption(&anyhow::Error) -> bool`) instead of widening visibility of the marker type.

3. **LOW - member status can label a cancelled detached turn `failed`.** The detached-turn `Err` consumers (`client_lifecycle.rs:785-796`, `client_actions.rs` agent-task handler) map any `Err`, including `TurnInterruptedError`, to member status `"failed"` with detail `turn interrupted`, while `cancel_processing_message` separately writes `"stopped"`/`"cancelled"`. In the owned-task path there is no race (cancel awaits the handle and takes the task). In the `NO_LOCAL_TASK` branch (post-reload/attach), final status is timing-dependent between `"failed"` and `"stopped"`. This is strictly better than the pre-W1 behavior, where the same race could label a cancelled turn `"ready"`/`"completed"`, and both outcomes are dead statuses for reclaim purposes. Cosmetic/observability only; fold into the W7 seam cleanup.

4. **LOW - `ClassifiedEvidenceError` severs the `anyhow` source chain.** `impl std::error::Error for ClassifiedEvidenceError {}` (`evidence.rs:272`) has no `source()`, so `downcast_ref::<StreamError>()` at `client_lifecycle.rs:798` / `client_actions.rs:1137` cannot see through a wrapped transport error, potentially losing `retry_after_secs` and rate-limit telemetry classification. I verified the mid-stream `StreamEvent::Error` path (the one that actually carries `retry_after_secs`) still returns an unwrapped `StreamError` (`turn_loops.rs:695`, `turn_streaming_mpsc.rs:1005`), so no current consumer breaks. This exact residual is already preserved as a non-blocking observation in the R12 ledger rereview amendment. Trivial fix available (implement `source()` returning the inner error); should ride along with W7.

5. **LOW - `append_progress_provenance` is unbounded where the code it replaced truncated at 120 chars.** `crates/jcode-plan/src/lib.rs:70-79` appends `" | note"` per reclaim/takeover to `checkpoint_summary` with no length cap. Reclaims are capped by `dead_assignee_reclaims`, but assignment takeovers (`comm_control.rs:1740-1750`) are not, so a long-lived churny plan can grow this field without limit and it is broadcast/persisted with the plan. History preservation was the reviewed intent (it fixed silent overwrite of prior checkpoints, asserted by tests), so this is a bounded-growth hardening recommendation, not a defect: add a generous cap (for example keep last N notes or last 2 KB) when W7 touches the plan seam.

6. **Observation, no action - test-injection env var in production file.** `JCODE_TEST_VISIBLE_SPAWN_ERROR` in `comm_session.rs:135` is `#[cfg(test)]`-gated, so it compiles out of release builds. Acceptable seam; the purer alternative (injected spawn closure) already exists in `prepare_visible_spawn_session` and could absorb it later.

No CRITICAL, HIGH, or IMPORTANT findings. No material mismatch found between code, tests, active docs, ledgers, and claim limits in any sampled surface.

## Cross-seam authority assessment

- **Protocol/schema stability (R03A/R06A):** empty `jcode-protocol` and `jcode-session-types` diffs across the whole Phase 5 range; `PROTOCOL_VERSION` unchanged at 1. W2's fallback provenance rides in existing free-text `detail`/`task_label` fields rather than widening the wire or replay schema, with the naming-governance decision explicitly pinned to `docs/proposals/observability-field-naming.md` instead of being improvised. This is the correct restraint.
- **Identity/runtime authority (R01/R13):** zero new `provider_session_id` assignments in the range (grep-verified); runtime-identity assignment additions are confined to `recovery_pilot_tests.rs` and the `mod tests` block of `reload_state.rs`. The new `reload_state.rs` fixture explicitly asserts the R01-vs-R03A separation (identity projection distinguishes dirty same-commit restarts while the compatibility verdict remains a build-hash/protocol decision), which encodes the authority boundary as an executable contract. Good.
- **Liveness authority (W2/R05B):** the dead-status predicate is now one function, `swarm_verbs::member_status_is_dead`, delegated to from `server/swarm.rs:373` and `comm_control.rs:734`. Three copies became one; deletion over abstraction, as the plan required.
- **Daemon/reload authority (W6/R10 vs R01):** installers no longer reload the daemon by default; reload requires `JCODE_RELOAD_SERVER=1` with `JCODE_SKIP_SERVER_RELOAD=1` retained as hard disable, consistently across `install.sh`, `install_release.sh`, `install.ps1`, and the R10 ledger/review artifacts. This moves live-daemon target selection back under R01, closing a real authority leak.
- **Consent authority (W5/R08A):** `onboarding_flow_control.rs:1580` timeout now routes to `onboarding_handle_login_failed(None)` instead of importing every checked credential. One-line production diff reusing an existing failure path; minimal and correct.
- **Test-only production seams (W4/R02):** `apply_subscription_me_fixture` is gated `#[cfg(any(test, feature = "test-support"))]` and reuses the exact production parse/cache/validation path (`apply_subscription_me_body`), so offline fixtures exercise the real seam rather than a parallel copy. Right design.

## Performance / security / maintainability assessment

**Security: clearly improved over both ancestors.**
- Persisted evidence error classes are a closed six-label enum (`EvidenceErrorClass::as_str`) instead of the first 120 chars of raw provider error text, with adversarial secret-prefix/no-colon fixtures proving secrets are not persisted (W1).
- Update path fails closed on missing `SHA256SUMS` (`update.rs::verify_asset_checksum_required`), and all three installers verify digests with format validation before install (W6).
- Releases stage as drafts and publish only after the full asset set plus checksums exist, in both `quick-release.sh` and `.github/workflows/release.yml`, eliminating the partially-published-release window (W6).
- Onboarding timeout no longer imports credentials silently (W5).

**Performance/liveness: one measured-free but structurally sound improvement; no runtime overclaim.** `active_pids.rs` splits lock acquisition into `acquire_writer` (blocking, short critical sections) and `acquire_bounded` (64 `try_lock` attempts with `yield_now`, fail-closed) for observer/sweep paths, with a test proving contended sweeps return a bounded default without deleting markers. This removes an unbounded-blocking hazard. The recovery correctly makes no unmeasured runtime-performance claims anywhere I checked; the "more performant" bar of the prompt is satisfied only in the liveness/fail-closed structural sense plus the absence of regressions in trusted gates, and the docs do not claim more than that.

**Maintainability: improved with honest, attributed debt.** Explicit outcome types (`SwarmSpawnCreation`, `CleanupClientConnectionOutcome`/`TerminalPersistenceOutcome`, `ReloadInterruptedToolResult`) replace boolean tuples and sentinel errors at three seams, and `resolve_swarm_spawn_creation` is a pure, directly-tested policy function. None of these is speculative abstraction: each has multiple call-site consumers or direct tests. The debt side (findings 1-5, R09 file-size and panic-count growth) is fully visible in expected-red gates with per-file attribution, not hidden. Compared to the fork/upstream ancestors this range replaces silent-consent, silent-checksum-skip, silent history-overwrite, and boolean-tuple seams with explicit contracts, which is the "more elegant/maintainable" bar met without exaggeration.

## Validation performed

- Recomputed spot-audit report hash, corrected package `SHA256SUMS` hash (tracked file and git blob), `shasum -c` 88/88 OK, `accepted/verify_raw.sh` all OK.
- Independently parsed `accepted/manifest.tsv`: 62 real checks, 0 mismatches, four expected-red (`panic`, `swallowed`, `code_size`, `test_size`) at 1/1.
- Reran `scripts/check_panic_budget.py` (cheap, read-only) at the working tree; its output matches the preserved accepted raw log byte-for-content (31 -> 48, identical per-file list).
- Read the full production diffs for: `agent/evidence.rs`, `agent/turn_loops.rs`, `agent/turn_streaming_mpsc.rs` (W1); `server/comm_session.rs`, `server/swarm.rs`, `swarm_verbs.rs`, `jcode-plan/src/lib.rs`, `server/comm_control.rs`, `tool/communicate.rs` (W2); `server/client_disconnect_cleanup.rs`, `server/client_lifecycle.rs`, `jcode-storage/src/active_pids.rs` (W3); `jcode-base/src/subscription_api.rs` (W4); `onboarding_flow_control.rs` (W5); `update.rs`, `install.sh`, `quick-release.sh`, `.github/workflows/release.yml` (W6); `reload_state.rs` tests (R01/R03A boundary).
- Grep-verified: no new `provider_session_id` assignment in the range; `TurnInterruptedError` confined to the agent module; `StreamError` consumer set (4 downcast sites) and the unwrapped mid-stream return paths; `JCODE_TEST_VISIBLE_SPAWN_ERROR` used only under `#[cfg(test)]` plus tests.
- Verified `51168d16e..6cbed3a95` is docs-only; verified prompt-diff hash and stash count; sampled R02/R05B/R12 ledger tails and the seams README Phase 6 rollup against the code they describe (all consistent, including the preserved W2 failed fixture attempt, W5 evidence-failure adjudication, W6 initial Grok FAIL, and the W4 SIGTERM'd invalid attempt).

## Deferred risks and triggers

I reviewed RECOVERY_PLAN §4 directly: every deferred item (R09 debt paydown, R02 restart-denial residual, R01/R03A remote-reload identity residual, R12/R13 `turn.rs:724` window, R03B WebSocket, R07A/R07C, hot-path dedup stashes, W7 refactors, W5 upstreaming, R06B/R07B/R08B-D) carries an owner, a reason, an evidence gap, and a concrete escalation trigger. All are genuinely low/medium, fail-safe in their current posture, and none is a disguised correctness defect. Amendments from this review: finding 1 argues the W7 trigger has already fired (owner should schedule, not wait); findings 2-5 should be folded into W7's scope when it runs. No new deferred item requires creation; no existing item lacks a trigger.

## Open questions

- Whether the W7 consolidation slice will be scheduled before the next R12-adjacent change lands; if another fix grows the duplicated emission blocks a third time, the deferral stops being credible (this is the plan's own trigger, restated).
- Live runtime behavior (daemon, reload, providers, network, real releases) remains deliberately unexercised by design; the PASS is bounded to the offline/fixture surface exactly as the ledgers state.
- The spot audit's non-blocking plan-head lineage question (`76ead5607` vs `e2464666d`) remains unresolved and remains non-blocking; it does not affect any fixed ref I reviewed.

## Confidence

High on: architecture and authority-boundary assessment of the six workstreams, evidence integrity, docs/ledger/code coherence, protocol/schema invariance, and the closed error-class/consent/checksum security improvements (all read directly from the fixed-ref sources and byte-verified evidence). Medium on: runtime behavior of the concurrent server paths (cancel/status races, draft-release race handling), which were reviewed statically and via the preserved fixture results, not exercised live, consistent with the recovery's declared no-daemon/no-network boundary.

## What I did not check

- Did not run any Cargo build or test suite (the decisive questions were answerable from byte-verified preserved logs plus source reading; per instruction).
- Did not exercise any live daemon, reload, provider, network, credential, release, installer, or updater path.
- Did not read all 66 review artifacts or all 17 ledgers end-to-end; I read the R02/R05B/R12 ledger tails, seams README, PROGRESS tail, evidence README, RECOVERY_PLAN §4, and the full spot audit, and verified their claims against the code.
- Did not re-derive the 288/243 patch-ID divergence measurement or re-review upstream commit clusters.
- Did not audit the PowerShell installer (`install.ps1`) line by line; only its opt-in reload gate and the ledger/review claims about it.
- Did not modify the repository, refs, stashes, worktrees, prompt, index, or the /tmp spot report.

Reported for the coordinator and the joint Sol/Fable sign-off. I am not the final decision maker.
