# R02 full-seam review (independent Opus)

- Reviewer: independent `verify`/full-seam reviewer, read-only. No repo files, refs, worktrees, or stashes changed. Did not read or search for any Grok R02 artifact.
- Worktree: `/Users/jrudnik/labs/jcode-seam-r02` at HEAD `8848f2d54f67f9a5a1de76bace9666c78036e116` (verified clean).
- Refs verified: `8848f2d54` is `7ff4fc6be` (behavioral baseline) plus one docs-only commit (`docs: Record Phase 1 responsibility map`); code is behaviorally identical to `7ff4fc6be`. upstream/master `802f6909825809e882d9c2d575b7e478dce57d3b`, merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` (confirmed `git merge-base HEAD upstream/master == 631935dd1`).
- Scope: R02 = layered config provenance, credential references and auth readiness, account/provider/model selection, sidecars, route outcome, subscription-tier model admission, `/v1/me` tier truth, offline cached-tier fallback, and usage data affecting admission. Excludes R12 turn execution, R03A wire verdicts, R06 persistence, R13 compaction except co-writer dependencies.
- Budget: 8 decisive checkpoints, plus one executed test run to close the primary gap.
- Confidence: high on divergence measurements and the tier-ladder reconcile finding; medium-high on final disposition (fork subscription/tier tests were built and executed, see Validation).

## Validation performed (executed)

Built and ran the fork tree's subscription/tier tests via `nix develop --command cargo test -p jcode-base --lib subscription_catalog` (7m05s cold build, then 0.04s run). Result: **10 passed, 0 failed**, including:

- `effective_tier_defaults_to_plus_when_unknown` (ok) — confirms the offline/unknown fallback returns `Plus`.
- `tier_gating_orders_plus_below_flagship`, `flagship_models_are_gated_above_plus` (ok) — confirm the fork's 2-tier admission ordering.
- `tier_parse_round_trips`, `tier_pricing_matches_launched_plans`, `default_model_is_opus`, `curated_model_*`, `runtime_mode_flag_tracks_subscription_activation`, `provider::tests::test_filtered_display_models_respects_curated_subscription_catalog` (ok).

Code confirmation of the under-entitlement path (`subscription_catalog.rs:53-58`): `JcodeTier::parse` matches only `"plus"` and `"flagship"`, returning `None` for everything else; combined with `effective_tier() = cached_tier().unwrap_or(JcodeTier::Plus)` (line 168-169), a `/v1/me` tier of `pro/max/ultra` parses to `None` and silently resolves to `Plus`. The passing `effective_tier_defaults_to_plus_when_unknown` test is exactly this fallback. This upgrades the under-entitlement finding from hypothesis to confirmed behavior. The fork has **no** test asserting a `pro/max/ultra` wire tier, so the downgrade is untested and silent.

## Bottom line

R02 is correctly scoped as a full seam. The dominant, decision-forcing divergence is **subscription-tier model admission**: fork and upstream both renamed the billing endpoint identically, but only **upstream expanded the tier ladder from 2 tiers (Plus, Flagship) to 5 (Plus, Pro, Max, Ultra, Flagship)** with a per-model `min_tier` floor and an order-invariant gating test. The fork tree still ships the 2-tier ladder. This is a genuine fork/upstream reconcile point, not a fork-only add, and it directly gates which models an account may select. My recommended disposition for the tier-gating sub-scope is **adopt-upstream** (medium confidence, pending the two prerequisites below). The rest of R02 (config provenance/layering, provider selection/failover, route building) is strongly **fork-dominant and coherent**; recommended disposition **retain-fork** for those sub-scopes. I do not recommend collapsing R02 into sub-seams; a single ledger with named sub-scopes is right, matching the coordinator's adjudication.

## Evidence (symbol-level, reproducible)

Two-sided numstat, `base=631935dd1`, `fork=7ff4fc6be`, `up=802f690982`:

| File | fork | upstream | Read |
|---|---|---|---|
| `crates/jcode-base/src/subscription_catalog.rs` | 2+/1- | **87+/19-** | **upstream-heavy reconcile point** |
| `crates/jcode-base/src/subscription_api.rs` | 2+/2- | 3+/3- | small two-sided (endpoint text) |
| `crates/jcode-base/src/config.rs` | **227+/21-** | 10+/17- | fork-dominant (provenance/layering) |
| `crates/jcode-config-types/src/lib.rs` | **544+/22-** | 73+/25- | fork-dominant (profiles, model_picker) |
| `crates/jcode-base/src/provider/mod.rs` | **192+/95-** | 84+/13- | fork-dominant (selection/credential mode) |
| `crates/jcode-base/src/provider/selection.rs` | 108+/59- | 0 | fork-only |
| `crates/jcode-base/src/provider/catalog_routes.rs` | 96+/14- | 75+/4- | **two-sided collision on OpenRouter fallback** |
| `crates/jcode-base/src/provider/models_catalog.rs` | 51+/5- | 51+/5- | two-sided, identical counts (likely same commit) |
| `crates/jcode-base/src/sidecar.rs` | **984+/48-** | 37+/17- | fork-dominant, present at base both sides |
| `crates/jcode-base/src/provider/account_failover.rs` | 1+/1- | 0 | fork-only, near-static |

Commands: `git diff --numstat $base $fork -- <path>` and `... $up ...`.

### 1. Subscription-tier admission (the decisive reconcile point)

- Current fork tree (`crates/jcode-base/src/subscription_catalog.rs:14-21`): `enum JcodeTier { Plus, Flagship }`, `ALL = [Plus, Flagship]`.
- Upstream (`git diff $base $up`): `enum JcodeTier { Plus, Pro, Max, Ultra, Flagship }`, `ALL = [Plus, Pro, Max, Ultra, Flagship]`, plus `retail_price_usd`/`usable_budget_usd`/`display_name` arms for the three new tiers.
- Both sides moved the default `min_tier` from `Flagship` to `Plus` for the general catalog and gate a named flagship model (`claude-fable-5`) at `Flagship` (upstream test `tier_gating_follows_catalog_order`, fork enforcement `provider/models.rs:191-199` `if !tier.allows(curated.min_tier) { bail! }`).
- Enforcement lives in **both** trees at `provider/models.rs` (`git show $up:.../models.rs | grep -c "tier.allows\|min_tier"` = 2; fork tree same). So the admission *mechanism* is shared; only the *ladder depth* diverges.
- `/v1/me` tier truth and offline cache: `subscription_api.rs` docstring names `GET /v1/me` as source of truth; `subscription_catalog.rs:168-169` `effective_tier() = cached_tier().unwrap_or(JcodeTier::Plus)` implements the offline fallback ("unknown/absent tier behaves like Plus"). This matches the gap review's claim exactly.

**Interpretation:** upstream's 5-tier ladder is the more complete pricing/entitlement model and its gating test asserts a real ordering invariant (`account_index >= required_index`). The fork's 2-tier ladder is a strict subset. Adopting upstream's ladder is low-risk for admission logic because the enforcement site is already shared, but it is **not** a mechanical no-op: it changes user-visible pricing/budget numbers and tier names, so it needs product confirmation, and any account whose `/v1/me` returns `pro/max/ultra` currently parses to `None` on the fork (`JcodeTier::parse` returns `None`), falling back to `Plus` and **silently under-entitling** a paying account. That is a latent correctness bug in the fork today, which strengthens the adopt-upstream case.

### 2. Endpoint rename already converged

Both fork (`git diff $base $fork` on `subscription_catalog.rs`, the only 2+/1- change) and upstream renamed `DEFAULT_JCODE_API_BASE` from `https://api.solosystems.dev/v1` to `https://api.jcode.sh/v1` and added `JCODE_PRICING_URL`. This is a convergent change; no conflict. Evidence that the fork's sole edit to this file was the endpoint, i.e. the fork deliberately did **not** take the tier expansion.

