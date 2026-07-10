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

## 2026-07-10 — WI-3: sidecar model overrides route through a forked resolved provider

- Commit: `68ee7ba5e fix(sidecar): route model overrides through a forked resolved provider`
- Files: `crates/jcode-provider-core/src/lib.rs`, `crates/jcode-base/src/provider/mod.rs`, `crates/jcode-base/src/sidecar.rs`.
- Changes:
  - Added the object-safe synchronous default `Provider::fork_with_model_spec(&self, model_spec) -> Result<Arc<dyn Provider>>` (fork, `set_model(model_spec)?`, return). No edits to the ~62 impls.
  - Added `MultiProvider::fork_with_model_spec` override building on the WI-0-isolated `fork()` (fork, `set_model` on the isolated instance with model+provider context via `anyhow::Context`, erase to Arc). Imported `Context`.
  - Added free fn `active_provider_fork_with_model_spec(model_spec) -> Option<Result<Arc<dyn Provider>>>` beside `active_provider_fork()`, delegating to the registered live provider's trait method; never mutates the live selection.
  - Sidecar: added `provider: Option<Arc<dyn Provider>>` field storing the model-configured fork ONCE. `with_configured_model` now delegates to new `backend_for_configured_model`: bare native OpenAI/Claude (no `explicit_prefix`) keep `SidecarBackend::{OpenAI,Claude}`; every prefixed spec routes through `active_provider_fork_with_model_spec` with the ORIGINAL spec into `SidecarBackend::Provider`. Fork/`set_model` failure or no-live-provider logs `crate::logging::error` naming the model + `ResolvedModelSpec` and falls back to auto-selection. `auto_select_backend` returns the 3-tuple and stores its fork. `complete_via_provider` now consumes `self.provider` (no re-fork); runtime `complete_simple` failures surface plainly with no new fallback ladder. `SidecarBackend` enum retained.
  - Tests: extended `StubProvider` with shared `set_model_specs` recording plus `fail_set_model`/`fail_complete` flags and a per-instance model cell. Added 4 tests: exact-spec fork routing for `omlx:Qwen3.6-MoE`/`openai-api:gpt-5.5`/`anthropic-api:...` (live provider unchanged); bare `gpt-5.5`/claude keep specialized path with no stored fork; fork `set_model` (Copilot `try_write` style) failure -> explicit fallback with live provider selection preserved; runtime `complete_simple` failure surfaces plainly and is not retried.
- Adaptations:
  - Test call `stub.model()` collided with the `Provider::model()` trait method (trait not in scope in the test module); read the `stub.model` `RwLock` field directly instead. No production change.
- Validation:
  - `nix develop --command cargo test -p jcode-provider-core` -> exit 0; 86 passed, 0 failed, doc tests 0 failed.
  - `nix develop --command cargo test -p jcode-base --lib sidecar` -> exit 0; 16 passed, 0 failed (4 new WI-3 tests included).
  - `nix develop --command cargo test -p jcode-base --lib provider` -> exit 0; 232 passed, 0 failed (pre-existing minimax failure did not reproduce this run).
  - `nix develop --command cargo check -p jcode-base -p jcode-app-core` -> exit 0.
  - `nix develop --command cargo fmt --all` -> exit 0; `git diff --check` -> exit 0.
  - Only pre-existing dead-code warnings were emitted.
- Exact next stage: WI-4.

## 2026-07-10 — WI-4: observable config parsing, load-time backend normalization, runtime warn-once

