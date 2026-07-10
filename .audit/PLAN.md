# IRONCLAD source-fix plan

**Prepared:** 2026-07-10
**Planning baseline:** the working tree already contains the uncommitted
`agents.memory_embedding_api_key_env` change in
`jcode-config-types/src/lib.rs`, `jcode-base/src/config.rs`,
`config/env_overrides.rs`, `config/default_file.rs`, and
`embedding_backend.rs`. `nix develop --command cargo check -p jcode-base`
passed on that baseline. It is a prerequisite, not work to be repeated below.

## Decisions made during verification

- **Do not use `register_api_key_fallback_resolver` for Z.AI.** Although
  `jcode-provider-env` exposes that registry, source search found no caller
  registering a resolver. It is a dormant global callback, and it does not
  carry the profile/env-file context needed to make the catalog authoritative.
  Profile-aware credential loading is the correct seam.
- **Keep `SidecarBackend`.** `Provider` is a necessary generic path, but the
  OpenAI and Claude arms contain real OAuth-only endpoint/fallback behavior.
  Removing the enum would be an unverified redesign. Instead, make the
  generic arm correctly fork a provider for the configured model.
- **Use observable permissive parsing, not `deny_unknown_fields`.**
  `Config::load()` deliberately survives stale config, and `UpdateChannel` is a
  reload-handoff compatibility exception. Runtime-owned string parsers remain
  authoritative for their aliases and fallbacks, but must warn once when an
  unrecognized configured value falls back. Only the embedding-backend selector
  is normalized at load time because WI-5 depends on an exact `local|openai`
  value.
- **Do not replace `CONFIG_ENV_KEYS` with a giant macro/table in this pass.**
  The current extraction test verifies every direct `apply_env_overrides`
  `std::env::var` reader is listed, while `CONFIG_ENV_KEYS` also correctly
  contains path and indirect hook inputs. Encoding 130 heterogeneous mutation
  blocks as callback entries would be a riskier rewrite than the current
  guard. WI-4 tightens the guard and documents this deliberate non-collapse.
- **Do not invent a global embedding-model catalog.** No source catalog exists
  for arbitrary OpenAI-compatible gateway models. The protocol-local table in
  WI-5 will describe only models for which OpenAI guarantees `dimensions`;
  custom models must supply `memory_embedding_dim`. This fixes the incorrect
  `_ => 1536` claim without pretending arbitrary gateway metadata is known.

## Ordered work items

### WI-0 — Repair `MultiProvider::fork()` provider-state isolation

- **Findings addressed:** New HIGH finding from council review: `MultiProvider::fork()` currently shares the live Copilot, Antigravity, and Gemini provider objects.
- **Dependencies:** None. **WI-3 is blocked on this item.**

#### Exact changes

1. **`crates/jcode-base/src/provider/mod.rs`**
   - In `MultiProvider::fork()` (currently beginning at `provider/mod.rs:2538`), replace the bare
     `Arc::clone` retrieval for `copilot_api`, `antigravity`, and `gemini` with
     a read of the optional inner `Arc<dyn Provider>` followed by
     `provider.fork()`. Preserve `None` when a slot is absent. The cloned
     `Arc` is the live object and is not an isolation boundary.
   - Keep the existing factory-based fresh-instance behavior for Claude,
     Anthropic, OpenAI, Cursor, Bedrock, and OpenRouter unchanged. This is a
     surgical repair to the three asymmetric slots, not a rewrite of fork
     construction or selection restoration.
   - Source verification confirms all three inner implementations provide a
     safe model fork: `GeminiProvider::fork` creates a fresh
     `Arc<RwLock<String>>` model (`jcode-provider-gemini-runtime/src/lib.rs`
     `GeminiProvider::fork`, 1006-1014), as do `AntigravityProvider::fork`
     (`jcode-provider-antigravity-runtime/src/lib.rs`
     `AntigravityProvider::fork`, 736-748) and `CopilotApiProvider::fork`
     (`jcode-provider-copilot-runtime/src/lib.rs` `CopilotApiProvider::fork`,
     1021-1042).
2. **Regression tests in the existing `jcode-base` provider test module**
   - For each active slot Gemini, Antigravity, and Copilot: capture the
     registered live provider's `model()`, call `MultiProvider::fork()`, set a
     distinct valid model on the returned fork, then assert the originally
     registered provider's `model()` is unchanged.
   - Exercise successful mutation of the fork, not only fork/set failure. The
     previous bug succeeds while silently changing the live session.
   - For Copilot, arrange the test so the fork is idle and select a valid
     non-1M model. `CopilotApiProvider::set_model` uses `try_write` and can
     return `Cannot change model while a request is in progress`; this expected
     error must be propagated/handled by callers rather than assumed impossible.

#### Why this is the right seam

`MultiProvider::fork()` is the shared isolation boundary used by the sidecar
and other fork consumers. The concrete runtime providers already know which
state may be shared and explicitly allocate independent model state in their
own `fork()` implementations. Calling that trait method at the aggregation
boundary fixes the actual ownership error once, rather than attempting to
protect only the new sidecar caller.

#### Blast radius

Only forks of the three affected provider slots change. Their clients,
credentials, catalogs, and other immutable/shared runtime resources retain the
leaf providers' existing fork semantics. The active session's provider and
model must remain untouched.

#### Validation

```bash
nix develop --command cargo test -p jcode-base --lib provider
nix develop --command cargo check -p jcode-base
```

#### Risk and rollback

The regression test gates the sole behavior change: model selection on a fork
can no longer leak into the active session. If a leaf provider's fork proves
incomplete, revert this isolated commit rather than restoring shared mutable
state. Copilot's transient `try_write` failure remains an ordinary recoverable
error and must not be hidden.

---

### WI-1 — Provider credentials are config-first, one path

- **Findings addressed:** A1, A2, A3, plus the catalog-provenance hole documented in `.audit/SOL-ASSESSMENT-R3.md`.
- **Dependencies:** None. It may land before or after the already-present embedding `api_key_env` baseline, but does not modify that feature.

