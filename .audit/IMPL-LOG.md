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
