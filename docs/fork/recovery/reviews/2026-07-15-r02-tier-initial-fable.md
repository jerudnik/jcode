I could not find a `swarm` tool in my available function set, so I cannot submit the report via `swarm action="report"`. I am returning the full review here as instructed by the task prompt.

---

# Second Independent Adversarial Review: R02 stale-tier fix

**Scope:** source commit `4f104a609` ("fix: fail closed stale jcode subscription tier") and docs commit `8a67d7c7b` ("docs: append R02 stale tier follow-up"), baseline `e7e47e42c`, repo `/Users/jrudnik/labs/jcode-fix-r02-tier`. Read-only. No cargo, no nix, no network, no reload.

**Note on repo state during review:** the branch advanced mid-review. When I started, the worktree carried uncommitted changes to `subscription_api.rs` / `subscription_catalog.rs`; those were committed by another actor as `760dcae11` ("fix: deny stale tier when cache clear fails") and `5ba8eed30` (docs amendment crediting "reviewer B") at 13:54 UTC. Those commits are **outside my review scope** but corroborate my Finding F1 independently. The worktree is now clean at `5ba8eed30`.

## VERDICT: FAIL (for the reviewed pair 4f104a609 + 8a67d7c7b)

The reviewed source commit does not fully satisfy the required invariant that unknown/absent/malformed/contradictory `/v1/me` truth cannot preserve stale Flagship access: under a durable cache-clear failure the stale Flagship cache survives and remains admitted, silently in the malformed-JSON path. The branch itself already contains the out-of-scope remediation (`760dcae11`), which confirms the finding is real, but the pair under review fails on it.

## Findings, ordered by severity

### F1 — HIGH: fail-open under durable cache-clear failure (invariant violation)
- `crates/jcode-base/src/subscription_api.rs:105` (at 4f104a609): the malformed-JSON path uses best-effort clearing: `let _ = subscription_catalog::store_cached_tier(None);`. If the env-file write/remove fails (fs error, permissions, disk), the stale `JCODE_TIER=flagship` line survives, `cached_tier()` keeps returning Flagship, and `is_model_allowed_for_current_tier("claude-fable-5")` keeps returning true. The error is swallowed with no marker and no surfacing.
- `crates/jcode-base/src/subscription_catalog.rs:284` (at 4f104a609): `SubscriptionTierFreshness::UnknownDenied => store_cached_tier(None)?` propagates the error (so `fetch_subscription_me` returns Err), but admission checks (`effective_tier()`, `is_model_allowed_for_current_tier`) never consult that error. Stale Flagship on disk remains admitted after an authoritative live denial.
- This is exactly the hole `760dcae11` later closes with the in-memory `LIVE_TIER_DENIED` marker plus mandatory `deny_live_tier_truth`. No test in 4f104a609 covers the clear-failure path (the later commit adds `denied_live_tier_overrides_stale_cache_when_durable_clear_fails`).

### F2 — MEDIUM: authoritative HTTP-level denial preserves stale Flagship
- `crates/jcode-base/src/subscription_api.rs:89-97` (at 4f104a609): any non-2xx `/v1/me` response bails early without touching the tier cache. For transient 5xx this offline-grace is defensible, but a live 401/403 (revoked key, cancelled subscription) is an authoritative server denial, and stale cached Flagship access is preserved indefinitely. The required invariant enumerates tier-shaped denials; a 403 is a contradictory live truth in every practical sense. Untested, and undocumented as a deliberate choice in the 8a67d7c7b ledger append. At minimum it needs a documented decision and a test pinning the intended behavior.

### F3 — MEDIUM: TUI activity card can misrepresent denied truth as "Flagship"
- `crates/jcode-tui/src/tui/app/auth.rs:158-162` (at 4f104a609): the label uses `me.parsed_tier()` and only falls back to `unknown-denied (reason)` when the wire tier fails to parse. For the contradictory fixture (`tier: "flagship"`, `status: "inactive"`), `parsed_tier()` is `Some(Flagship)` while `tier_truth()` is `UnknownDenied`. The card renders "Tier: Flagship (inactive)" while admission is Plus. The computed `tier_truth` is right there but its `parsed_tier` is not used for the label. Admission and the model picker do agree (both route through `effective_tier()`), so the picker/display invariant holds, but the status surface can misstate denied entitlement. No test covers this display path.