### 3. Config provenance and layering (fork-dominant, retain-fork)

`config.rs` fork additions introduce a first-class provenance system absent upstream (`git show $up:.../config.rs | grep -c ConfigProvenance` = 0): `enum ConfigProvenance`, `struct ConfigLayerMetadata { provenance: BTreeMap<String, ConfigProvenance>, pinned_paths }`, `Config::provenance()`, `provenance_for(key_path)`. Runtime-only, never serialized. This directly satisfies R02's "layered provenance is explainable" invariant and has no upstream competitor. Recommend retain-fork.

### 4. Provider selection / credential mode (fork-dominant, retain-fork)

`provider/mod.rs` fork additions: `active_provider_fork_with_model_spec` (documented as producing an independent instance so "the live agent's own selection is never mutated"), `credential_mode()`/`set_credential_mode()` with explicit "Provider does not support OAuth/API-key credential selection" rejection, `resolve_model_spec`/`resolve_current_model_spec` with explicit-prefix and profile resolution. `selection.rs` is fork-only (108+/59-). These implement R02's "explicit provider/model choice beats stale ambient state" and "credentials are references, not values" invariants. Retain-fork.

### 5. catalog_routes.rs is a real two-sided collision (compose, not pick-a-side)

Both sides edit the **same** OpenRouter fallback-route logic: `openrouter::standard_catalog_lists_model(&or_model) != Some(false)` appears in added hunks on both fork and upstream, and upstream adds provider-specific skip arms (`(Some("claude"), None)`, `(Some("openai"), None)`) for models "OpenRouter definitively does not offer." The fork adds `resolve_current_model_spec` provider-key routing and `load_api_key(ApiKeyCredentialSource::primary_only(...))`. These touch overlapping control flow. This sub-scope needs **compose** (merge both intents) and is the one place inside R02 where a naive adopt-either could silently drop the other side's fallback-suppression. Flag for careful seam work.

