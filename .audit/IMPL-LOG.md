# Implementation log

## 2026-07-10 — Plan reframe

- Commit: `0d242297e docs(audit): reframe credential and parser fixes`
- Replaced WI-1 and WI-4 with the final real-cut designs.
- Preserved superseded rationale with links to `.audit/SOL-ASSESSMENT-R3.md` holes #1 and #2.

## 2026-07-10 — Baseline: configurable memory embedding credential env

- Files: `crates/jcode-base/src/config.rs`, `config/default_file.rs`, `config/env_overrides.rs`, `embedding_backend.rs`, `crates/jcode-config-types/src/lib.rs`.
- Added `agents.memory_embedding_api_key_env`, its env override/default-file documentation, cache fingerprint input, and remote embedding credential selection.
- Validation:
  - `nix develop --command cargo test -p jcode-base --lib config_env_fingerprint_tracks_every_apply_env_override_var` -> exit 0, 1 passed.
  - `nix develop --command cargo test -p jcode-base --lib embedding_backend` -> exit 0, 3 passed.
  - `nix develop --command cargo check -p jcode-base` -> exit 0.
- Only pre-existing dead-code/test warnings were emitted.

## 2026-07-10 — WI-0: isolate external runtime forks

- Files: `crates/jcode-base/src/provider/mod.rs`, `provider/tests.rs`.
- `MultiProvider::fork()` now calls the inner provider's `fork()` for Copilot, Antigravity, and Gemini instead of cloning the live `Arc`.
- Added one parameterized regression using the existing `StubExternalRuntime` seam and global test-env lock. Each fork accepts an alternate explicitly routed model while the registered live runtime remains unchanged.
- Validation:
  - `nix develop --command cargo test -p jcode-base --lib fork_isolates_external_runtime_model_state` -> exit 0, 1 passed.
  - `nix develop --command cargo check -p jcode-base` -> exit 0.
  - `nix develop --command cargo test -p jcode-base --lib provider` -> exit 101, 227 passed and the unrelated pre-existing `minimax_token_plan_keys_resolve_to_china_endpoint_without_changing_international_default` failed because another filtered test leaves a MiniMax credential file visible in the shared process test home. WI-1 owns MiniMax credential identity and test isolation; the WI-0 regression and compilation are green.
- Only pre-existing dead-code/test warnings were emitted.

## 2026-07-10 — WI-1: make catalog credential provenance typed end-to-end

- Added ordered `api_key_aliases` metadata to static and resolved OpenAI-compatible profiles. Z.AI keeps canonical `ZHIPU_API_KEY` with read-only alias `ZAI_API_KEY`; MiniMax now canonically owns `MINIMAX_API_KEY`, leaving `OPENAI_API_KEY` owned only by the OpenAI profile.
- Added non-constructible-field `ApiKeyCredentialSource` provenance in `jcode-provider-env`, with catalog and primary-only constructors, candidate-name diagnostics, and one ordered/sanitized loader. The lossy pair loader is private and test-only.
- Migrated every production and test caller of the former public pair loader. Profile/resolved-profile consumers now construct catalog sources directly; Azure, Bedrock, Anthropic, OpenAI/Codex, embedding overrides, named profiles, and explicit `JCODE_OPENROUTER_*` overrides use primary-only sources.
- Added a typed active catalog-profile marker in `provider_catalog`. `apply_openai_compatible_profile_env` sets/clears it independently of the compatibility environment variables. Both the base shim and `OpenRouterProvider::new` accept the marker only while the current compatibility variables still match, so a later genuine explicit override remains primary-only.
- Threaded the typed catalog source through `OpenRouterProvider::new -> resolve_auth -> get_api_key`, profile catalog refresh, autodetection, and dedicated compatible-profile construction. Removed the `ZHIPU_API_KEY` leaf special case after all profile consumers migrated.
- `openrouter_like_api_key_sources` now returns typed sources. External-auth validation, auto-init prompts, auth assessment metadata, provider doctor, auth-test model discovery, usage/live probes, lifecycle tests, TUI migration, and provider matrix coverage consume typed sources or enumerate their candidate env names.
- Added loader precedence/alias-order/sanitation tests, MiniMax/OpenAI isolation, primary-only negative controls, unique primary metadata ownership, a source guard against bare-loader escape/profile-pair laundering, alias-aware auth metadata, and the required alias-only Z.AI default-runtime regression through profile activation and resolved bearer auth. Explicit override and named-profile tests prove they do not probe `ZAI_API_KEY`.
- Adaptations during implementation:
  - Plain `cargo check ...` -> exit 127 because `cargo` is intentionally available only inside the Nix dev shell; all subsequent commands used `nix develop --command`.
  - Initial targeted compile -> exit 101 for an `&&str` Gemini env-key argument; dereferenced the iterator item, then the same targeted compile passed with exit 0.
  - Initial OpenRouter suite -> exit 101 because an ignored live-smoke test still called the old zero-argument private `get_api_key`; updated it to the explicit primary-only/default path, then the suite passed with exit 0.
