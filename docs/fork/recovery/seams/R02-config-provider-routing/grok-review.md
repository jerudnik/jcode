# Independent Grok R02 review: configuration, auth readiness, entitlement, provider and routing authority

Worktree: `/Users/jrudnik/labs/jcode-seam-r02`  
HEAD reviewed: `8848f2d54f67f9a5a1de76bace9666c78036e116`  
Behavior baseline: `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`  
Upstream/master: `802f6909825809e882d9c2d575b7e478dce57d3b`  
Merge base: `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`  
Mode: read-only. No credentials, network, stash replay, repo modifications, or destructive actions.

Independence note: I did not open or read `/tmp/jcode-r02-opus-review.md`. A broad grep result did show that a recovery Opus review file also mentions `/v1/me`, but I did not open that file and did not use it as evidence. Findings below are from code, fixed refs, tests, and `RESPONSIBILITIES.md` scope only.

## Executive disposition

**Do not pilot R02 as-is. Recommend `compose`, with a pilot blocker.**

Retain fork-side fixes for config provenance, env-file credential refresh, route identity preservation, and configured sidecar routing. Adopt or reconcile upstream subscription tier catalog changes only after confirming product truth with a non-secret `/v1/me` fixture. Block the pilot until live tier truth cannot leave stale cached tier entitlement in place.

The strongest blocker is local tier admission truth. `fetch_subscription_me()` persists only recognized tiers. If `/v1/me` returns an unknown or newly added tier while the cache already says `flagship`, the live fetch succeeds but local admission can continue using stale Flagship entitlement because unknown tiers are not cleared or failed closed.

## Decisive checkpoints

### 1. R02 scope and current worktree provenance

Evidence:

- R02 owns layered provenance, credential references, account/provider/model selection, sidecars, route outcome, tier-gated admission, `/v1/me` tier truth, offline cached-tier fallback, and usage data affecting admission: `docs/fork/recovery/RESPONSIBILITIES.md:23`.
- Cross-seam invariant: selected account, provider, model, entitlement, and route must equal R12 recorded identity, with no stale ambient config substituting another route: `docs/fork/recovery/RESPONSIBILITIES.md:63-65`.
- Current HEAD is only Phase 1 docs on top of the behavior baseline. `git diff --name-status 7ff4fc6be..8848f2d54` showed no R02 code files changed. R02 code equals fork baseline for reviewed files.
- R02 file hash comparison showed fork/HEAD identical for `subscription_catalog.rs`, `subscription_api.rs`, `provider/models.rs`, `provider/selection.rs`, `config/config_file.rs`, `provider-env/src/lib.rs`, and `sidecar.rs`, while upstream differs on most of them.

Risk call: current worktree is a review/adjudication branch, not a new behavior branch. Disposition should compare fork baseline versus upstream, not treat HEAD as a new R02 implementation.

Test/static check: git hash and diff checks only.

### 2. Layered config provenance

Fork evidence:

- `Config::load()` loads file state, then applies env overrides: `crates/jcode-base/src/config/config_file.rs:35-40`.
- Fork merges durable `config.toml` with policy `config.nix.toml`, policy overriding durable values: `crates/jcode-base/src/config/config_file.rs:119-124`, `:401-421`.
- Fork records layer metadata and logs one `CONFIG_LAYER` summary: `crates/jcode-base/src/config/config_file.rs:147-168`.
- Fork marks durable paths unless policy pins them, then marks policy paths: `crates/jcode-base/src/config/config_file.rs:371-399`.
- Tests assert policy-over-durable behavior and provenance classifications: `crates/jcode-base/src/config_tests.rs:608-660`, `:662-712`.

Upstream/base contrast:

- At upstream/master `802f690982`, grep found no `build_layer_metadata`, `merge_policy_over_durable`, `provenance_for`, `config.nix.toml`, or `CONFIG_LAYER` in `config_file.rs` / `config_tests.rs`.

Risk call: fork is the authority for file-layer provenance. However, env overrides happen after file load and are not represented in the same provenance map. For pilot, either disable env overrides in the fixture or expose env-origin provenance for provider/model-affecting keys.

Test/static check: targeted warmed run passed `subscription_catalog::tests` and `subscription_api::tests`. The broader `config_tests::config_` filter matched zero tests under the dev wrapper, so config provenance remains code/test-read evidence rather than newly executed evidence in this run.