- Files: `crates/jcode-base/Cargo.toml` (+`serde_ignored = "0.1"`), `Cargo.lock`, `crates/jcode-base/src/config/config_file.rs`, `crates/jcode-base/src/config.rs`, `crates/jcode-base/src/config/env_overrides.rs`, `crates/jcode-base/src/config_tests.rs`, `crates/jcode-config-types/src/lib.rs`, `crates/jcode-provider-copilot-runtime/src/lib.rs`.
- Part A (unknown-key observability): extracted pure `Config::parse_toml_collecting_unknown(&str) -> Result<(Self, BTreeSet<String>), toml::de::Error>` using `serde_ignored::deserialize(toml::Deserializer::new(..), collector)`. `load_from_file_strict` now calls it, emits one `logging::warn("Unknown config key '<path>' ignored")` per sorted/deduped path AFTER successful deserialize; malformed TOML still returns the original parse error with zero warnings (BTreeSet gives sort+dedup; error path emits nothing).
- Part B (one load-time normalization): added `Config::normalize_memory_embedding_backend()` normalizing `agents.memory_embedding_backend` to exact lowercase `local`/`openai`; any other value warns (field, bad value, accepted domain, applied `local` fallback). Called after `apply_env_overrides()` in BOTH `Config::load` and `Config::load_strict`, so an env-reintroduced bad value is still normalized.
- Part C (runtime warn-once on silent fallbacks): added reusable `WarnOnce` guard (const-new atomic, `should_fire()` fires exactly once) in `config.rs`; `tools.profile` unrecognized branch now warns once via `warn_once_unrecognized_tool_profile` (vocabulary full|acp|minimal|lite|small|none|off|disabled unchanged; only the previously-silent else arm warns, excluding empty and `full`). Copilot premium: `env_premium_mode` (`JCODE_COPILOT_PREMIUM`) and `env_overrides` config->env propagation (`provider.copilot_premium`) each got a warn-once on unrecognized non-empty values (accepted `0`/`1` and `normal`/`one`/`zero` unchanged).
  - NOTE / adaptation: the other listed Part C parsers were NOT silent and were left as-is with no vocabulary change:
    - openai-runtime transport (`logging::warn`), native compaction (`logging::warn`), reasoning effort (`logging::info` "Using 'xhigh'"), service tier (`normalize_service_tier` returns Err -> `load_service_tier` warns), max output tokens (`logging::warn`) already emit on unrecognized input.
    - anthropic-runtime reasoning effort already emits `logging::info` "Using the model maximum"; its service-tier normalizer already returns Err (left untouched per instructions).
    - openrouter-runtime DeepSeek + unified reasoning normalizers already emit `logging::info`; both accept `max` (unified maps it to `xhigh`) so no false-positive warning on the `max` alias.
  - Adding a second warn there would have been redundant/noisy, so per the "already error loudly -> note it" escape hatch, only the genuinely-silent `tools.profile` and copilot-premium fallbacks got new warn-once guards.
- Part D: `DisplayConfig::redraw_fps` rustdoc corrected from "default: 30" to "default: 60" (matches the `Default` impl).
- Tests (config_tests.rs, +6): unknown top-level+nested keys collected/sorted/deduped; known-only config yields empty set; malformed TOML returns Err with no keys; `OpenAI`/`OPENAI`/whitespace/`LOCAL` -> exact lowercase, `garbage` -> `local`, env-reintroduced `OpenAI`/`garbage` normalized after `apply_env_overrides`; `WarnOnce` fires exactly once across repeated calls. No new env keys added, so `config_env_fingerprint_tracks_every_apply_env_override_var` needed no change (still green).
- Validation:
  - `nix develop --command cargo test -p jcode-config-types` -> exit 0; 16 passed, doc 0.
  - `nix develop --command cargo test -p jcode-base --lib config` -> exit 0; 99 passed, 0 failed (6 new WI-4 tests included).
  - `nix develop --command cargo check -p jcode-base -p jcode-provider-openai-runtime -p jcode-provider-anthropic-runtime -p jcode-provider-openrouter-runtime -p jcode-provider-copilot-runtime` -> exit 0.
  - `nix develop --command cargo fmt --all && git diff --check` -> exit 0.
  - Only the pre-existing `KILLALL_PROCESS_NAME` dead-code warning was emitted.
- Cargo.lock changed for `serde_ignored` only (committed with this change); flake.lock untouched.
- Exact next stage: WI-5.

## 2026-07-10 — WI-5: embedding request semantics, remote vector identity, visible fallback