#### Exact changes

1. **Make profile-aware credential loading the single catalog path.**
   - Add ordered legacy credential aliases to static and resolved OpenAI-compatible profiles. Z.AI keeps primary `ZHIPU_API_KEY` with the one real legacy alias `ZAI_API_KEY`.
   - Correct MiniMax from the erroneous `OPENAI_API_KEY` primary to `MINIMAX_API_KEY`.
   - Expose one typed, profile-aware credential source/loading path for every caller holding an `OpenAiCompatibleProfile` or `ResolvedOpenAiCompatibleProfile`.
   - Make the lossy bare `(api_key_env, env_file)` loader private or otherwise unreachable outside `jcode-provider-env`. Reserve primary-only credential sources for genuinely non-catalog origins such as Azure, Bedrock, named-user profiles, and explicit user env overrides.
   - Preserve the exact lookup order: primary process env, primary env-file entry, then each metadata alias in declaration order with process env before env-file entry. Apply the common secret sanitation to every candidate.

2. **Carry typed catalog provenance through OpenRouter auth.**
   - Do not use the presence of `JCODE_OPENROUTER_*` variables as the provenance boundary. `apply_openai_compatible_profile_env` currently writes internally selected catalog profiles into those variables during startup, which makes catalog activation indistinguishable from a genuine user override if strings are re-read later.
   - Carry a typed catalog-profile credential source through `OpenRouterProvider::new`, `resolve_auth`, and `get_api_key`. Internally activated/default catalog profiles must retain their aliases even when startup compatibility variables have been populated.
   - Genuine explicit `JCODE_OPENROUTER_API_KEY_NAME` / `JCODE_OPENROUTER_ENV_FILE` overrides and named user profiles remain primary-only.
   - Delete the hardcoded `ZHIPU_API_KEY` special case only after every profile-holding consumer uses the typed path.

3. **Migrate every profile-holding consumer rather than reconstructing pairs.**
   - Update OpenRouter runtime autodetection, refresh, default construction, and dedicated compatible-profile construction.
   - Update base catalog/configuration probes, auth status/lifecycle, usage reporting, pricing, live tests, Codex/OpenAI key reads, external-auth validation, provider doctor/init/auth-test CLI paths, provider-doctor lifecycle, and TUI credential migration wherever the source already owns a static or resolved profile.
   - `openrouter_like_api_key_sources()` returns typed sources rather than string pairs. Diagnostics and prompts derive candidate names from the typed source.
   - Alias discovery is read-only compatibility. Login/save/sync continues writing only the canonical primary key and env file.

4. **Tests and mechanical guard.**
   - Metadata tests assert catalog primary env names are unique, MiniMax owns `MINIMAX_API_KEY`, and OpenAI alone owns `OPENAI_API_KEY`.
   - Loader tests cover primary-env/file precedence over aliases, alias declaration order, alias env/file loading, sanitation, MiniMax/OpenAI isolation, and genuine primary-only controls.
   - Add a guard that fails if a catalog/resolved-profile-holding call site can reach the bare pair loader. Type/API privacy is the primary invariant, with a focused source audit/compile-fail guard as a backstop.
   - Add the required end-to-end regression: activate Z.AI with only `ZAI_API_KEY` through `apply_openai_compatible_profile_env` or default-provider startup, construct the default OpenRouter runtime, and prove `resolve_auth -> get_api_key` retains bearer auth after removal of the leaf special case.
   - Keep explicit override and named-user controls proving those sources do not probe catalog aliases.

#### Superseded approach (why)

The previous WI-1 attempted to contain a static catalog through a hand-enumerated caller table and treated `JCODE_OPENROUTER_*` presence as an explicit-override boundary. `.audit/SOL-ASSESSMENT-R3.md` hole #1 proved startup launders catalog profiles through those same variables. The real cut is a typed credential source carried end-to-end, so provenance cannot be erased and future catalog callers cannot bypass aliases through a pair loader.

#### Validation

```bash
nix develop --command cargo test -p jcode-provider-metadata -p jcode-provider-env
nix develop --command cargo test -p jcode-provider-openrouter-runtime
nix develop --command cargo test -p jcode-base --lib provider_catalog
nix develop --command cargo test -p jcode-app-core --lib external_auth
nix develop --command cargo check --workspace
```

#### Risk and rollback

The behavioral risk is changing precedence or accidentally applying catalog aliases to a genuine explicit override. Keep the ordered scanner covered by matrix tests and keep primary-only constructors typed by non-catalog origin. Land as one focused commit after the catalog-path guard and alias-only default-runtime regression pass.

---

### WI-2 — One context-aware model-spec parser and resolver

- **Findings addressed:** B2, B3, B4, B6.
- **Dependencies:** None. WI-3 depends on this item.

#### Exact changes

1. **`crates/jcode-provider-core/src/selection.rs`**
   - Make `ActiveProvider` own its canonical key and aliases through methods
     such as `key()`, `aliases()`, and `from_key_or_alias()`. Refactor
     `parse_provider_hint`, `provider_key`, `provider_from_model_key`, and the
     builtin portion of `cli_provider_arg_for_session_key` to delegate to it.
     This removes the current divergent `bedrock` vocabulary.
   - Replace the 13-arm `explicit_model_provider_prefix` chain with a table
     encoding **the current model-prefix vocabulary**, then parse its recognized
     dual-auth spellings through `AuthRoute::parse_explicit_credential_prefix`.
     Do not use broad `AuthRoute::parse` as a prefix recognizer: it accepts
     runtime/CLI aliases such as `anthropic-api` (`auth_mode.rs:128-146`) that
     current model-prefix parsing intentionally does **not** recognize
     (`selection.rs::explicit_model_provider_prefix`, 164-193). Retain exact
     auth-route spelling in the result because OAuth/API intent must survive.