Disposition: retain fork, add/env-provenance prerequisite before pilot if env overrides are allowed.

### 3. Credential references and auth readiness

Fork evidence:

- Jcode subscription credentials live in `JCODE_API_KEY`, optional `JCODE_API_BASE`, and `jcode-subscription.env`: `crates/jcode-base/src/subscription_catalog.rs:3-11`, `:204-210`.
- Runtime maps subscription credentials into OpenRouter-compatible env vars and disables provider fallback knobs: `crates/jcode-base/src/subscription_catalog.rs:232-246`.
- Auth status describes Jcode as API key plus router base or router-base pending: `crates/jcode-base/src/auth/mod.rs:444-460`.
- Auth assessment for Jcode is a presence check over process env and `~/.config/jcode/jcode-subscription.env`: `crates/jcode-base/src/auth/mod.rs:606-621`.
- Login activation binds both `JCODE_API_KEY` and `JCODE_API_BASE` to `jcode-subscription.env`: `crates/jcode-base/src/auth/lifecycle.rs:797-817`.
- Env loader prefers process env, but fork adds config-file-only loading for explicit auth-change flows to avoid stale process env winning after `/login`: `crates/jcode-provider-env/src/lib.rs:212-245`; save also updates process env: `:277-299`.

Risk call: readiness is mostly presence-based. A configured but invalid key is not rejected until `/v1/me` or route use. That is acceptable for normal UI, but not enough for a pilot prerequisite. Pilot needs a fixture-backed `/v1/me` success, not just `JCODE_API_KEY` presence.

Test/static check: `subscription_api::tests::*` passed in warmed run. No live auth or network was used.

Disposition: retain fork credential source and stale-env refresh behavior. Pilot prerequisite: non-secret fixture must show credential source, router base, `/v1/me` status, and no ambient env override surprises.

### 4. Account/provider/model selection and route outcome

Fork evidence:

- Startup computes provider availability from actual credential probes, auto-selects, honors forced provider even if unconfigured with a warning, and honors configured preferred provider only if configured: `crates/jcode-base/src/provider/startup.rs:67-110`, `:209-280`.
- Startup then applies configured default model through `set_config_default_model`: `crates/jcode-base/src/provider/startup.rs:301-317`.
- User route selection persists provider runtime key, route API method, resolved model, provider runtime state, and logs an env snapshot: `crates/jcode-app-core/src/agent/provider.rs:71-87`.
- Session restore reconstructs model requests from `model`, `provider_key`, and `route_api_method`: `crates/jcode-app-core/src/agent.rs:461-483`; route-specific reconstruction preserves dual-auth and compatible-profile route prefixes: `crates/jcode-base/src/provider/selection.rs:394-430`.
- Model switch retries once after preserving current provider auth refresh: `crates/jcode-base/src/provider/mod.rs:286-310`.

Risk call: fork has strong route identity preservation for session restore and route picker paths. Forced-provider behavior deliberately allows unconfigured requests to fail later. Pilot should not use forced unconfigured provider state.

Test/static check: broad warmed run with `provider::tests::auth_refresh` completed exit 0, but output preview did not enumerate matching tests. Code-level evidence is primary here.

Disposition: retain fork route identity behavior. Route observables for pilot must include selected provider name, `session.provider_key`, `session.route_api_method`, `provider.model()`, active resolved credential, and R12 request route identity.

### 5. Sidecars and co-writer dependency

Fork evidence:

- Sidecar configured `agents.memory_model` routes prefixed and profile specs through `active_provider_fork_with_model_spec()` using the original spec, preserving route/profile/credential mode: `crates/jcode-base/src/sidecar.rs:417-479`.
- Fork logs explicit diagnostics and falls back to auto-selection on fork failure or no live provider: `crates/jcode-base/src/sidecar.rs:480-506`.
- Auto sidecar selection prefers Codex, then Claude, then an active provider fork, and exposes availability without pretending the Claude placeholder is usable: `crates/jcode-base/src/sidecar.rs:510-571`.
- Upstream has active provider fallback, but configured override only accepts OpenAI/Claude-ish direct routing and warns on unsupported overrides: upstream `802f690982:crates/jcode-base/src/sidecar.rs:90-155`.