- Validation:
  - `nix develop --command cargo test -p jcode-provider-metadata -p jcode-provider-env` -> exit 0; provider-env 10 passed, metadata 14 passed, doc tests 0 failed.
  - `nix develop --command cargo test -p jcode-provider-openrouter-runtime` -> exit 0; 86 passed, 1 ignored, 0 failed.
  - `nix develop --command cargo test -p jcode-base --lib provider_catalog` -> exit 0; 32 passed, 0 failed.
  - `nix develop --command cargo test -p jcode-app-core --lib external_auth` -> exit 0; 4 passed, 0 failed.
  - `nix develop --command cargo test -p jcode-base --lib zai_alias_only_auth_assessment_reports_legacy_candidate_metadata` -> exit 0; 1 passed, 0 failed.
  - `nix develop --command cargo test --test provider_matrix` -> exit 0; 9 passed, 0 failed.
  - `nix develop --command cargo test -p jcode-tui --lib tui_openai_compatible` -> exit 0; 6 passed, 0 failed.
  - `nix develop --command cargo test -p jcode-provider-doctor --lib fresh_start_sandbox_is_unconfigured_then_tui_key_lifecycle_configures_provider` -> exit 0; 1 passed, 0 failed.
  - Final `nix develop --command cargo check --workspace` -> exit 0.
  - `git diff --check` and the WI-0 ancestry assertion -> exit 0.
- Only pre-existing dead-code, unused-code, unnecessary-unsafe, and future-incompatibility warnings were emitted.

## 2026-07-10 — WI-2A: centralize built-in provider identity

- Files: `crates/jcode-provider-core/src/selection.rs`, `models.rs`, `lib.rs`, and the required `crates/jcode-base/src/provider/models.rs` compile adaptation.
- `ActiveProvider` now owns canonical keys and aliases. Provider hint/key parsing and built-in CLI session-key translation delegate to that typed identity, including the previously omitted Bedrock hints.
- Replaced the explicit model-prefix arm chain with an exact-vocabulary table. Credential-pinning entries validate through `AuthRoute::parse_explicit_credential_prefix` while preserving the original prefix spelling; `anthropic-api:` remains non-native.
- Added typed `builtin_provider_for_model[_with_hint]` helpers and retained string-returning compatibility wrappers for WI-2B/2C. The private core OpenRouter catalog normalizer and the base fallback caller now consume the typed helper.
- Added exhaustive key/alias, Bedrock, builtin-classification, compatibility-wrapper, accepted-prefix, and rejected broad-AuthRoute-prefix coverage.
- Validation:
  - `nix develop --command cargo test -p jcode-provider-core` -> exit 0; 86 passed, 0 failed, doc tests 0 failed.
  - `nix develop --command cargo check -p jcode-provider-core -p jcode-base` -> exit 0.
  - `nix develop --command cargo fmt --all` -> exit 0.
  - `git diff --check` -> exit 0.