### F4 — LOW: status surfaces bypass the snapshot API
- `crates/jcode-tui/src/tui/app/auth.rs:74,96-98` (`show_jcode_subscription_status`): renders `cached_tier()` with stale wording "unknown (treated as Plus)", not distinguishing Default from UnknownDenied ("cached tier is not accepted"). `crates/jcode-tui/src/tui/app/support.rs:109` likewise reads `cached_tier()` directly. Effective tier is still safe (Plus), so this is presentational drift, but the new `effective_tier_snapshot()` freshness/reason contract was built precisely to feed these surfaces and they were not migrated.

### F5 — LOW: freshness-state coverage is incomplete
- `SubscriptionTierFreshness::Cache` is never asserted by any test at 4f104a609 (grep: it appears only in source arms, `subscription_catalog.rs:241,285`, `subscription_api.rs:121`). `effective_tier_defaults_to_plus_when_no_live_or_cached_truth` stores Flagship and checks admission but never asserts `freshness == Cache`.
- The `effective_tier_snapshot()` branch at `subscription_catalog.rs:246-248` (persisted file contains an unaccepted value like `JCODE_TIER=pro` → `unknown_denied("cached tier is not accepted")`) has no test. The "freshness states covered" invariant is therefore only 3 of 4 proven.

## Invariant-by-invariant assessment (at 4f104a609)

| Invariant | Status | Evidence |
|---|---|---|
| Product-owned accepted tier contract | PASS | `JcodeTier::parse` accepts only `plus`/`flagship` (`subscription_catalog.rs:53-59`); `classify_live_tier` (:117-133) denies everything else; test `live_tier_truth_accepts_only_product_owned_tiers` (:418) covers `pro`/`max`/`ultra`/blank/absent/inactive |
| No authority imported from upstream constants | PASS | No `Pro`/`Max`/`Ultra` variants, no pricing/budget changes vs baseline (`retail_price_usd`/`usable_budget_usd` unchanged at :23-35); upstream names appear only as denied fixtures |
| Unknown/absent/malformed/contradictory cannot preserve stale Flagship | **FAIL** | Happy path proven (7 API + catalog tests), but F1: fail-open under durable clear failure (`subscription_api.rs:105`, `subscription_catalog.rs:284`); F2: HTTP 401/403 preserves stale cache untouched (`subscription_api.rs:89-97`) |
| Every accepted tier covered | PASS | `JcodeTier::ALL` loops in `live_tier_truth_accepts_only_product_owned_tiers`, `local_me_fixture_accepts_each_product_owned_tier...`, and `test_subscription_admission_and_display_agree_for_each_accepted_tier` |
| Freshness states covered | PARTIAL | Live/UnknownDenied/Default asserted; Cache never asserted (F5) |
| Admission and picker/display agree | PASS | `test_subscription_admission_and_display_agree_for_each_accepted_tier` (catalog_subscription.rs:261) checks `ensure_model_allowed_for_subscription` vs `filtered_display_models` per tier per curated model; both route through `effective_tier()` (`models.rs:161,174,191`) |
| Auth readiness and route identity proved | PASS (upgrade direction) | Fixture test asserts `CredentialPresent` → `RequestValid` transition via `record_jcode_validation` and `auth/mod.rs:181` smoke gate; route identity via `canonical_model_id("{id}@openai")` assertion. Downgrade after denial is implied by `auth/mod.rs` logic but not directly asserted for jcode (missing test) |
| Saved credentials / process env cannot silently grant entitlement | PASS | `cached_tier()` reads config file only (`subscription_catalog.rs:259-266` with rationale comment); tests `ambient_process_tier_env_does_not_grant_entitlement` (:517) and `test_subscription_filters_do_not_activate_from_saved_credentials_alone` (:347). Note `JCODE_SUBSCRIPTION_ACTIVE` remains a process-env flag, but it only enables restriction, never grants |
| API return contract cannot misrepresent denied truth | PARTIAL | `fetch_subscription_me` returns `Ok(me)` on denied truth; `SubscriptionMe.tier` remains the raw wire value (`Some("pro")`) and `parsed_tier()` returns `Some(Flagship)` for the contradictory case. `tier_truth()` exists as the honest API, but the one production caller mixes both (F3). The contract relies on caller discipline rather than encoding denial in the return value |

