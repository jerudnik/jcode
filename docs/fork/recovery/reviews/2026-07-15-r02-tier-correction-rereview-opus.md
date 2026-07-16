I have full evidence. Every prior finding is resolved. Let me note one residual risk on the process-global marker and check whether 5xx/transient is genuinely excluded (the `is_authoritative_denial_status` gate confirms this). Final assessment ready.

---

# R02 Correction Re-Review: 91475c598 (clean branch)

## Verdict: PASS

Working tree is clean at `91475c598`. The prior F1 (uncommitted divergence) is fully resolved: all changes are now committed. Prior F2 (best-effort durable cache-clear) and the new authoritative-denial and display-truth items are resolved with real, deterministic, no-secret coverage. Final test counts on disk match the cited evidence exactly: subscription API 8/8, catalog 14/14, provider admission/display 16 total (5 subscription-guard focused), TUI denied-label helper present.

---

## Prior findings, re-evaluated

- F1 (working-tree divergence): RESOLVED. `git status` clean; the previously-uncommitted denial latch is now committed as `760dcae11` and refined by `114daee99`. Evidence and code are in sync.
- F2 (best-effort malformed durable clear): RESOLVED. `subscription_api.rs:105-108` malformed path now calls `deny_live_tier_truth(...)?` (propagates error) instead of `let _ = store_cached_tier(None)`. `deny_live_tier_truth` sets `LIVE_TIER_DENIED=true` BEFORE `store_cached_tier(None)?`, so an early `?` return leaves the in-memory marker set. `effective_tier_snapshot` checks the marker first (catalog:243-245), returning `UnknownDenied`/Plus even when the stale Flagship file survives on disk. Proven by `denied_live_tier_overrides_stale_cache_when_durable_clear_fails`.
- F3 (honest display label): RESOLVED and hardened. `jcode_subscription_tier_label` (auth.rs:18-33) maps Live/Cache -> real name, Default -> "unknown (treated as Plus)", UnknownDenied -> "unknown-denied (reason)". Inactive-Flagship truth proven by `jcode_subscription_tier_label_uses_tier_truth_not_raw_tier` (auth_tests.rs).

## New correction items, verified

- 401/403 authoritative denial: `is_authoritative_denial_status` gates on `UNAUTHORIZED | FORBIDDEN` only (subscription_api.rs:138-140). On match: records validation failure, then `deny_live_tier_truth(...)?` clears cache and latches the marker, then still bails with the HTTP error. Proven by `local_me_fixture_http_401_403_clear_stale_flagship_and_downgrade_auth_readiness`: stale Flagship cleared, effective Plus, fable denied, validation false, readiness RequestValid -> CredentialPresent, HTTP error returned.
- 5xx/transient exclusion: correctly NOT in the denial set, so a transient failure does not overwrite cached entitlement truth. Confirmed by the `matches!` gate; documented in ledger `91475c598`.
- Cache and unaccepted-persisted freshness: `effective_tier_defaults_...` asserts `Cache` for accepted persisted tier; `unaccepted_persisted_tier_is_unknown_denied_truth` asserts `UnknownDenied` reason "cached tier is not accepted" for a persisted `pro`. All four freshness states now constructed and asserted.
- Auth-readiness downgrade: gated on `provider_smoke_ok == Some(true)` (auth/mod.rs:181). Denial writes a failed validation record, so `check()` downgrades to CredentialPresent. Proven in the 401/403 fixture.
- Denial-marker ordering: set-before-store in both `deny_live_tier_truth` and `apply_live_tier_truth` UnknownDenied arm; success reset only in `store_cached_tier` when `tier.is_some()` and in the Live arm. Marker persists across a successful denied clear until a later accepted live tier. Ordering is fail-closed and deterministic.

## Findings (severity)

- Low / residual: `LIVE_TIER_DENIED` is a process-global `AtomicBool` with no persistence. A denied result latched in-memory does NOT survive process restart. If durable clear fails AND the process restarts, `cached_tier()` reads the stale Flagship file (config-file only, ambient env still excluded) and admits it until the next `/v1/me`. This is a narrow disk-write-failure-plus-restart window, materially smaller than the original bug, and the next live probe re-denies. Not a blocker; worth a durable "denied" sentinel if you want restart-proof fail-closed. Documented honestly in ledger `760dcae11`.
- Low / test-isolation: the process-global marker relies on `reset_live_tier_denial_for_tests()` in each catalog test under `lock_test_env()`. Correct today; a future test that latches denial without reset under the shared lock could bleed. Cosmetic risk only.
- Informational: single-account model is explicit and intended; no multi-account state. Consistent with product-owned narrow contract.

## Unresolved risks
- Restart-after-clear-failure stale admission window (Low, above). Only viable mitigation is a durable denial sentinel; current in-memory approach is an accepted, documented tradeoff.

## What I did not check
- Did not run Cargo/Nix/tests/build; test-outcome counts (28/28 matrix, R09 17 classifier + expected reds, selfdev TUI build) are taken from coordinator evidence. I independently confirmed on-disk test COUNTS (API 8, catalog 14, provider-subscription file 16) match the cited API 8/8 and catalog 14/14.
- Did not re-audit the full `assessment_for_provider` beyond the readiness mapping already verified in the prior review.
- Did not evaluate reqwest `StatusCode` behavior for non-standard/proxy-injected 401/403 bodies; the gate keys on status only, which is correct.
- Did not inspect R09 ratchet internals; treated as provided infrastructure evidence.

Bottom line: decisive PASS for `91475c598` and the follow-up chain `760dcae11, 5ba8eed30, 114daee99, 6606194ef, 91475c598`. Every prior finding is resolved; the only residual is a documented, low-severity restart-window tradeoff inherent to the in-memory denial marker.
