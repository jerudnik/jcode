No `swarm` tool is exposed in my function set, so the full report is returned inline as before.

---

# R02 Correction Re-Review at 91475c598

Scope: follow-up commits 760dcae11, 5ba8eed30, 114daee99, 6606194ef, 91475c598 on top of the previously reviewed pair. Read-only. No cargo/nix/network/reload, no edits. Worktree is clean at 91475c598.

## VERDICT: PASS

All three substantive findings from my prior review (F1, F2, F3) and both coverage gaps (F5 pair) are resolved with code plus deterministic tests. Remaining items are low-severity residual risks, documented below.

## Prior finding disposition

**F1 (HIGH, durable cache-clear failure fail-open): RESOLVED.**
- `subscription_catalog.rs`: process-global `LIVE_TIER_DENIED: AtomicBool` set before any denied-truth clear attempt (`deny_live_tier_truth`, and the `UnknownDenied` arm of `apply_live_tier_truth`). `effective_tier_snapshot()` checks the marker first, so a failed durable clear still yields `unknown_denied("live tier denied")` and Plus admission.
- Ordering fixed vs 760dcae11: 114daee99 removed the `store(false)` after successful clear, so the marker now persists after any authoritative denial and is cleared only by an accepted live tier (`store_cached_tier(Some(_))` or the `Live` arm). I verified `store_cached_tier(Some)` has no production callers besides `apply_live_tier_truth`, so the marker cannot be cleared by a non-live path.
- Test `denied_live_tier_overrides_stale_cache_when_durable_clear_fails` (catalog.rs ~:591) forces the write failure via `FORCE_TIER_STORE_ERROR`, confirms Flagship stays on disk yet effective tier is Plus and Fable-5 is denied, then confirms accepted live truth restores entitlement.

**F2 (MEDIUM, 401/403 preserves stale Flagship): RESOLVED.**
- `subscription_api.rs`: `is_authoritative_denial_status` matches exactly `UNAUTHORIZED | FORBIDDEN`; that path records validation failure, calls `deny_live_tier_truth("subscription authorization denied")`, and still bails with the HTTP error, so the return contract does not misrepresent denial. 5xx and transport errors deliberately stay outside the denial path (offline grace), now explicitly documented in the ledger append at 91475c598, which closes my "undocumented choice" objection.
- Test `local_me_fixture_http_401_403_clear_stale_flagship_and_downgrade_auth_readiness` covers both statuses, cache clear, Plus admission, UnknownDenied freshness, validation failure, returned error string, and the auth-readiness downgrade RequestValid -> CredentialPresent (my missing-test item 5).

**F3 (MEDIUM, TUI label misrepresents inactive Flagship): RESOLVED.**
- `auth.rs`: new pure helper `jcode_subscription_tier_label` derives the label from `tier_truth.freshness`, not raw `parsed_tier()`. Inactive Flagship now renders "unknown-denied (inactive subscription status)".
- Test `jcode_subscription_tier_label_uses_tier_truth_not_raw_tier` covers exactly the contradictory case plus accepted Flagship.

**F5 (LOW, freshness coverage gaps): RESOLVED.**
- `Cache` freshness now asserted in `effective_tier_defaults_to_plus_when_no_live_or_cached_truth` (catalog.rs ~:523) and again in the clear-failure test's recovery step.
- New test `unaccepted_persisted_tier_is_unknown_denied_truth` covers the persisted `JCODE_TIER=pro` branch including the exact reason string "cached tier is not accepted".

**F4 (LOW, status surfaces bypass snapshot): STILL OPEN.**
- `auth.rs` `show_jcode_subscription_status` (line ~91, ~115) and `support.rs:109` still read `cached_tier()` directly with "unknown (treated as Plus)" wording. Concretely: after a denial whose durable clear failed, the `/subscription` status panel would show "Tier: Flagship" while admission is Plus. Presentational only, admission is unaffected. Downgraded to residual risk, not a blocker.

## Marker ordering analysis (requested)

I walked the interleavings of concurrent denied and accepted fetches: every ordering ends either correctly entitled (live accepted response wins) or fail-safe under-entitled (Default/denied Plus). No interleaving grants stale Flagship, because the only marker-clearing writes are preceded by a successful accepted-tier store. `SeqCst` throughout. The marker is process-global rather than per-account; acceptable for the single jcode subscription account and stated in the ledger.

## Unresolved residual risks

1. Process-restart escape (LOW-MEDIUM): the denial marker is in-memory. If the durable clear failed and the process restarts, the stale Flagship file is re-admitted as `Cache` truth until the next `/me` probe runs. Bounded by the startup status fetch, but a window exists. Not covered by tests, inherent to the chosen design.
2. F4 status surfaces (LOW), as above.
3. 5xx/transport failures preserve cached entitlement indefinitely if probes keep failing (accepted design, now documented).
4. `JCODE_SUBSCRIPTION_ACTIVE` remains process-env, but it only enables restriction, never grants.

## Validation evidence check (static, not re-run)

- Static test counts at 91475c598 match every coordinator claim: subscription_api.rs 8 tests (8/8), subscription_catalog.rs 14 tests (14/14), catalog_subscription.rs 5 subscription tests of 16 in file (5/5, consistent with the previously corrected exact-5 rerun), TUI denied-label 1 test (1/1). 8+14+5+1 = 28, matching the 28/28 matrix.
- R09 classifier 17 green, four expected ratchets red with no `--update`, TUI check, selfdev-profile build pass, no reload: accepted as reported, not independently executed (read-only mandate). Neither correction commit touches R09 baselines or `scripts/check_*`.
- Note: the 91475c598 ledger records the correction slice itself as static-only validated pre-commit; the 28/28 matrix is the coordinator's post-commit evidence. The two statements are consistent.

## Scope-creep assessment

None. The correction slice touches only the R02 boundary files plus the append-only ledger. Fix and tests are split into separate commits (114daee99 / 6606194ef) as required. No pricing, model-floor, or routing changes; no upstream tier constants imported (pro/max/ultra appear only as denied fixtures).

## Not checked

- No execution of any tests, builds, ratchets, or reloads; all pass counts beyond static inventory are taken from coordinator evidence.
- No live endpoint behavior, no multi-process concurrent env-file write audit.
- Desktop crate untouched by these commits and not inspected.
- SHA-256 hashes in pre-existing ledger sections not revalidated.

Bottom line: PASS. Every actionable finding from the prior FAIL is remediated with exact tests; what remains is documented residual risk (restart window after failed durable clear, and the F4 status-panel wording) suitable for a follow-up slice, not a blocker.