2. **`crates/jcode-provider-core/src/models.rs`**
   - Demote the public static `provider_for_model[_with_hint]` logic to an
     explicitly named builtin-only helper returning `ActiveProvider`. It remains
     the dependency-safe fallback for `jcode-provider-core`; it must not claim
     to resolve named config profiles.
   - Make `provider_key_from_hint` delegate to `ActiveProvider` rather than
     maintain a second string match. This fixes the verified `bedrock` omission.
   - Update the private core `openrouter_catalog_model_id` caller to use this
     *explicitly builtin-only* helper. It is not a runtime resolver because the
     core crate cannot see `Config`.
3. **`crates/jcode-base/src/provider/models.rs` and
   `crates/jcode-base/src/provider/selection.rs`**
   - Introduce one public base-layer API:

     ```rust
     pub struct ResolvedModelSpec {
         pub provider_key: Option<String>,
         pub bare_model: String,
         pub explicit_prefix: Option<String>,
     }
     pub fn resolve_model_spec(model: &str, cfg: &Config) -> ResolvedModelSpec;
     ```

     Resolution order is fixed and tested: valid explicit dual-auth/builtin
     prefix, recognized openai-compatible catalog profile prefix, configured
     `cfg.providers` profile prefix, `model@provider` OpenRouter form, then the
     base builtin detector (including Bedrock, Antigravity, and Cursor). An
     unrecognized `prefix:model` remains an unclassified model, not an implicit
     provider. `openai-api:` is permanently reserved by the current explicit-
     prefix vocabulary and must win before the same-named catalog profile id.
     **Sol's proposed equivalent `anthropic-api:` reservation is not adopted**:
     source disproves it. `explicit_model_provider_prefix` recognizes
     `claude-api:` but has no `anthropic-api:` arm (provider-core
     `selection.rs:164-193`), while the static catalog defines
     `ANTHROPIC_OPENAI_COMPAT_PROFILE.id = "anthropic-api"`
     (`provider-metadata/src/catalog.rs:99-115`). Thus
     `anthropic-api:<model>` remains a catalog-profile prefix today and after
     this refactor. The resolver table must preserve both behaviors rather than
     broadening `AuthRoute::parse` aliases into model prefixes, preserving the
     existing HTTP-client, auth, pricing, and billing paths.
   - Expose only thin compatibility wrappers while moving callers. A convenience
     `resolve_current_model_spec(model)` may read `config()` for read-only UI
     paths, but all selection paths that already possess a `&Config` must pass
     it explicitly. Remove the duplicate base `provider_for_model` body rather
     than letting it remain an alternate source of truth.
   - Rewrite `explicit_session_provider_key_for_model_request`,
     `model_switch_request_for_session_model`, and
     `resolve_config_provider_selection` to consume `ResolvedModelSpec`. Keep
     their session-key/auth-route normalization after parsing, not in another
     prefix parser.
4. **Move every production model-spec classifier.**
   - Base: `provider/catalog_routes.rs`, `pricing.rs`, `route_builders.rs`,
     `provider/mod.rs`, capability lookup, selection, and all public
     `provider_for_model[_with_hint]` exports.
   - App-core: retain the existing override-precedes-model policy in
     `provider_key_for_spawn_model` and `provider_key_for_launch_model` in
     `src/server/comm_session.rs` and `src/server/jade_relay.rs`. Only replace
     each helper's inner `split_once(':')` classification block with
     `resolve_model_spec(model, cfg).provider_key`; do not move spawn/launch
     override policy into `jcode-base` or collapse these helpers into a base
     provider-key API. Also rewrite
     `comm_session.rs::explicit_route_for_configured_model` (262-293) to consume
     `ResolvedModelSpec::{explicit_prefix,bare_model,provider_key}`. This source
     currently both parses `explicit_model_provider_prefix` and independently
     reparses `AuthRoute`; the resolver output is the one parsed authority.
   - **TUI migration table.** These are semantic policies, not mechanical name
     substitutions. Each source currently calls the static classifier or parses
     `prefix:model`, shown by the cited lines; each must receive current config
     through `resolve_current_model_spec`/`resolve_model_spec`.

     | TUI function and source proof | Authoritative input / precedence | Required resolver behavior |
     |---|---|---|
     | `inline_interactive.rs::model_picker_provider_hint_from_model_spec` (238-264), used by default-route marking (278-325) | Full configured default model spec is authoritative, then explicit `config_default_provider` still constrains route matching | Delete hand parser/nine-name table. Use `ResolvedModelSpec` (`bare_model` for name match, `provider_key` for route match) so named `omlx:` and dual-auth aliases mark defaults. |
     | `inline_interactive.rs::extend_remote_routes_for_uncovered_models` (396-425) | Each remote catalog model plus current config/auth status; only Claude identity controls dual-auth route synthesis | Resolve each model with current config before deciding Claude. A named profile must not synthesize Claude API/OAuth routes just because its bare model resembles Claude. |
     | `inline_interactive/openers.rs::open_agent_model_picker` (173-207) | Full picker entry/saved spec, not `model_entry_base_name(entry)` | Replace the OpenAI/Claude-only memory filter with **no provider-family filter**: keep all full picker entries and preserve saved override matching against the exact persisted spec. This is deliberate because WI-3 accepts all resolver-recognized profiles/builtins and reports a fork failure at sidecar construction. Do not probe `fork_with_model_spec` merely to populate a UI picker, because it can allocate clients or mutate a temporary provider. |
     | `tui_lifecycle_runtime.rs::new_for_replay_with_title` (14-46) | Persisted `session.provider_key` is authoritative when present; only otherwise resolve persisted `session.model`; final fallback remains current demo Claude | Never overwrite a saved route key by re-resolving a bare model. Map saved key to display identity first, then resolver fallback. |
     | `tui_state.rs::remote_effort_identity` (95-105) | Server `remote_provider_name` wins, then resolver of server/session/config model, then configured hint | Use resolver only for model-derived fallback so named profile effort identity is stable without overriding server truth. |
     | `tui_state.rs::remote_header_provider_name` (179-190) | Server provider name wins, then resolver of effective remote model, then configured hint | Same precedence as existing UI but resolver supplies configured profile key. |

     `ui_header.rs::parse_provider_hint` is excluded from this resolver migration:
     it receives an already-derived provider display/name rather than a model
     spec (`ui_header.rs:421-454`). It should instead benefit from
     `ActiveProvider` canonical aliases.
   - **OpenRouter runtime:** replace
     `jcode-provider-openrouter-runtime/src/lib.rs::strip_session_profile_prefix`
     (1360-1404) with a resolver-backed predicate. This crate demonstrably has
     `Config` (`config().providers` at 1399-1402), so it is not eligible for the
     provider-core no-Config exception. Preserve its critical provider-instance
     guard: strip only a prefix resolving to this `OpenRouterProvider`'s own
     profile/compatible profile it serves; never strip a recognized builtin
     prefix that intends to switch away from OpenRouter. The current builtin
     guard at 1377-1381 is source evidence for that invariant.
   - Preserve core's isolated builtin helper only where a no-`Config` core API
     genuinely needs a static classification. It must no longer be named or
     documented as the general resolver.
