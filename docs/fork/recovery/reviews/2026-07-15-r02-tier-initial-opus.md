The swarm report tool isn't available in my function set. I'll deliver the full review as my final response.

---

# Adversarial Review: R02 Stale Subscription Tier Fix

## Verdict: **PASS** (with one required-follow-up caveat on uncommitted working-tree divergence)

The reviewed commits `4f104a609` (source) and `8a67d7c7b` (docs) satisfy every stated invariant with real, deterministic, no-secret test coverage. However, the working tree contains **uncommitted changes** that diverge from the reviewed commit and from the cited rerun evidence. My PASS is scoped to the committed state `4f104a609..8a67d7c7b`; the uncommitted delta is a separate, unreviewed artifact flagged below.

---

## Findings by severity

### F1 (Medium, scope/integrity): Working tree diverges from reviewed commit and from cited evidence
`git status` shows `subscription_api.rs` and `subscription_catalog.rs` modified vs `HEAD` (`8a67d7c7b`). The uncommitted diff:
- Adds a `LIVE_TIER_DENIED: AtomicBool` process-global denial latch and `deny_live_tier_truth()` (`subscription_catalog.rs` working tree lines ~15, ~292-296, ~242-245).
- Rewrites the malformed-JSON path in `subscription_api.rs:105` from the committed best-effort `let _ = store_cached_tier(None);` to a hard `deny_live_tier_truth(...)?`.
- Adds a 13th catalog test (`denied_live_tier_overrides_stale_cache_when_durable_clear_fails`) plus `FORCE_TIER_STORE_ERROR` test seam.

Consequence: the cited rerun evidence (**subscription API 7/7, catalog 12/12**) matches the *committed* state exactly (I counted 7 `#[test]/#[tokio::test]` in committed `subscription_api.rs`, 12 in committed `subscription_catalog.rs`). The working tree has **13** catalog tests, so the "12/12" green does not describe the on-disk code. This is not a defect in the reviewed commit, but the evidence and the working tree are out of sync. The uncommitted work is unreviewed and unbuilt per instructions.

### F2 (Low, real but bounded in committed code): Malformed-JSON durable-clear is best-effort
In committed `4f104a609`, `subscription_api.rs:98-102` handles malformed JSON with `let _ = subscription_catalog::store_cached_tier(None);` and ignores a write failure. If the durable clear fails (disk error), stale `JCODE_TIER=flagship` in the config file would survive, and `cached_tier()` would still return Flagship. The unknown-denied path from a *parseable* body (`apply_live_tier_truth`) correctly propagates errors via `?`, but the malformed branch does not. This is precisely the gap the uncommitted `deny_live_tier_truth` + in-memory `LIVE_TIER_DENIED` latch was written to close. In the committed state this is a narrow, disk-failure-only residual risk, not a normal-path stale-Flagship leak. The committed test `local_me_fixture_malformed_json_clears_stale_flagship_cache` proves the happy-path clear works; it does not exercise a store failure. Acceptable for PASS; noted as the strongest remaining edge.

### F3 (Informational): `unknown-denied` display label is honest
`auth.rs:158-162` now renders `unknown-denied (<reason>)` instead of echoing the raw wire tier. The API return contract cannot misrepresent denied truth: `SubscriptionMe.tier` is `Option<String>` normalized so non-string/blank become `None`, `parsed_tier()` returns `None`, `tier_truth().freshness == UnknownDenied`, and the display path no longer surfaces an unaccepted raw string as a plan name.

---

## Invariant-by-invariant verdict (committed state)