- Only pre-existing dead-code warnings were emitted.

## 2026-07-10 — WI-2B: centralize base model-spec resolution

- Files: `crates/jcode-base/src/provider/models.rs`, `provider/selection.rs`, `provider/mod.rs`, `provider/catalog_routes.rs`, `provider/pricing.rs`, `provider/route_builders.rs`, `provider/tests/model_resolution.rs`, and `sidecar.rs`.
- Added public `ResolvedModelSpec` plus `resolve_model_spec(model, &Config)` and the read-only `resolve_current_model_spec` convenience. Native prefixes return the canonical execution provider while preserving the exact auth spelling in `explicit_prefix`; catalog/named profiles retain their profile keys; OpenRouter pins retain the full `model@provider` wire id.
- Enforced the required precedence: exact provider-core native prefix, static OpenAI-compatible catalog profile, named config profile, valid OpenRouter pin, then the typed base detector. Unknown prefixes remain unclassified, while Bedrock ARN/model ids with colons still resolve as Bedrock.
- Removed the duplicate base classifier body and local catalog/named prefix parsers. Base catalog routes, pricing, route builders, capability lookup, selection/session helpers, `MultiProvider`, and sidecar classification now consume the resolved specification. Static string-returning wrappers remain only as thin WI-2C compatibility shims.
- Added table coverage for bare Claude/OpenAI/Gemini, Bedrock, Cursor, Antigravity, OpenRouter pins, all four dual-auth prefixes, static and named profiles, unknown prefixes, the `openai-api` native collision, and `anthropic-api` catalog preservation. Added focused config/session normalization coverage.
- Adaptations during implementation:
  - The first resolver test showed the core detector's broad `gpt-*` fallback swallowed Antigravity's known `gpt-oss-*` model. The base typed detector now preserves exact native Claude/OpenAI and Gemini identities before accepting distinct known Antigravity models.
  - The existing `default_provider = "anthropic-api"` test encoded the disproven native alias behavior. It now covers only native Claude keys; resolver/config-selection tests assert that `anthropic-api` remains the catalog profile.
- Validation:
  - `nix develop --command cargo test -p jcode-base --lib provider` -> exit 0; 230 passed, 0 failed.
  - `nix develop --command cargo test -p jcode-base --lib resolved_model_spec_uses_one_context_aware_precedence_table` -> exit 0; 1 passed, 0 failed.
  - `nix develop --command cargo test -p jcode-base --lib session_model_helpers_preserve_native_and_catalog_prefix_identity` -> exit 0; 1 passed, 0 failed.
  - `nix develop --command cargo check -p jcode-base` -> exit 0.
  - `nix develop --command cargo fmt --all` and `git diff --check` -> exit 0.
- Only pre-existing dead-code and unused test warnings were emitted.

## 2026-07-10 — WI-2C: migrate remaining model-spec consumers