5. **Tests**
   - Table-test one resolver result for bare Claude/OpenAI/Gemini, Bedrock,
     Cursor, OpenRouter `@`, each dual-auth prefix, a catalog profile prefix,
     a named `[providers.omlx]` profile prefix, and an unknown prefix.
     Add a negative collision case for `openai-api:gpt-5.5`: it must resolve
     as the native pinned dual-auth route and prove the same-named catalog
     profile lookup is unreachable. Add the converse preservation test for
     `anthropic-api:<model>`: it must resolve as the OpenAI-compatible catalog
     profile, not a newly accepted native model-prefix alias.
   - Assert both spawn and Jade relay return the same provider key for every
     explicit/named case. Put shared parser tests in base and focused wrapper
     tests in app-core. Include `explicit_route_for_configured_model` asserting
     its bare model/provider key comes from the one resolver result.
   - Add focused TUI tests for configured `omlx:` default-route marking, memory
     picker eligibility, replay identity with saved provider key overriding a
     conflicting model inference, remote Claude dual-auth route synthesis,
     remote effort identity, and header identity.
   - Add OpenRouter runtime tests that strip its own named/catalog profile prefix
     for the wire model but retain `openai-api:`/other builtin prefixes intended
     to switch providers. Add session-restore wire-model coverage.
   - Add regression coverage that pricing, catalog route classification, and
     TUI header resolution see the named `omlx` key, not `None`.
   - Before removing wrappers, workspace-search for
     `provider_for_model`, `provider_for_model_with_hint`,
     `explicit_model_provider_prefix`, and `split_once(':')` in model-selection
     code. Every production hit must be either the named core-only fallback or
     explicitly documented non-model parsing; no public general classifier
     wrapper survives WI-2C.

#### Before/after sketch

```rust
// Before: every caller picks a partial resolver or re-parses ':' itself.
if let Some((prefix, _)) = model.split_once(':') { /* three local checks */ }
provider_for_model(model) // static-only, differs by crate

// After: the base owns runtime interpretation once.
let spec = resolve_model_spec(model, cfg);
let provider_key = spec.provider_key.as_deref();
// session/auth code consumes `explicit_prefix`; execution consumes `bare_model`.
```

#### Why this is the right seam

The disagreement is caused by resolver choice being based on crate locality.
The base layer is the first layer with both provider metadata and `Config`, so
it is the only layer that can correctly resolve catalog and named profiles.
`provider-core` retains only context-free facts. `ActiveProvider` then becomes
the one owner of builtin names and aliases instead of nine string tables.

#### Blast radius

This is broad but **not** a mega-commit. Develop it through three compile-green
checkpoints: **WI-2A** provider-core `ActiveProvider` canonical APIs plus clearly
named builtin-only helpers while forwarding wrappers remain; **WI-2B** base
`ResolvedModelSpec` plus every base caller; **WI-2C** app-core, OpenRouter runtime,
and all six TUI surfaces, then remove forwarding general-classifier wrappers only
when the required workspace search is clean. Dependency direction makes this
safe: app-core/TUI consume base, and base consumes provider-core. Saved-session
vocabulary and `AuthRoute` spellings retain their serialized values throughout.

#### Validation

```bash
# Run at each WI-2A/B/C checkpoint so every staging commit is compile-green.
nix develop --command cargo test -p jcode-provider-core
nix develop --command cargo test -p jcode-base --lib model_resolution
nix develop --command cargo test -p jcode-app-core --lib comm_session
nix develop --command cargo test -p jcode-app-core --lib jade_relay
nix develop --command cargo test -p jcode-provider-openrouter-runtime
nix develop --command cargo check -p jcode-base -p jcode-app-core -p jcode-provider-openrouter-runtime -p jcode-tui
```

#### Risk and rollback

This is the largest semantic change: provider/auth selection can be altered by
prefix precedence. The resolver table tests, including the `openai-api:` reservation negative and
`anthropic-api:` catalog-profile preservation positive, are the rollback safety
net against silently rerouting a prefix to a different HTTP client or billing
path. Keep legacy wrappers only for the commit while all callers migrate, mark
them deprecated, then remove them before merging the same work item. Revert the
commit to restore the previous independent parsers if any persisted-session
route regresses.

---

### WI-3 — Sidecar model overrides route through a forked resolved provider

- **Findings addressed:** B1, B5, D5 (sidecar override half).
- **Dependencies:** WI-0 and WI-2. WI-0's successful isolation regression is a
  merge gate, because this item calls `set_model` on the fork.

#### Exact changes

1. **`crates/jcode-provider-core/src/lib.rs`**
   - Add the object-safe synchronous default
     `Provider::fork_with_model_spec(&self, model_spec: &str) ->
     Result<Arc<dyn Provider>>` as exactly:

     ```rust
     let fork = self.fork();
     fork.set_model(model_spec)?;
     Ok(fork)
     ```

     This is isolation by construction, not an "unsupported" default. It adds
     no fan-out across the 62 implementations: `Provider` is already object-safe
     `Send + Sync` (`jcode-provider-core/src/lib.rs:66-77`) with synchronous
     `fork(&self) -> Arc<dyn Provider>` (407-415), and `set_model` errors remain
     the actionable capability diagnostic for leaf registrations.