Preserved hot-path stashes:

- `stash@{2}` is explicitly `WIP fix-config-hotpath-spam ... config warn-once + sidecar log dedup` and touches `config_file.rs`, `config_tests.rs`, `account_failover.rs`, and `sidecar.rs`.
- `stash@{0}` is `account_failover hot path`; `stash@{1}` is TUI/config caller cleanup; `stash@{3}` is unrelated docs.
- I treated these as preserved evidence only. They are not applied, not open work in the fixed HEAD, and not part of the reviewed behavior.

Risk call: sidecar route correctness is fork-superior, but warning/log dedup hot-path work is preserved, not integrated. For pilot, avoid sidecar/memory unless explicitly included, or require dedup and configured-route diagnostics to be checked.

Test/static check: no sidecar tests rerun in this review. Evidence is code and fixed-ref comparison.

Disposition: retain fork sidecar route preservation. Do not broaden Phase 3 pilot to memory sidecar unless this R02 dependency is explicitly observed.

### 6. Subscription-tier model admission

Fork evidence:

- Fork tier enum has only `Plus` and `Flagship`: `crates/jcode-base/src/subscription_catalog.rs:14-21`.
- Fork curated models gate Opus 4.8 and GPT-5.5 at Plus, Fable 5 and GPT-5.6 Sol at Flagship: `crates/jcode-base/src/subscription_catalog.rs:87-124`.
- Admission checks only run when subscription runtime mode is active. Non-runtime mode lets models through: `crates/jcode-base/src/provider/models.rs:156-181`.
- Runtime mode rejects non-curated models and rejects models above current tier: `crates/jcode-base/src/provider/models.rs:183-203`.
- `JcodeProvider::set_model()` enforces subscription admission before delegating to the inner provider and canonicalizes the selected model: `crates/jcode-base/src/provider/jcode.rs:95-104`; filtered display/routes apply the same tier filter: `:111-136`.

Upstream contrast:

- Upstream adds `Pro`, `Max`, and `Ultra`, with expanded pricing/budget and parse support: upstream `802f690982:crates/jcode-base/src/subscription_catalog.rs:14-89`.
- Upstream changes GPT-5.6 Sol minimum tier to Plus while keeping Claude Fable 5 at Flagship: upstream `802f690982:crates/jcode-base/src/subscription_catalog.rs:139-147`, tests at `:367-383`.

Risk call: neither side is automatically authoritative. Fork may be stale because it cannot parse newer `/v1/me` tiers. Upstream may be product-authoritative on tier names and Sol entitlement, but lowering Sol to Plus is a billing decision that needs confirmation. Since R02 owns entitlement, this must be resolved before pilot.

Test/static check: warmed run passed all 9 `subscription_catalog::tests`, including tier ordering, aliases, default model, cached fallback, and runtime flag.

Disposition: compose. Keep fork runtime enforcement shape, adopt/reconcile upstream tier catalog only after confirming paid tier policy and adding fixture tests for every `/v1/me` tier value.

### 7. `/v1/me` tier truth and offline cached-tier fallback

Fork/upstream shared evidence:

- `/v1/me` is documented as source of truth and last-known tier is cached for offline gating: `crates/jcode-base/src/subscription_api.rs:1-6`; fork catalog repeats this at `crates/jcode-base/src/subscription_catalog.rs:163-169`.
- `fetch_subscription_me()` fetches with `JCODE_API_KEY`, parses JSON, and stores tier only when `me.parsed_tier()` returns `Some`: `crates/jcode-base/src/subscription_api.rs:53-87`.
- Unknown/absent cached tier defaults to Plus: `crates/jcode-base/src/subscription_catalog.rs:163-177`.
- Test explicitly accepts unknown live tier as `None`: `crates/jcode-base/src/subscription_api.rs:118-131`.
- Test verifies no cached tier means Plus, and stored Flagship enables Flagship-only models: `crates/jcode-base/src/subscription_catalog.rs:335-357`.

Blocking risk:

If cache contains `flagship` and live `/v1/me` returns `pro`, `max`, `ultra`, or any unknown tier in fork, `fetch_subscription_me()` returns success but does not clear/downgrade the cached tier. Local admission therefore can keep granting Flagship-only models. Upstream reduces this risk for `pro/max/ultra` by parsing them, but still has the same unknown-tier non-clear behavior.