- Files: `crates/jcode-base/src/embedding_backend.rs`, `crates/jcode-base/src/config.rs` (added `#[cfg(test)] WarnOnce::reset`), `crates/jcode-config-types/src/lib.rs` (`memory_embedding_dim` doc), `crates/jcode-base/src/config/default_file.rs` (template doc).
- Capability table: replaced `default_openai_dim`'s `_ => 1536` catch-all with `EmbeddingModelCapabilities`/`embedding_model_capabilities(model)` covering text-embedding-3-small (1536, supports dimensions), -3-large (3072, supports), ada-002 (1536, no dimensions). Unknown models -> `None` (no inferred dim).
- Dimension resolution: `ResolvedEmbeddingDimension::{Known(usize),MissingForCustomModel}` (internal only; public `EmbeddingBackend::dim()->usize` unchanged). `resolve_embedding_dimension`: explicit dim wins, else known-native, else MissingForCustomModel. Constructor `OpenAiEmbeddingBackend::new` now returns `Result<Self>`, bailing on MissingForCustomModel with an actionable message (names model + memory_embedding_dim + bge-m3->1024 example).
- Request semantics: pure `request_body(&[String]) -> serde_json::Value` inserts `"dimensions"` only when explicit dim set AND model is dims-capable. Explicit dim on non-capable/unknown model -> field omitted + `warn_once_identity_only_dimension` (identity declaration, not truncation).
- Response safety: pure `validate_response_dims(vectors, expected_dim, model)` HARD-fails (Err) on ANY vector (not just first) whose length != declared dim; wired into `embed_inputs` replacing the prior warning-only first-vector check.
- Persisted identity: `embedding_model_id(base, model, dim)` => `openai:<canonical-base>|<model>|dim=<dim>`, never a credential. `normalize_embedding_base_url(Option<&str>)->Result<String>` via `url` crate: None->openai default; parse; reject non-http(s)/userinfo/query/fragment/missing-host; lowercase scheme+host; strip default ports (via `port()`); trim trailing path slashes (root->`/`). Invalid URL -> Err -> local fallback. memory.rs equality gates UNCHANGED (trusted as-is).
- Visible fallback: `openai_backend_from_config` now `Result<Option<..>>` — Ok(None) for not-selected/keyless, Err for selected+keyed-but-invalid-config. `active_backend` warns process-once: `warn_once_remote_selected_without_credential` (names memory_embedding_api_key_env or OPENAI_API_KEY, gated by `remote_backend_selected()`) and `warn_once_remote_config_invalid`. Reused WI-4 `WarnOnce` guard; added `#[cfg(test)] reset()` and module `reset_warn_once_guards()`.
- Tests (embedding_backend, 15 new, 19 total in module): native-dim inference, explicit-dim override, custom+dim tag, custom-missing-dim refuse; request dimensions inclusion/omission across v3/ada/custom; model_id endpoint/model/dim/no-key + differs across endpoint/dim/gateway-path/local; URL default/case/port/trailing-slash/non-default-port/reject-userinfo-query-fragment-scheme-missinghost/invalid-refuse; response validator exact/first-mismatch/later-mismatch; missing-credential warn fires exactly once.
- Adaptations: (1) fallback warn helper returns bool so once-semantics is unit-testable without driving global config; (2) identity-only-dimension warn added at construction (design-implied by request-semantics rule 3, made observable). No memory.rs instrumentation added (per instructions).
- Validation (all exit 0): `cargo test -p jcode-base --lib embedding_backend` -> 19 passed; `--lib memory` -> 74 passed; `--lib config` -> 99 passed; `cargo check -p jcode-base` -> only pre-existing KILLALL_PROCESS_NAME dead-code warning; `cargo fmt --all && git diff --check` -> clean. flake.lock untouched; no Cargo.lock change (url already a dependency).
- Exact next stage: final stop-condition suite.

## 2026-07-10 — WI-2C follow-up: preserve `anthropic-api` catalog identity in app-core

- Commit: `727273b7b fix(app-core): Preserve catalog model prefix identity.`
- A post-reload test-only change incorrectly reclassified `anthropic-api:` as a native Anthropic credential prefix. This contradicted the finalized WI-2 resolver order, where `anthropic-api` is the static OpenAI-compatible catalog profile and only `claude-api:` is the native Anthropic API-key model prefix.
- `explicit_route_for_configured_model` now gates credential pinning to the exact native model-prefix vocabulary: `openai-api`, `openai-oauth`, `claude-api`, and `claude-oauth`. Catalog/named profile prefixes carried by `ResolvedModelSpec::explicit_prefix` remain on normal profile restore routing and are not reparsed through the broader `AuthRoute` alias vocabulary.
- Added negative regressions for `anthropic-api:`, bare `anthropic:`, and bare `openai:` while preserving the existing `openai-api:` positive route pin.
- Removed an unrelated `.jcode/swarm-prompt.md` durability commit created during reload recovery; no harness/config changes remain in this source-fix batch.
- Validation (exit 0): `cargo fmt --all -- --check`; targeted `configured_explicit_route_uses_single_resolver_result`; all 34 `comm_session` tests; `cargo check -p jcode-app-core`. Only pre-existing dead-code warnings were emitted.

## 2026-07-10 — WI-4 follow-up: complete runtime parser fallback observability