2. **`crates/jcode-base/src/provider/mod.rs`**
   - Build on WI-0's independent `MultiProvider::fork`; its default
     `fork_with_model_spec` must call `set_model(model_spec)` on that isolated
     `MultiProvider`. Source proves this retains explicit route pinning:
     `MultiProvider::set_model` parses a prefix with
     `AuthRoute::parse_explicit_credential_prefix` and applies the resulting
     API-key/OAuth mode before setting the target (provider `mod.rs:1767-1810`).
     It therefore receives the full original spec unchanged.
   - Propagate `set_model` failure with the requested model and active provider
     as context. In particular, Copilot's `try_write` can return `Err` while a
     request is in flight. `Sidecar::with_configured_model` must handle this as
     an ordinary fork-construction failure, emit its explicit diagnostic, and
     use the existing auto-selection fallback. It must never assume set-model
     succeeds or mutate the registered live provider as a workaround.
   - Add `active_provider_fork_with_model_spec` beside
     `active_provider_fork`; it delegates to the registered live provider's new
     trait method. It must never mutate the live agent's selection.
3. **`crates/jcode-base/src/sidecar.rs`**
   - Keep the copyable `SidecarBackend::{OpenAI, Claude, Provider}` discriminator
     and add `provider: Option<Arc<dyn Provider>>` on `Sidecar`. `Provider`
     construction stores the model-configured fork once; `complete_via_provider`
     consumes that stored handle and must never re-fetch/fork `ACTIVE_PROVIDER`.
     This is necessary because current `complete_via_provider` does re-fork at
     completion time (`sidecar.rs:204-218`) and would otherwise discard the
     configured model.
   - In `Sidecar::with_configured_model`, call WI-2's
     `resolve_current_model_spec`. The routing rule is intentionally singular:
     **only an unprefixed, native OpenAI or Claude model uses its specialized
     ambient-auth fast path. Every `prefix:model` string, including all explicit
     auth-route spellings and named/catalog profiles, goes to
     `active_provider_fork_with_model_spec` with the original, unstripped text.**

     | Configured model class | Sidecar backend | Wire/configured model and auth reason |
     |---|---|---|
     | Bare native OpenAI or Claude model, no `:` prefix | specialized OpenAI/Claude | Bare `self.model`; retain existing ambient credential behavior and OAuth fallback ladder. |
     | `openai-api:`, `openai-oauth:`, `claude-api:`, `claude-oauth:` | stored `MultiProvider` fork | Full prefix unchanged. `MultiProvider::set_model` already pins credential mode (provider `mod.rs:1767-1810`). |
     | Any other recognized builtin prefix, catalog profile (including `anthropic-api:`), or `[providers.<name>]` prefix | stored `MultiProvider` fork | Full prefix unchanged so the resolver-selected route/profile remains intact. `anthropic-api:` remains a catalog profile, not an explicit native pin. |
     | Unknown prefix or fork/set failure | existing auto-selection fallback | Log requested string plus `ResolvedModelSpec`; no silent unsupported-model message. |

     The specialized branches **cannot** implement explicit pinning safely: they
     send `self.model` directly through their request builders (`sidecar.rs:226-255`
     for OpenAI and 434-480 for Claude) and select credential mode from ambient
     auth (`227-235`, `397-431`), not `ResolvedModelSpec.explicit_prefix`. An
     unstripped `openai-api:`/`claude-api:` would be an invalid wire model.
   - Make `complete_via_provider` require and use the stored fork. A runtime
     `complete_simple` error after successful construction is not retried through
     the OpenAI-specific fallback ladder. It returns a plain provider error, the
     present generic-provider behavior (sidecar.rs:196-218).
   - Retain `SidecarBackend::{OpenAI, Claude, Provider}`. The specialized OAuth
     request and fallback ladder are real behavior, so enum removal is not
     justified by the audited evidence.
   - A runtime failure from the forked provider's `complete_simple` after it
     was constructed is **not** retried through the OpenAI-specific fallback
     ladder. It surfaces as a plain error to the memory caller, matching today's
     generic `SidecarBackend::Provider` behavior. This item adds no fallback
     logic for that path.
4. **Tests: use the existing no-network seams, never concrete downstream runtimes**
   - Extend `provider/tests.rs::StubExternalRuntime` rather than construct real
     Gemini/Antigravity/Copilot runtimes. It exists precisely because downstream
     runtimes cannot be constructed in `jcode-base` tests (763-790), owns an
     independent `RwLock<String>` model, validates models, and forks fresh
     state (830-914). Inject it directly into each private `MultiProvider` slot
     for WI-0 isolation tests.
   - Extend `sidecar.rs`'s existing `StubProvider` (1123-1163) with an
     `RwLock<Vec<String>>` record of fork/set specs and configurable fork/set or
     `complete_simple` failure. It exercises real sidecar dispatch without
     credentials/network. Keep `crate::storage::lock_test_env()` in each test:
     `ACTIVE_PROVIDER` and auth environment/account overrides are process-global
     (sidecar.rs:1038-1045, 1169-1179).
   - Table-test the routing table above: bare OpenAI and bare Claude choose their
     specialized paths; every explicit API/OAuth prefix, `omlx:Qwen3.6-MoE`, and
     a catalog profile reach the stored provider fork with the **exact original
     spec**; the default no-override OAuth preference remains unchanged.
   - Assert fork failure and `set_model` failure log the explicit fallback
     diagnostic and do not mutate the registered live provider. Copilot's
     transient `try_write` error is an ordinary tested failure, not impossible.
   - Assert a failure from `complete_simple` after a successful stored-fork
     construction is returned directly and does not invoke OpenAI/Claude fallback.

#### Before/after sketch