### 6. Sidecar (fork-dominant, likely retain-fork, low admission relevance)

`sidecar.rs` is present at base on both sides and fork-expanded to 984+/48- vs upstream 37+/17-. It is process/health machinery adjacent to R02 rather than admission logic. Low pilot relevance; retain-fork provisionally, but it is large enough that its own light sub-ledger is warranted; I did not open its body in depth (budget).

### 7. Preserved hot-path stashes are evidence, not open work

`git stash list` shows `stash@{0..2}` = the three `fix-config-hotpath-spam` WIP stashes (`account_failover hot path`, `Config::load->config() TUI callers`, `config warn-once + sidecar log dedup`) plus `stash@{3}` pre-sync WIP. The fork tree's `account_failover.rs` is near-static (1+/1- vs base) and still calls `crate::config::config()` at line 21. Consistent with the coordinator's adjudication: the hot-path fixes are **preserved stashes, not open R02 work**; the live re-read behavior remains in-tree and is a *check inside the R02 ledger* (does `config()`/`Config::load` get re-invoked on the account-failover hot path), not a separate seam. I did not pop or replay any stash.

## Challenge to both authorities

- **Against upstream:** upstream's tier expansion ships new pricing/budget constants (`Pro=$20/$40`, `Max=$100/$225`, `Ultra=$200/$500`) that are product/business decisions, not pure code correctness. Adopting them blindly imports a pricing model the fork's operator may not have agreed to. Adopt-upstream must be gated on explicit product confirmation of those numbers, not merged as a mechanical reconcile.
- **Against fork:** the fork's 2-tier ladder plus `JcodeTier::parse` returning `None` for `pro/max/ultra` means a real paying account on those tiers is silently downgraded to Plus entitlement. The fork has no test covering a non-{Plus,Flagship} wire tier. This is a latent under-entitlement bug; the fork side is not "safely conservative," it is incomplete.
- **Shared risk:** admission enforcement is duplicated in `provider/models.rs` on both sides. Whichever ladder wins, there must be exactly one enforcement path; a second undiscovered enforcement site would let a gated model through. I confirmed the `models.rs` site on both trees but did not exhaustively grep every crate for a second `tier.allows`/`min_tier` consumer (see gaps).