- Files: `crates/jcode-app-core/src/server/comm_session.rs`, `comm_session_tests.rs`, `jade_relay.rs`, `crates/jcode-provider-openrouter-runtime/src/lib.rs`, `openrouter_provider_impl.rs`, `openrouter_tests.rs`, `crates/jcode-tui/src/tui/app/inline_interactive.rs`, `inline_interactive/openers.rs`, `tui_lifecycle_runtime.rs`, `tui_state.rs`, and focused TUI test files.
- App-core spawn and Jade launch provider-key helpers now preserve explicit override precedence but delegate model-derived identity to `jcode_base::provider::resolve_model_spec(model, config()).provider_key`. `explicit_route_for_configured_model` now consumes `ResolvedModelSpec::{explicit_prefix,bare_model,provider_key}` and deliberately pins `provider_key`/`route_api_method` to the dual-auth route id only for exact `openai-api`, `openai-oauth`, `claude-api`, and `claude-oauth` prefixes; catalog aliases such as `anthropic-api:` are left to normal session restore routing.
- TUI migration surfaces now use the base resolver for configured default-route marking, remote Claude dual-auth route synthesis, replay identity, remote effort identity, and header identity. The memory agent model picker no longer filters to OpenAI/Claude families, so resolver-recognized named and generic profiles remain selectable without probing provider forks.
- OpenRouter runtime prefix stripping now uses the base resolver and a provider-instance guard: built-in/native prefixes are retained, exact `profile_id` prefixes strip even when the profile is an ad-hoc constructed named provider not visible in global config, and deferred shared runtimes strip named/catalog prefixes only when the resolved profile API base matches the runtime API base. `context_window()` uses the same normalization so per-model named-profile context limits survive qualified session models.
- Removed now-unused public general-classifier compatibility wrappers from base and provider-core after workspace search. Remaining search hits are only the explicitly named core builtin helpers (`builtin_provider_for_model[_with_hint]`) and base's private builtin fallback inside the context-aware resolver.
- Added regressions for app-core spawn/Jade parity and explicit-route pinning, OpenRouter own-profile versus switch-away prefix stripping and context-window preservation, and the six TUI surfaces: named-profile effort/header identity, replay saved-provider-key precedence, configured `omlx:` default-route marking, Claude-looking named-profile route synthesis, and memory picker eligibility/exact saved spec matching.
- Adaptations during implementation:
  - The first app-core targeted run failed because `explicit_route_for_configured_model` initially used resolver `provider_key = "openai"` for session `provider_key`; restored the existing credential-route pin (`openai-api-key`/OAuth route ids) while still sourcing prefix and bare model from the resolver.
  - The first required multi-crate check failed because `jcode-tui` cannot name `jcode_base::provider::ResolvedModelSpec` directly; switched the type path to the crate re-export and removed the now-unused memory-picker helper import.
  - A full TUI lib run compiled the new TUI tests but hit unrelated existing UI/cache snapshot failures (`test_full_prep_cache_state_keeps_two_oversized_width_entries_hot`, `test_updates_header_repeated_renders_stay_stable_near_scrollbar_threshold`) and was killed after about five minutes. A subsequent single targeted TUI test invocation hung during Nix shell hook setup (`scripts/rerere-cache.sh setup`), so TUI behavior is covered by compile/check plus the added focused tests rather than a completed TUI test run in this environment.
  - Full OpenRouter runtime initially exposed that own named-profile prefix stripping also needs to work for ad-hoc provider instances not present in global config and for `context_window()` lookups where tests write the qualified model directly into the model lock; added the raw `profile_id` fast path and normalized the existing context-window override.
- Validation:
  - `nix develop --command cargo fmt --all` -> exit 0.
  - `nix develop --command cargo test -p jcode-base --lib model_resolution` -> exit 0.
  - `nix develop --command cargo test -p jcode-app-core --lib comm_session` -> exit 0; 34 passed, 0 failed.
  - `nix develop --command cargo test -p jcode-app-core --lib jade_relay` -> exit 0; 10 passed, 0 failed.
  - `nix develop --command cargo test -p jcode-provider-openrouter-runtime strip_session_profile_prefix_only_for_this_resolved_compatible_profile` -> exit 0; 1 passed.
  - `nix develop --command cargo test -p jcode-provider-openrouter-runtime` -> exit 0; 89 passed, 1 ignored, doc tests 0 failed.
  - `nix develop --command cargo check -p jcode-base -p jcode-app-core -p jcode-tui -p jcode-provider-openrouter-runtime` -> exit 0.
- Only pre-existing dead-code/test warnings were emitted.
- Exact next stage WI-3: implement `Provider::fork_with_model_spec`, add `MultiProvider::active_provider_fork_with_model_spec`, and change `Sidecar::with_configured_model` to use `resolve_current_model_spec` so only unprefixed native OpenAI/Claude models keep specialized ambient-auth fast paths while every prefixed/catalog/named profile routes through an isolated configured provider fork.