```rust
// Before: non-OpenAI/Claude is discarded; Provider later ignores `self.model`.
_ => { warn("Ignoring unsupported ..."); auto_select_backend() }
active_provider_fork()?.complete_simple(...)

// After: resolve once and fork an isolated execution provider for the override.
let spec = resolve_current_model_spec(&model);
let provider = active_provider_fork_with_model_spec(&model)?;
SidecarBackend::Provider(provider)
```

#### Why this is the right seam

The defect is not only weak classification. The existing generic sidecar path
is model-blind because it forks the active selection at call time. A
model-configured fork is the minimal extension that uses the already-working
`MultiProvider::set_model` machinery without changing the active session or
reimplementing every provider transport. WI-0 establishes that isolation for
every affected slot before this item depends on it. It keeps the dedicated OAuth
paths where they add value.

#### Blast radius

The `Provider` trait is shared across the workspace, so the default method
must compile for every implementation. `ACTIVE_PROVIDER` registration,
`MultiProvider::fork`, sidecar tests, and any test mock that overrides/fakes
forking are the coupled sites. WI-0 must land first to preserve the active
session, and WI-2 must land first because this work consumes its authoritative
parser.

#### Validation

```bash
nix develop --command cargo test -p jcode-provider-core
nix develop --command cargo test -p jcode-base --lib sidecar
nix develop --command cargo test -p jcode-base --lib provider
nix develop --command cargo check -p jcode-base -p jcode-app-core
```

#### Risk and rollback

The risk is an override selecting an unavailable provider/model where the old
code silently chose OAuth. Make the fallback explicit and preserve the old
fallback only on an actual construction or `set_model` error. Do not expand that
fallback to runtime generic-provider failures. The item is one commit and can
be reverted without undoing WI-0's isolation repair or WI-2's general resolver.

---

### WI-4 — Runtime parser is the source of truth; fallback is observable

- **Findings addressed:** C1, C2, C3, C4, C5, D4, A4.
- **Dependencies:** None, but its lowercase embedding-backend normalization must land before WI-5.

#### Exact changes

1. **Warn on ignored configuration keys without rejecting compatible files.**
   - Add `serde_ignored` to `jcode-base` and parse successful TOML loads through it.
   - Collect, sort, and deduplicate ignored paths, then emit one warning per unknown path only after deserialization succeeds. Malformed TOML retains the existing parse error and emits no partial unknown-key warnings.

2. **Keep runtime parsers authoritative for runtime string vocabularies.**
   - Do not add a second load-time alias/canonicalization table for tools profile, transports, reasoning effort, service tier, compaction, or other runtime-owned strings.
   - Refactor each runtime string parser touched by the audit to report whether the configured spelling was recognized or whether its existing fallback was applied. Preserve every runtime's current aliases and context-sensitive behavior, including OpenRouter reasoning-effort handling and `max`.
   - When a configured value is unrecognized and a fallback is applied, emit a WARN-ONCE containing the setting, raw value, and selected fallback. Repeated requests/config reads for the same invalid setting/value must not spam logs.
   - Direct runtime setters that intentionally return an error may retain that contract. This item targets silent fallback paths, not successful aliases or explicit errors.

3. **Keep only genuinely load-time normalization in `Config`.**
   - Normalize `agents.memory_embedding_backend` by trimming and ASCII-lowercasing. Accept only `local` and `openai`; warn and fall back to `local` for any other value. Apply after file parsing and again after env overrides so WI-5 sees exactly lowercase `local|openai`.
   - Do not add an `anthropic_service_tier` config field. It does not exist in the schema and remains runtime/UI-only.
   - Correct `DisplayConfig::redraw_fps` rustdoc from default 30 to default 60.
   - Preserve serialized string/config compatibility and do not rewrite user config files during load.

4. **Tests and guards.**
   - Add parser tests proving each affected runtime accepts its existing aliases without warnings, returns its existing fallback for an invalid value, and warns only once for repeated use of the same invalid configured value.
   - Include OpenRouter's context-dependent reasoning parser in coverage so aliases such as `max` remain runtime-owned and accepted where currently supported.
   - Add config tests for top-level/nested unknown keys, known keys, malformed TOML with no ignored-key warning, file/env lowercase embedding-backend normalization, invalid fallback to `local`, env fingerprint coverage, and duplicate-free `CONFIG_ENV_KEYS`.

#### Superseded approach (why)

The previous WI-4 proposed a shared load-time canonicalization table mirrored into runtime adapters. `.audit/SOL-ASSESSMENT-R3.md` hole #2 showed OpenRouter already has context-dependent reasoning aliases, including `max`, outside that table. A second validator would inevitably diverge. Runtime parsers now remain the single source of truth, while warn-once observability fixes the actual defect: silent fallback on unrecognized configured strings.

#### Validation

```bash
nix develop --command cargo test -p jcode-config-types
nix develop --command cargo test -p jcode-base --lib config_tests
nix develop --command cargo test -p jcode-provider-openai-runtime
nix develop --command cargo test -p jcode-provider-anthropic-runtime
nix develop --command cargo test -p jcode-provider-copilot-runtime
nix develop --command cargo test -p jcode-provider-openrouter-runtime
nix develop --command cargo check -p jcode-base
```

#### Risk and rollback

Warnings can expose long-standing invalid values and must remain once-only to avoid request-path noise. The runtime fallback and alias behavior must not change. Rollback is isolated to parser observability and load-time-only config handling; permissive file parsing remains intact.

---

### WI-5 — Embedding request semantics, remote identity, and visible fallback

- **Findings addressed:** D1, D2, D3, D5 (embedding-dimension half).
- **Dependencies:** The already-present `memory_embedding_api_key_env` baseline
  must be committed or included in this commit. **WI-4 MUST land first.** It
  guarantees that `memory_embedding_backend` is exactly lowercase `"local"` or
  `"openai"` before this selector reads it.

#### Exact changes