## Recommended disposition (supported subset only)

- Tier-gating sub-scope: **adopt-upstream** (medium confidence), conditioned on (a) product sign-off of the new pricing/budget constants and (b) proof of a single enforcement path. This also fixes the fork's silent under-entitlement bug.
- Config provenance/layering, provider selection/credential-mode, account-failover: **retain-fork** (medium-high confidence).
- `catalog_routes.rs` OpenRouter fallback: **compose** (medium confidence); do not pick a side.
- Endpoint rename: already **converged**, no action.
- Sidecar: **retain-fork** provisionally, own light sub-ledger.
- I make **no** disposition on `config-types` profile/model_picker additions beyond noting they are fork-only UX surface with no upstream competitor, so retain-fork by default.

## Pilot prerequisites and route/identity observables for R02

Route/identity observables the pilot must be able to read without secrets:
1. Effective config provenance for the selected provider/model keys (`Config::provenance_for(key_path)`).
2. Resolved model spec and selected route (`resolve_current_model_spec`, `RouteSelection`), including provider label and api method.
3. Auth readiness as a boolean/reference, never the secret value (`credential_mode()`, `load_api_key(...primary_only)`).
4. Effective tier and its source (`effective_tier()`, cached vs `/v1/me`), and the admission verdict for the chosen model (`tier.allows(curated.min_tier)`).

Prerequisites before the R02 slice of the pilot:
1. A fixture `/v1/me` payload (no network, no real account) exercising at least one non-Plus tier to prove the offline-cache path and to expose the fork's `parse -> None -> Plus` downgrade.
2. A decision on the tier ladder (adopt-upstream vs retain-fork) recorded before the pilot asserts an admission outcome, since the two ladders give different verdicts for `pro/max/ultra`.
3. Confirmation of a single tier-enforcement path (one `tier.allows` consumer) so the pilot's admission observable is authoritative.
4. R13 co-writer acknowledgement: R02 model-switch and R13 compaction both null `provider_session_id`; the pilot must not switch models or cross the compaction threshold, or must assert the joint invalidation (per RESPONSIBILITIES invariant #3).

## Confidence and explicit gaps

- High: numstat divergence, the 2-tier-vs-5-tier ladder difference, endpoint convergence, provenance system being fork-only, hot-path stashes being preserved-not-open, and the under-entitlement fallback (now confirmed by code read plus the passing `effective_tier_defaults_to_plus_when_unknown` test).
- Medium-high: adopt-upstream recommendation (fork subscription/tier tests built and executed green; upstream ladder not executed in this worktree since it is a different ref).
- Not checked / gaps:
  - Upstream's 5-tier tests were read, not executed here (they live on `802f690982`, not this worktree). Fork tests were executed.
  - Did not open `sidecar.rs` (984 lines) body; classified by role only.
  - Did not exhaustively grep every crate for a second `tier.allows`/`min_tier` enforcement site beyond `provider/models.rs`.
  - Did not read the fork/upstream bodies of `config-types/src/lib.rs` line by line; classified additions by symbol names.
  - Did not read any Grok R02 artifact (per instruction) and did not perform stable patch-ID equivalence for R02 files.
  - `models_catalog.rs` shows identical 51+/5- on both sides (likely the same commit via curated sync); I did not confirm patch-ID equivalence, so I did not mark it converged.

## One-line summary

R02 is a valid full seam whose forcing question is the tier ladder: adopt upstream's 5-tier admission model (fixing a latent fork under-entitlement bug) while retaining the fork's superior config-provenance and selection machinery and composing the two-sided OpenRouter fallback logic. Confirm by executing the `jcode-base` subscription/provider tests before acting.