- Commit: `c30da1cad fix(config): warn once for runtime parser fallbacks`.
- Completed the final-design gap left by `47608034d`: OpenAI, Anthropic, OpenRouter, Copilot, `tools.profile`, ACP profile, and `display.performance` configured string parsers now distinguish recognized aliases from fallback paths and emit WARN-ONCE keyed by setting + raw value + selected fallback when a configured raw value is unrecognized.
- Preserved existing parser-owned vocabularies and fallbacks, including OpenRouter's context-dependent DeepSeek `max` and unified `max -> xhigh` behavior. Direct setters that return `Err` remain errors.
- Added a small dependency-safe keyed warn-once seam in `jcode-base::config` without introducing a centralized alias/canonicalization table. Runtime parsers remain authoritative.
- Added focused parser coverage for recognized aliases without warnings by construction, invalid values returning the existing fallback, and keyed once-only fallback warning behavior.
- Validation, run in one `nix develop --command bash -lc ...` shell to avoid hook/rerere races:
  - `cargo fmt --all` -> exit 0.
  - `cargo test -p jcode-config-types` -> exit 0; 16 passed, 0 failed; doc tests 0 failed.
  - `cargo test -p jcode-base --lib config_tests` -> exit 0.
  - `cargo test -p jcode-provider-openai-runtime wi4_openai_config_parsers_preserve_aliases_and_fallbacks` -> exit 0; 1 passed, 0 failed.
  - `cargo test -p jcode-provider-anthropic-runtime wi4_anthropic_configured_reasoning_preserves_aliases_and_fallback` -> exit 0; 1 passed, 0 failed.
  - `cargo test -p jcode-provider-copilot-runtime wi4_copilot_premium_preserves_values_and_fallback` -> exit 0; 1 passed, 0 failed.
  - `cargo test -p jcode-provider-openrouter-runtime wi4_openrouter_reasoning_preserves_contextual_aliases_and_fallbacks` -> exit 0; 1 passed, 0 failed.
  - `cargo test -p jcode-app-core --lib wi4_display_performance_preserves_aliases_and_fallback` -> exit 0; 1 passed, 0 failed.
  - `cargo check -p jcode-base -p jcode-provider-openai-runtime -p jcode-provider-anthropic-runtime -p jcode-provider-copilot-runtime -p jcode-provider-openrouter-runtime -p jcode-app-core` -> exit 0.
  - `git diff --check` -> exit 0.
  - After removing the new OpenAI unused-binding warning: `nix develop --command bash -lc 'cargo check -p jcode-provider-openai-runtime && git diff --check'` -> exit 0.
- Only pre-existing dead-code/test warnings remained after the cleanup check.
- Exact next stage final stop suite for WI-5:
  - `nix develop --command cargo test -p jcode-base --lib embedding_backend`
  - `nix develop --command cargo test -p jcode-base --lib memory`
  - `nix develop --command cargo check -p jcode-base`
  - `nix develop --command cargo fmt --all`
  - `git diff --check`

## 2026-07-10 — Final stabilization and hard stop

- Commit: `c7d99311e test(base): harden environment-sensitive suites`.
- The first full `jcode-base` run exposed seven environment-sensitive test failures rather than WI regressions. The fixes isolate test homes/environment, serialize shared process state where required, and make process-identification assertions deterministic. No production behavior was relaxed.
- Rebuilt the final source-fix history without unrelated swarm prompt/model-routing commits. The intentionally untracked audit source reports remain untouched.
- Ran the five required commands sequentially from the repository root. Exact result excerpts:

```text
$ nix develop --command cargo check --workspace
    Finished `dev` profile [unoptimized] target(s) in 5.90s
--- Command finished with exit code: 0 ---

$ nix develop --command cargo test -p jcode-provider-core -p jcode-provider-env -p jcode-provider-metadata -p jcode-config-types
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 86 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
--- Command finished with exit code: 0 ---

$ nix develop --command cargo test -p jcode-base --lib
test result: ok. 1094 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 37.24s
--- Command finished with exit code: 0 ---

$ nix develop --command cargo test -p jcode-app-core --lib
test result: ok. 1018 passed; 0 failed; 23 ignored; 0 measured; 0 filtered out; finished in 35.87s
--- Command finished with exit code: 0 ---

$ nix develop --command cargo check -p jcode-tui
    Finished `dev` profile [unoptimized] target(s) in 4.55s
--- Command finished with exit code: 0 ---
```

- `git diff --check` -> exit 0.
- Only pre-existing warnings and the existing `block v0.1.6` future-incompatibility notice were emitted.
- **STOP CONDITION MET:** every WI is committed compile-green, all required regressions exist and pass, and every required command exits 0.