| Invariant | Verdict | Evidence |
|---|---|---|
| Product-owned accepted tier contract | PASS | `JcodeTier::ALL = [Plus, Flagship]` (catalog:27); `parse` accepts only `plus`/`flagship` (catalog:59-65) |
| No authority from upstream business constants | PASS | Only fork-owned `Plus`/`Flagship` accepted; ledger records `Pro`/`Max`/`Ultra` as evidence-only. Test `live_tier_truth_accepts_only_product_owned_tiers` denies `pro`/`max`/`ultra` |
| Unknown/absent/malformed/contradictory cannot preserve stale Flagship | PASS (committed happy path); see F2 for disk-failure edge | `classify_live_tier` denies all four; `apply_live_tier_truth` clears cache on `UnknownDenied`; api test `..._unknown_absent_malformed_and_contradictory_tier_clear_stale_flagship` asserts `cached_tier()==None` and `claude-fable-5` denied |
| Every accepted tier covered | PASS | Loops over `JcodeTier::ALL` in catalog, api, and provider admission/display tests |
| Freshness states covered | PASS | All four `Live/Cache/Default/UnknownDenied` constructed and asserted (Cache: 2 asserts committed catalog, 1 api; Default: catalog:472/526; Live+UnknownDenied throughout) |
| Model admission and picker/display agree | PASS | `test_subscription_admission_and_display_agree_for_each_accepted_tier` cross-checks `ensure_model_allowed_for_subscription` vs `filtered_display_models` per tier |
| Auth readiness and route identity proved | PASS | Readiness gated on `provider_smoke_ok==Some(true)` (auth/mod.rs:181-182 -> `RequestValid`); api fixture test proves `CredentialPresent` before, `RequestValid` after accepted `/me`. Route identity: `canonical_model_id("id@openai")==Some(id)` asserted |
| Saved credentials/process env cannot silently grant entitlement | PASS | `cached_tier()` uses `load_env_value_from_config_file` (ignores process env; provider-env/lib.rs:245-275), not `load_env_value_from_env_or_config`. Test `ambient_process_tier_env_does_not_grant_entitlement` asserts `JCODE_TIER=flagship` yields `Default`/denied |
| API return contract cannot misrepresent denied truth | PASS | `Option<String>` tier + `tier_truth()` + honest `unknown-denied` label (auth.rs); see F3 |

---

## Responsibility-boundary inspection (beyond changed lines)
- Confirmed no other consumer reads `JCODE_TIER` from ambient env for entitlement (`grep` of `subscription_catalog.rs`; only line 256/273 config-file reads).
- Confirmed `is_runtime_mode_enabled()` reads `JCODE_SUBSCRIPTION_ACTIVE_ENV` (activation flag only), not tier authority.
- Confirmed the `SubscriptionMe.tier: Option<String>` change does not break unrelated `.tier` consumers: `support.rs:49` is a separate diagnostics struct; other `.tier` hits are `perf`/`google`/`safety` unrelated types.
- `find_curated_model` normalizes and matches id/alias case-insensitively, backing the route-identity suffix claim.

## Missing tests
- No test exercises malformed-JSON with a forced `store_cached_tier` failure in the committed state (F2). The uncommitted tree adds exactly this, but it is out of scope.
- No test asserts `Cache` freshness surviving across a no-live-probe restart beyond the seed-and-read path (covered indirectly; adequate).

## Scope-creep assessment
Committed change is tightly scoped to R02 tier fail-closed: catalog classification, api fetch handling, one-line honest label in TUI, and provider test hardening. No unrelated refactors. The docs commit is append-only. **No scope creep in the reviewed commits.** The uncommitted working-tree work (global denial latch) is arguably beyond the minimal fix and is the item most worth a fresh review before it is committed.

## What I did NOT check
- I did not run Cargo/Nix, build, or execute any test (per constraints); all "pass" claims about test *outcomes* rely on the coordinator's cited rerun, which I confirmed matches the committed test *counts* but not the working tree (F1).
- I did not verify the R09 trusted-greens or TUI selfdev build independently; I treated them as provided infrastructure evidence.
- I did not inspect the full auth `assessment_for_provider` implementation beyond the readiness mapping at mod.rs:176-208.
- I did not evaluate runtime concurrency semantics of the uncommitted `AtomicBool` latch (out of scope; unreviewed).

**Bottom line: PASS for `4f104a609` + `8a67d7c7b`.** Required follow-up before merge: reconcile the uncommitted working-tree changes (F1) either by committing and re-reviewing them or reverting, so the shipped code, the tests, and the cited green evidence describe the same artifact.