Test/static check: warmed run passed `subscription_api::tests` and `subscription_catalog::tests`. No test covers stale cached Flagship plus unknown live tier.

Disposition: pilot blocker. Fix by either failing closed or clearing cached tier on unknown live tier, and add deterministic test: cached Flagship + unknown `/v1/me` must not admit Flagship-only models after refresh.

### 8. Usage data affecting admission

Evidence:

- `/v1/me` usage shape has `used_usd`, `budget_usd`, and optional `resets_at`: `crates/jcode-base/src/subscription_api.rs:16-33`.
- UI displays usage after background `/v1/me` fetch: `crates/jcode-tui/src/tui/app/auth.rs:149-190`.
- Grep for `used_usd` and `budget_usd` found only `subscription_api.rs`, `subscription_catalog.rs` budget constants/tests, and TUI display code. I found no local admission gate using usage values.
- Local admission uses only curated model membership and effective tier: `crates/jcode-base/src/provider/models.rs:179-203`.

Risk call: usage over-budget is not a local R02 admission input today. That may be acceptable if the server router is authoritative and returns an error for over-budget requests, but the pilot cannot claim local usage-based admission. It can only claim usage display and server-side route denial, if a fixture covers it.

Test/static check: static grep only, plus `subscription_api` parse tests for usage fields.

Disposition: document as server-authoritative usage admission, or add local over-budget policy. Pilot prerequisite should include one fixture where usage is surfaced and one route denial observable for budget-exhausted accounts if that behavior is expected.

## Recommended pilot prerequisites and observables

Before R02 participates in the bounded pilot:

1. Confirm subscription tier catalog truth: tier set, pricing/budget labels, and each curated model min tier. Resolve fork versus upstream `Pro/Max/Ultra` and `gpt-5.6-sol` disagreement.
2. Add fixture coverage for all accepted `/v1/me.tier` values and for unknown tier behavior. Unknown live tier must not preserve stale higher cached entitlement.
3. Require fixture-backed `/v1/me` success before declaring auth readiness. Presence check alone is insufficient.
4. Freeze or record provider-affecting env overrides. Config provenance currently covers file layers, not post-load env overrides.
5. Route identity observables must include: configured provider source, selected account label/source, `provider.name()`, `provider.model()`, `session.provider_key`, `session.route_api_method`, active resolved credential, and the R12 request/result route identity.
6. If sidecars/memory are exercised, observe configured `agents.memory_model` route and sidecar fallback diagnostics. Otherwise explicitly keep sidecars out of the pilot.
7. Usage observables must be classified. If usage affects admission server-side only, record the router error path. If local usage gating is required, implement it before pilot.
8. Run no-secret tests: `subscription_catalog::tests`, `subscription_api::tests`, config provenance tests, route reconstruction/auth-refresh tests, and one stale-tier regression.

## Validation performed

- Git revision and file hash comparisons across base, fork, HEAD, and upstream for R02 files.
- Read-only `git diff --name-status` and symbol greps at fixed refs.
- Read-only `git stash list` / `git stash show --stat` to distinguish preserved hot-path stashes from fixed HEAD behavior.
- No-secret tests:
  - Initial exact multi-filter command failed due Cargo single-filter syntax, no code failure.
  - Exact rerun passed `subscription_catalog::tests::tier_gating_orders_plus_below_flagship`, then the dev wrapper failed on zero matches in other test binaries.
  - Warmed group run completed with exit 0. It explicitly passed all 9 `subscription_catalog::tests` and all 3 `subscription_api::tests`. Some later broad filters matched zero tests in the preview, so I do not count them as executed evidence.

## Confidence and gaps

Confidence: **medium-high** for config/auth/route/sidecar/tier-cache findings, **medium** for provider startup route behavior, **medium-low** for usage admission because server/router behavior was not exercised.

Gaps:

- No network or real `/v1/me` call by design.
- No credentials or live router route execution by design.
- No stash replay by design.
- No R12 request/result evidence inspected except for declared route identity dependencies.
- No full `jcode-base` test suite. Only targeted no-secret groups were attempted.
- The dev wrapper made some broad test filters ambiguous. I counted only visibly matched/passed tests.