## Validation evidence assessment (static, could not re-run: read-only mandate)

- Claimed subscription API 7/7: static count at 4f104a609 confirms exactly 7 tests in `subscription_api.rs` tests module. Consistent.
- Claimed catalog 12/12: static count confirms exactly 12 tests in `subscription_catalog.rs` tests module. Consistent.
- Claimed provider admission/display 5/5: static count confirms exactly 5 subscription-related tests in `catalog_subscription.rs`. Consistent. Note the ledger (8a67d7c7b) instead reports "32 subscription-filtered + 4 provider subscription guard" via substring filters. Both are arithmetically consistent with the test inventory (`test_filtered_display_models_respects_curated_subscription_catalog` does not match the `test_subscription_` filter but does match `subscription_`).
- Infrastructure events preserved, not counted as passes: (a) the earlier provider filter that selected zero tests, corrected by the exact 5-test rerun, and (b) the ledger-recorded first focused command that used multiple cargo filters and failed before compiling. These are two distinct infra events. Neither is evidence of a pass.
- TUI check pass, R09 trusted greens with expected reds visible, TUI selfdev build pass, no reload: **accepted as reported, not independently re-verified** (execution prohibited). I did confirm the R09 ledger's no-`--update` policy exists (`docs/fork/recovery/seams/R09-quality-gates/ledger.md:27`) and that neither reviewed commit touches R09 baselines, ratchet scripts, or `scripts/check_*` (commit file lists contain only the 4 source files and the R02 ledger).

## Missing tests
1. Durable clear failure keeps denial fail-closed (added later in 760dcae11, absent at 4f104a609).
2. HTTP 401/403 `/v1/me` behavior with a stale Flagship cache (currently preserves access, F2).
3. Assertion that `freshness == Cache` when serving a persisted accepted tier (F5).
4. Unaccepted persisted file value (`JCODE_TIER=pro` in the env file) → `unknown_denied("cached tier is not accepted")` branch (F5).
5. Auth readiness downgrade (RequestValid → CredentialPresent) after a denied `/me` for the jcode provider specifically.
6. TUI label for contradictory truth (would have caught F3).

## Scope-creep assessment
Minimal and justified. The commit touches exactly the responsibility boundary: classification (`subscription_catalog.rs`), fetch/persist (`subscription_api.rs`), one 3-line TUI label change (`auth.rs`), and provider tests migrated from env-var seeding to `store_cached_tier` (required by the `cached_tier()` semantics change). `record_jcode_validation` adds a new validation-record side effect per fetch, but that is what proves the auth-readiness invariant, in-scope. No pricing, budget, model-floor, or routing-policy changes. The docs commit is append-only to the R02 ledger (35 added lines, no deletions), preserving prior sign-offs and the R09 debt section.

## What I did not check
- Did not execute any tests, builds, `jcode check`, R09 ratchets, or reloads (prohibited). All pass/fail counts above the static inventory are taken on faith from the task statement and ledger.
- Did not verify behavior against the real `api.jcode.sh` endpoint (no network).
- Did not audit concurrency of `jcode-subscription.env` writes across multiple simultaneous jcode processes (the atomic-marker fix in 760dcae11 is also per-process only).
- Did not review the out-of-scope amendment pair (760dcae11 / 5ba8eed30) beyond confirming it addresses F1; it deserves its own review (e.g. `LIVE_TIER_DENIED` is process-global and its clear-on-success ordering).
- Did not inspect the desktop crate (per session focus); grep shows the changed boundary is consumed only by jcode-base, jcode-tui, and jcode-provider-doctor (credentials-presence only).
- Did not validate the SHA-256 hashes cited in the pre-existing ledger sections.

**Bottom line:** the design is sound and the happy-path coverage is strong (all statically verifiable test counts match the claims), but the reviewed commit pair leaves two fail-open escapes (durable-clear failure, HTTP-authoritative denial) and one denied-truth display misrepresentation. FAIL for 4f104a609 + 8a67d7c7b as reviewed; the first escape is already remediated by the subsequent out-of-scope commit 760dcae11, the second and third remain open.