1. **`crates/jcode-base/src/embedding_backend.rs`**
   - Replace `default_openai_dim`'s `_ => 1536` with a small
     protocol-local `EmbeddingModelCapabilities` table. It contains
     `text-embedding-3-small` (1536, supports dimensions),
     `text-embedding-3-large` (3072, supports dimensions), and
     `text-embedding-ada-002` (1536, no dimensions). Unknown/custom models
     have no inferred dimension.
   - **Correct the rejected `dim=unknown` design.** Do not create a remote
     backend with an unknown dimension. `EmbeddingBackend::dim()` and
     `OpenAiEmbeddingBackend::dim` remain `usize`: source proves the existing
     public contract is numeric (`embedding_backend.rs:33-40,109-121`),
     constructor resolution is numeric (138-159), and response checking compares
     against it (168-176,220-231). Represent resolution internally as
     `ResolvedEmbeddingDimension::{Known(usize), MissingForCustomModel}` only.
     Change `openai_backend_from_config` from `Option<OpenAiEmbeddingBackend>`
     to `Result<Option<OpenAiEmbeddingBackend>>`: `MissingForCustomModel`, an
     invalid canonical URL, and any declared/response dimension mismatch return
     `Err`; the existing not-selected/no-key cases remain `Ok(None)`. `active_backend`
     logs that `Err` once and chooses local. Thus no error is silently confused
     with an intentional local selection, and no failed remote configuration
     constructs an `EmbeddingBackend` or persists an identity. This is a
     deliberate alternative to an `Option<usize>` public trait change: with
     `active_backend()` freshly constructing a backend on every call
     (278-283, 321-345), an observed-first-response dimension could not remain
     stable across passage/query calls, and a static `dim=unknown` tag would let
     future changed-length vectors pass `memory.rs`'s exact-ID equality gate.
   - Resolution is therefore: explicit `agents.memory_embedding_dim` first,
     otherwise known-native dimension, otherwise `MissingForCustomModel`.
     The template must tell custom endpoints such as bge-m3 to set
     `memory_embedding_dim = 1024`. This makes the memory equality gate sound:
     every remote model ID carries a known declared dimension, and a known
     response length mismatch is a hard request error, never a warning or a
     vector persisted under a false `dim=N` identity. Validate every vector in a
     batch has that declared length, not merely the first.
   - Extract request JSON construction to a pure helper. Include
     `"dimensions": configured_dim` only when the user explicitly supplied a
     dimension **and** the selected model is in the known dimensions-capable
     table. For a requested dimension on a known non-capable/unknown model,
     omit the unsupported field and warn once explaining that the value is an
     identity/sanity declaration, not a server truncation request.
   - Add pure `normalize_embedding_base_url(raw: Option<&str>) -> Result<String>`
     using the existing `url` dependency (`jcode-base/Cargo.toml:61`), then use
     its one canonical output for both request construction and model identity.
     `None` selects `https://api.openai.com/v1`; otherwise require an absolute
     `http` or `https` URL with a host, reject userinfo, query, and fragment,
     lowercase scheme/host through `url::Url`, remove only the scheme default
     port (`:80` HTTP, `:443` HTTPS), remove a single/all redundant trailing
     path slashes so root becomes `/` and `/v1///` becomes `/v1`, and retain
     non-default ports plus path case and interior slashes. An invalid URL is a
     configuration error causing the explicit local fallback, not a guessed
     string normalization. This replaces the current trailing-slash-only logic
     (`embedding_backend.rs:138-152`) with specified behavior.
   - Make the persisted `OpenAiEmbeddingBackend::model_id` include that canonical
     full base URL, bare model, and known declared dimension. Example:
     `openai:https://api.openai.com/v1|text-embedding-3-small|dim=1536`.
     Use the full endpoint, not merely hostname, because two gateway paths at
     one host can be distinct vector services. Never include a key. The exact
     equality gate already compares the opaque ID (`memory.rs::score_and_filter`,
     843-873), so endpoint/model/dimension differences are dense-ineligible by
     construction.
   - Keep `memory.rs` equality gates unchanged: once the ID is complete, its
     existing exact comparison in both retrieval paths is precisely the desired
     guard. New embeddings after URL/model/dimension changes receive a new tag;
     old entries stay BM25-eligible but dense-ineligible. This shrink is
     observable already: `memory.rs::score_and_filter` counts
     `skipped_model_mismatch` and emits its existing info log (`~848-873`) with
     the active model, so no duplicate re-tagging instrumentation is needed.
   - In `active_backend`, if validated config selects `openai` but
     `openai_backend_from_config()` returns `None`, emit a process-once warning
     naming the selected credential env (`memory_embedding_api_key_env` or
     `OPENAI_API_KEY`) and the local fallback. Use a module-private
     `AtomicBool` compare-exchange guard so a query loop cannot spam logs.
   - WI-5 deliberately does **not** validate `memory_embedding_backend` again.
     `embedding_backend.rs` continues to trust WI-4's load-time invariant and
     uses its existing `eq_ignore_ascii_case("openai")` check only for backend
     selection. A second validation choke point is prohibited.
2. **`crates/jcode-config-types/src/lib.rs` and
   `crates/jcode-base/src/config/default_file.rs`**
   - Revise `memory_embedding_dim` documentation from “vector-space metadata /
     sanity checks” to its exact contract: it requests server-side truncation
     for supported OpenAI v3 models, forms part of vector identity, and is
     required/recommended for custom model dimensions.
   - Revise remote-backend documentation to point to the explicit missing-key
     fallback warning. Keep the already-present separate
     `memory_embedding_api_key_env` documentation intact.
3. **Tests in `crates/jcode-base/src/embedding_backend.rs`**
   - Replace the old test that only verifies `dim()` with table tests asserting
     request JSON: v3+explicit 256 sends `dimensions: 256`; v3 without an
     override omits it; ada/custom omit it.
   - Table-test URL canonicalization: equivalent scheme/host case, default-port,
     and trailing-slash forms produce exactly one request URL/ID; non-default
     ports and distinct gateway paths produce different IDs; userinfo, query,
     fragment, unsupported scheme, missing host, and malformed URLs fail remote
     construction and select the explicit local fallback.
   - Assert model IDs differ for a different normalized endpoint and for
     dimension 256 versus 1536, while local remains distinct. Assert a custom
     `bge-m3` with explicit 1024 is tagged `dim=1024`, not `1536`; missing custom
     dimension refuses remote construction and emits the actionable declaration
     warning, rather than creating an unsafe `dim=unknown` identity.
   - Feed mocked response payloads through the pure response validator: a known
     declared dimension accepts vectors of exactly that length, rejects a first
     vector of another length, and rejects a later heterogeneous batch vector.
     This test is the proof that no vector survives under a false declared ID.
   - Test that a missing selected remote credential returns local and flips the
     one-time warning state only once (provide a `#[cfg(test)]` reset helper).

#### Before/after sketch

```rust
// Before: same tag across services/dimensions and no truncation request.
model_id: format!("openai:{model}"),
json!({ "model": model, "input": inputs })

// After: only a known vector space is constructed and persisted.
let dim = resolve_embedding_dimension(model, configured_dim)?;
model_id: embedding_model_id(&canonical_base_url, &model, dim),
json!({ "model": model, "input": inputs, "dimensions": requested_dim })
// `dimensions` is conditionally inserted only for models that support it.
```

#### Why this is the right seam

`OpenAiEmbeddingBackend::new` is where endpoint, model, and configured
dimension first coexist, and `embed_inputs` is the one wire-format construction
point. Completing identity there makes the already-correct memory equality
gate safe without duplicating gate logic. The runtime selector is the one place
that can distinguish an intentional remote choice with a missing credential
from the normal local default.

#### Blast radius

The active backend is used by stored-passage and query embedding calls, while
`memory.rs` consumes only the opaque ID equality contract. Existing remote
memory rows intentionally become dense-ineligible when their endpoint or
configured dimension differs. Preserve their lexical/BM25 path. This work must
not land before the separate embedding credential-env baseline because that
baseline makes non-OpenAI endpoint use reachable.

#### Validation

```bash
nix develop --command cargo test -p jcode-base --lib embedding_backend
nix develop --command cargo test -p jcode-base --lib memory
nix develop --command cargo test -p jcode-base --lib config_tests
nix develop --command cargo check -p jcode-base
```

If CI can securely reach OpenAI, add a live smoke test, or a VCR-style recorded
fixture captured from it, for a dimensions-capable `text-embedding-3-small` and
non-capable `text-embedding-ada-002`. It must verify the request/response
contract behind the include-versus-omit `dimensions` branch. The ada-002/v3
capability distinction is community knowledge in this audit, not verified
against a live API. If network validation is unavailable, record that as an
accepted residual risk rather than treating the unit table as proof of the live
contract.

#### Risk and rollback

The primary behavioral change is that historical remote vectors may cease to
participate in dense ranking after endpoint/dimension identity changes. That is
safer than comparing incompatible vectors and is already the documented gate
policy. BM25 remains available, and the existing `skipped_model_mismatch` info
log makes the change visible at retrieval. The capability-table behavior retains
the explicit residual risk described in Validation until it is checked against a
live or recorded OpenAI response. Roll back one commit to restore legacy tags if
needed, without deleting memories or vectors.

## Integration and commit sequence

1. **Baseline commit:** commit only the already-present
   `memory_embedding_api_key_env` change after its focused tests. Do not fold
   audit fixes into it.
2. **WI-0:** repair the three shared `Arc` slots and land the `StubExternalRuntime`
   isolation tests first. It is the hard merge gate for every model-setting fork
   consumer.
3. **WI-1:** land the alias schema and behaviorally complete typed credential-source
   slice, including OpenRouter runtime, usage/auth/lifecycle, doctor/live, app-core,
   and TUI consumers. The catalog-pair guard must pass before removing Z.AI's leaf
   special case.
4. **WI-2A:** add provider-core `ActiveProvider` canonical APIs and explicitly
   builtin-only helpers while compatibility forwarding wrappers remain.
5. **WI-2B:** add base `ResolvedModelSpec` and migrate all base callers.
6. **WI-2C:** migrate app-core, OpenRouter runtime, and all six TUI surfaces; run
   the classifier/parser workspace search, then remove wrappers. Each A/B/C
   checkpoint must compile independently, though release history may squash them.
7. **WI-3:** only after WI-0 and WI-2. Implement the stored provider fork with the
   explicit-prefix routing table and the existing sidecar stub seam.
8. **WI-4:** land parser observability and the alias-preserving canonicalization
   table together. Keep runtime parsers defensive until table compatibility tests
   establish equivalent behavior.
9. **WI-5:** after WI-4 and the baseline, with strict known-dimension construction
   and URL canonicalization. Unknown custom dimensions deliberately fall back to
   local rather than create a false vector identity.
10. **Final validation:** run all affected-crate tests/checks and the two mechanical
    guards: no catalog pair-loader erosion and no former general classifier/parser
    caller outside its documented core-only exception.

```bash
nix develop --command cargo check -p jcode-provider-metadata -p jcode-provider-env -p jcode-provider-core -p jcode-config-types -p jcode-base -p jcode-app-core -p jcode-provider-openrouter-runtime -p jcode-provider-doctor -p jcode-tui
nix develop --command cargo test -p jcode-provider-env -p jcode-provider-core -p jcode-config-types
nix develop --command cargo test -p jcode-base --lib
```

## Explicitly deferred non-findings / non-collapses

- **A4 structural macro/table rewrite:** deferred as described above. The
  existing coverage test verifies the audited set is in sync. WI-4 adds
  duplicates/order and new-key reload coverage, which is proportional to the
  verified risk.
- **B5 full enum removal:** rejected for now. The source verifies the generic
  path, but also verifies OAuth-specific sidecar behavior that a generic
  provider path has not been proven to reproduce.
- **D5 catalog-derived dimensions for arbitrary gateways:** rejected because
  source has no such catalog. WI-5 uses authoritative OpenAI protocol metadata
  and requires explicit custom dimension declaration instead of a false
  default.
