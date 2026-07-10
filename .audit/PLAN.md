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
- **Use warn-and-normalize validation, not `deny_unknown_fields` or lenient
  enum deserialization.** `Config::load()` deliberately survives stale config,
  and `UpdateChannel` is a special reload-handoff compatibility exception.
  Silently defaulting all bad strings would reproduce the defect. A single
  post-load/post-env `Config::validate()` can warn and restore a documented
  safe value.
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
   - In `MultiProvider::fork()` (current `2538-2623`), replace the bare
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
     `1006-1013`), as do `AntigravityProvider::fork`
     (`jcode-provider-antigravity-runtime/src/lib.rs` `736-743`) and
     `CopilotApiProvider::fork` (`jcode-provider-copilot-runtime/src/lib.rs`
     `1021-1037`).
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

### WI-1 — Catalog-owned provider credentials and aliases

- **Findings addressed:** A1, A2, A3.
- **Dependencies:** None. It may land before or after the already-present
  embedding `api_key_env` baseline, but does not modify that feature.

#### Exact changes

1. **`crates/jcode-provider-metadata/src/lib.rs`**
   - Add `legacy_api_key_envs: &'static [&'static str]` to
     `OpenAiCompatibleProfile`. This is deliberately a slice, not an
     `Option`, so future aliases do not need another schema change.
   - Add the corresponding owned field to `ResolvedOpenAiCompatibleProfile`.
     Preserve it when resolving a static profile. Runtime OpenAI-compatible
     overrides may change the primary key name but do not synthesize aliases.
2. **`crates/jcode-provider-metadata/src/catalog.rs`**
   - Populate `legacy_api_key_envs: &[]` on every profile.
   - Set `ZAI_PROFILE.legacy_api_key_envs` to `&["ZAI_API_KEY"]` while retaining
     primary `ZHIPU_API_KEY`.
   - Change `MINIMAX_PROFILE.api_key_env` from `OPENAI_API_KEY` to
     `MINIMAX_API_KEY`, matching `minimax.env` and the external-import map.
   - Keep the OpenAI profile as the single primary owner of
     `OPENAI_API_KEY`.
3. **`crates/jcode-provider-env/src/lib.rs`**
   - Extract the current environment/file scanning into a private helper that
     accepts an ordered list of candidate variable names and applies the
     existing `clean_loaded_value` sanitation to each value.
   - Keep `load_api_key_from_env_or_config(primary, file)` as the
     primary-only compatibility API for non-catalog callers.
   - Add `load_api_key_from_openai_compatible_profile(profile)` (and a resolved
     profile equivalent if the resolved type is needed by the existing base
     call sites). It tries the primary key first, then catalog aliases, in both
     process environment and the same profile env file.
   - Delete the `if env_key == "ZHIPU_API_KEY"` branch at current lines
     120-135 and delete the now-provider-specific test. Do **not** wire the
     unused fallback-resolver registry into this behavior.
4. **`crates/jcode-base/src/provider_catalog.rs`**
   - Re-export the profile-aware loader.
   - Change openai-compatible credential consumers, especially
     `openai_compatible_profile_is_configured`, `openrouter_like_api_key_sources`,
     and profile probing, to pass the catalog/resolved profile rather than only
     `(api_key_env, env_file)`. This preserves aliases wherever a profile is
     checked, not just in one auth screen.
5. **OpenAI literal consumers in `jcode-base`**
   - In `src/auth/codex.rs::load_env_api_key`, replace the raw
     `std::env::var("OPENAI_API_KEY").trim()` branch and the literal fallback
     with one profile-aware call using `OPENAI_COMPAT_PROFILE`. This makes its
     sanitation exactly match the common loader.
   - In `src/auth/mod.rs::openai_api_key_configured` and
     `src/provider/pricing.rs::openai_effective_auth_mode`, replace the
     `OPENAI_API_KEY`/`openai.env` literals with the same catalog profile and
     loader. Keep pricing's OAuth-vs-key decision unchanged.
   - Replace remaining OpenAI-primary literal *reads* in the audited paths
     (`usage/api_keys.rs` via its profile loop is already correct) with the
     catalog profile. Test scaffolding may retain literal env names when it is
     deliberately setting/restoring the public environment contract.
6. **Tests**
   - Add a metadata test that primary credential env names are unique among
     `requires_api_key` profiles, specifically asserting MiniMax is
     `MINIMAX_API_KEY` and OpenAI remains `OPENAI_API_KEY`.
   - Replace the old Z.AI test with a profile-loader matrix: primary env wins
     over alias, alias env works, and alias in `zai.env` works. Assert a
     MiniMax key configures MiniMax without an OpenAI key and vice versa.
   - Add a Codex loader regression with leading BOM/NBSP and trailing Unicode
     whitespace, asserting the result matches
     `sanitize_secret_value`/the common profile loader.

#### Before/after sketch

```rust
// Before: metadata has no place for the alias.
api_key_env: "ZHIPU_API_KEY",
// provider-env special-cases one vendor:
if env_key == "ZHIPU_API_KEY" { /* read ZAI_API_KEY */ }

// After: catalog owns the complete credential declaration.
api_key_env: "ZHIPU_API_KEY",
legacy_api_key_envs: &["ZAI_API_KEY"],
// generic ordered loader has no vendor literals:
load_api_key_from_openai_compatible_profile(ZAI_PROFILE)
```

#### Why this is the right seam

`OpenAiCompatibleProfile` is already the shared source for profile id, base
URL, key name, file, setup URL, and configured-state checks. Making it own
aliases eliminates the only provider-specific branch in the leaf crate and
prevents a profile's import, auth, usage, and readiness surfaces from naming
different credentials. MiniMax becomes independent of OpenAI at the metadata
source rather than being patched in every consumer.

#### Blast radius

All profile construction sites must receive the new field. Move the profile
credential checks in `provider_catalog.rs`, `provider/openrouter.rs`,
`usage/api_keys.rs`, `auth/mod.rs`, external auth validation, and the provider
catalog tests together. Do not change unrelated direct providers such as Azure
until they have a catalog profile.

#### Validation

```bash
nix develop --command cargo test -p jcode-provider-metadata
nix develop --command cargo test -p jcode-provider-env
nix develop --command cargo test -p jcode-base --lib provider_catalog_tests
nix develop --command cargo test -p jcode-base --lib codex
nix develop --command cargo check -p jcode-base
```

#### Risk and rollback

The main risk is changing legacy-key precedence. Preserve the existing primary
then alias order, and keep the public primary env name unchanged for every
profile except the verified MiniMax defect. Roll back this single commit to
restore old lookup behavior. No credential value is migrated or written.
Any user with only `OPENAI_API_KEY` will see MiniMax flip from “configured” to
“not configured.” This is the intended correction of accidental credential
coupling, but warrants a user-facing release-note callout.

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
   - Replace the 13-arm `explicit_model_provider_prefix` chain with parsing of
     the prefix before `:` followed by `AuthRoute::parse` or
     `ActiveProvider::from_key_or_alias`. Retain exact auth-route spelling in
     the result, because OAuth/API prefix intent must survive.
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
     provider. `openai-api:` and `anthropic-api:` are permanently reserved
     dual-auth spellings: they must win before the same-named catalog profile
     ids and can never select the OpenAI-compatible runtime by prefix. This
     preserves the native HTTP-client, auth, pricing, and billing path.
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
4. **Move every former runtime classifier caller in the same commit.**
   - Base: `provider/catalog_routes.rs`, `pricing.rs`, `route_builders.rs`,
     `provider/mod.rs`, capability lookup, selection, and all public
     `provider_for_model[_with_hint]` exports.
   - App-core: retain the existing override-precedes-model policy in
     `provider_key_for_spawn_model` and `provider_key_for_launch_model` in
     `src/server/comm_session.rs` and `src/server/jade_relay.rs`. Only replace
     each helper's inner `split_once(':')` classification block with
     `resolve_model_spec(model, cfg).provider_key`; do not move spawn/launch
     override policy into `jcode-base` or collapse these helpers into a base
     provider-key API.
   - TUI: `jcode-tui/src/tui/app/tui_state.rs` header/effort classification
     must use the context-aware resolver, so a selected named profile is not
     displayed as unknown.
   - Preserve core's isolated builtin helper only where a no-`Config` core API
     genuinely needs a static classification. It must no longer be named or
     documented as the general resolver.
5. **Tests**
   - Table-test one resolver result for bare Claude/OpenAI/Gemini, Bedrock,
     Cursor, OpenRouter `@`, each dual-auth prefix, a catalog profile prefix,
     a named `[providers.omlx]` profile prefix, and an unknown prefix.
     Add explicit negative cases for `openai-api:gpt-5.5` and
     `anthropic-api:<model>`: each must resolve as the native pinned dual-auth
     route and prove the same-named catalog-profile lookup is unreachable via
     prefix parsing.
   - Assert both spawn and Jade relay return the same provider key for every
     explicit/named case. Put shared parser tests in base and focused wrapper
     tests in app-core.
   - Add regression coverage that pricing, catalog route classification, and
     TUI header resolution see the named `omlx` key, not `None`.

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

This is intentionally a broad but atomic rename/move. Do not merge the parser
without moving all listed callers, or named profiles will remain write-only.
The public base exports, app-core server helpers, and TUI consumers must
compile together. Saved-session vocabulary and `AuthRoute` spellings are
compatibility contracts and must retain their existing serialized values.

#### Validation

```bash
nix develop --command cargo test -p jcode-provider-core
nix develop --command cargo test -p jcode-base --lib model_resolution
nix develop --command cargo test -p jcode-app-core --lib comm_session
nix develop --command cargo test -p jcode-app-core --lib jade_relay
nix develop --command cargo check -p jcode-base -p jcode-app-core -p jcode-tui
```

#### Risk and rollback

This is the largest semantic change: provider/auth selection can be altered by
prefix precedence. The resolver table tests, including the permanent
`openai-api:`/`anthropic-api:` reservation negatives, are the rollback safety
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
   - Add a default `Provider::fork_with_model_spec(&self, model_spec: &str) ->
     Result<Arc<dyn Provider>>` method that returns an actionable unsupported
     error by default. Existing concrete mock/leaf providers need no changes
     because the default is safe.
2. **`crates/jcode-base/src/provider/mod.rs`**
   - Build on WI-0's independent `MultiProvider::fork` and expose a
     `fork_with_model_spec` override that runs `set_model(model_spec)` on that
     isolated instance before erasing it to `Arc<dyn Provider>`.
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
   - Change `SidecarBackend::Provider` to hold the forked `Arc<dyn Provider>`
     (or add an equivalent field on `Sidecar`), rather than re-fetching and
     forking the active provider during every completion.
   - In `Sidecar::with_configured_model`, call WI-2's
     `resolve_current_model_spec`. For a configured model:
     - retain the dedicated OpenAI/Claude fast paths only when the resolved
       native provider and its explicit route are compatible with those clients;
     - otherwise call `active_provider_fork_with_model_spec` with the original
       model specification, including named/profile prefix, and store it in
       `SidecarBackend::Provider`;
     - if the active provider cannot fork that model, log an explicit error
       containing the model and resolver result, then use the existing
       auto-selection policy. Do not silently say the model is unsupported.
   - Make `complete_via_provider` use the stored fork. Today it ignores
     `self.model` and re-forks the active provider, so merely selecting
     `SidecarBackend::Provider` would still fail to honor the configured model.
   - Retain `SidecarBackend::{OpenAI, Claude, Provider}`. The specialized OAuth
     request and fallback ladder are real behavior, so enum removal is not
     justified by the audited evidence.
   - A runtime failure from the forked provider's `complete_simple` after it
     was constructed is **not** retried through the OpenAI-specific fallback
     ladder. It surfaces as a plain error to the memory caller, matching today's
     generic `SidecarBackend::Provider` behavior. This item adds no fallback
     logic for that path.
4. **Tests**
   - Add a small test `Provider` implementation whose `fork_with_model_spec`
     records the requested spec. Assert `omlx:Qwen3.6-MoE` is preserved and the
     provider path is selected rather than auto-selecting OAuth.
   - Assert an `openai-api:gpt-5.5` and a catalog profile prefix arrive at the
     selected fork unchanged, while the default no-override OAuth preference
     remains unchanged.
   - Assert fork failure produces the explicit fallback diagnostic and never
     mutates the registered live provider model.
   - Assert a `set_model` failure from a model-configured fork follows that same
     explicit fallback path. The test must not treat Copilot's transient
     `try_write` error as an impossible condition.

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

### WI-4 — Observable configuration parsing and one validation pass

- **Findings addressed:** C1, C2, C3, C4, C5, D4, A4 (guarded, not a risky
  macro collapse).
- **Dependencies:** None. This may land in parallel with WI-1/WI-2.

#### Exact changes

1. **`crates/jcode-base/Cargo.toml` and `Cargo.lock`**
   - Add the small `serde_ignored` dependency. It reports Serde-ignored fields
     while preserving the repository's permissive `#[serde(default)]` parsing.
2. **`crates/jcode-base/src/config/config_file.rs`**
   - Replace direct `toml::from_str::<Config>(&content)` in
     `load_from_file_strict` with a private parser helper backed by
     `serde_ignored::deserialize(toml::Deserializer::new(&content), ...)`.
   - Collect every ignored path, sort/deduplicate it, and issue one
     `logging::warn` per path in the form
     `Unknown config key 'display.redraw_fpss' ignored` **only after
     `serde_ignored::deserialize` succeeds**. On malformed TOML, discard the
     partial collection and return the original parse error unchanged, without
     unknown-key warnings. This preserves the present error contract in
     `config_file.rs::load_from_file_strict` (40-54) while making successful,
     permissive parsing observable. The composition is sound because `Config`
     is `Deserialize + Default` with `#[serde(default)]` (`config.rs:436-439`),
     nested fields use defaults, and this config shape has no `serde(flatten)`
     fields.
   - Invoke `config.validate()` after `display.apply_legacy_compat()` and, in
     both `Config::load` and `Config::load_strict`, invoke it again after
     `apply_env_overrides()`. The latter is required because env precedence can
     introduce an invalid value after file validation.
3. **`crates/jcode-base/src/config.rs` — a single canonicalization policy**
   - Add `pub(crate) fn Config::validate(&mut self)` plus small local
     normalize-and-warn helpers. It must trim and ASCII-lowercase the values
     below, emit field, raw value, accepted spellings, canonical result, and
     fallback, and write the canonical string (or `None`) back to config.
   - **This table is the sole policy table.** It is deliberately a transcription
     of each current consumer parser, not a documentation-only whitelist. The
     source-of-truth evidence is: tools' branch ordering in
     `jcode-base/src/config.rs::ToolsConfig::base_allowed_tools` (627-675),
     OpenAI transport (openai-runtime `OpenAITransportMode::from_config`,
     129-145), reasoning (727-745), and service tier (792-806), Anthropic
     reasoning (553-573) and service tier (667-679), and Copilot config-to-env
     propagation (base `env_overrides.rs`, 715-727) followed by its runtime
     reader (copilot-runtime `env_premium_mode`, 150-155). Centralization must
     mirror, never broaden or narrow, these existing accepted spellings.

     | Config field | All accepted input spellings | Canonical stored value | Invalid fallback | Evidence / preserved behavior |
     |---|---|---|---|---|
     | `agents.memory_embedding_backend` | `local`, `openai` | same lowercase | `local` | Default is exactly `local` (`jcode-config-types/src/lib.rs:555-557`); selector checks `openai` in `embedding_backend.rs:292-299`. |
     | `tools.profile` | empty, `full`, `acp`, `minimal`, `lite`, `small`, `none`, `off`, `disabled` | empty/full -> empty (full/default set), `acp` -> `acp`, `small` -> `minimal`, `minimal`/`lite` unchanged, `none`/`off`/`disabled` -> `none` | empty | `none|off|disabled` produces `Some(empty set)` at `config.rs:635-637`, while the final else is `None` full/default at 673-675. Thus **off/disabled must never normalize to empty**. |
     | `acp.profile` | `standard`, `extended`, `full` | same | `standard` | Existing configuration default/test contract is `standard` (`config_tests.rs::acp_config_defaults_to_standard_profile_and_acp_tools`, 331-337); this is a new validated config domain, not a runtime alias parser. |
     | `display.performance` | empty, `auto`, `full`, `reduced`, `minimal` | empty/`auto` -> empty; others unchanged | empty | `app-core/src/perf.rs::PerformanceTier::detect` (309-337) consumes these display strings; empty is the auto/default branch. |
     | `provider.openai_transport` | empty, `auto`, `websocket`, `ws`, `wss`, `https`, `http`, `sse` | empty/`auto` -> `auto`; `ws`/`wss` -> `websocket`; `http`/`sse` -> `https` | `auto` | Exact OpenAI runtime parser set at 129-145. |
     | `provider.openai_reasoning_effort` | empty, `none`, `low`, `medium`, `high`, `xhigh`, `swarm`, `swarm-deep` | empty -> `None`; others unchanged | `xhigh` | Exact parser and current unsupported fallback at openai-runtime 727-745. |
     | `provider.openai_service_tier` | empty, `fast`, `priority`, `flex`, `default`, `auto`, `none`, `off` | empty/`default`/`auto`/`none`/`off` -> `None`; `fast`/`priority` -> `priority`; `flex` unchanged | `None` | Exact parser at openai-runtime 792-806. |
     | `provider.anthropic_reasoning_effort` | empty, `default`, `auto`, `off`, `disabled`, `none`, `low`, `medium`, `high`, `xhigh`, `max`, `swarm`, `swarm-deep` | empty/`default`/`auto` -> `None`; `off`/`disabled` -> `none`; remaining levels unchanged | `max` | Exact parser and fallback at anthropic-runtime 553-573. |
     | `provider.anthropic_service_tier` | empty, `default`, `off`, `standard`, `standard_only`, `priority`, `auto` | empty/`default` -> `None`; `off`/`standard`/`standard_only` -> `standard_only`; `priority`/`auto` -> `auto` | `None` | Exact parser at anthropic-runtime 667-679. |
     | `provider.copilot_premium` | `zero`, `0`, `one`, `1` | `zero`/`0` -> `0`; `one`/`1` -> `1` | `None` | Base mapping only exports `0|1` (env_overrides 715-727) and runtime recognizes only `0|1` (copilot-runtime 150-155). |
     | `provider.openai_native_compaction_mode` | `auto`, `explicit`, `off` | same | `auto` | Default is `auto` in `ProviderConfig::default` (`jcode-config-types/src/lib.rs:1245-1266`); OpenAI construction consumes this configured field (`openai-runtime::new`, 543-632). |

   - Retain downstream runtime parsers as **defensive consumers** for direct
     environment/programmatic construction, but refactor them to delegate to
     the shared canonical helper where crate layering permits. They must accept
     the same table during transition and must not own a divergent second policy.
     `Config::validate()` is authoritative for file/config-derived values.
   - Keep these fields serialized as strings. A serialized enum migration is a
     separate compatibility change and would either reject old files or hide
     typos before this observable validation point.
4. **`crates/jcode-base/src/config/env_overrides.rs`**
   - It may parse primitive values, but must not make its own domain decision.
     Run `Config::validate()` immediately after all overrides **and before**
     any config-to-env propagation. In particular `copilot_premium` may export
     only the canonical `0`/`1`; invalid file/env values become `None` and are
     never written to `JCODE_COPILOT_PREMIUM`.
5. **`crates/jcode-config-types/src/lib.rs`**
   - Correct `DisplayConfig::redraw_fps` rustdoc from default 30 to default 60.
   - Update embedding backend documentation: unknown values are warned and
     normalized, and a selected remote backend with missing credentials is
     reported by WI-5 rather than silently described as normal behavior.
6. **`crates/jcode-base/src/config_tests.rs`**
   - Factor the parser/validator enough to test unknown TOML paths without
     requiring a user home. Add tests for a top-level and nested unknown key,
     a known key producing no unknown paths, and malformed TOML returning its
     parse error with **no** unknown-key warnings.
   - Table-test every accepted alias in the canonicalization table from TOML
     and env, asserting its exact canonical output. In particular assert
     `tools.profile=off|disabled -> none` remains an empty allow-list and
     `small -> minimal`, not default/full tools; assert every OpenAI/Anthropic
     transport/reasoning/service-tier alias and both Copilot spellings. Also
     test each invalid fallback exactly as tabulated. These compatibility tests
     are a merge prerequisite before the runtime parser logic is centralized.
   - Retain `config_env_fingerprint_tracks_every_apply_env_override_var`.
     Extend it to assert `CONFIG_ENV_KEYS` has no duplicates, and add
     a targeted reload-fingerprint test for `JCODE_MEMORY_EMBEDDING_API_KEY_ENV`.
     This is the proportionate protection for A4; do not create a 130-entry
     callback macro merely to remove a list that also has non-override members.

#### Before/after sketch

```rust
// Before: unknown TOML fields vanish; file and env strings take different paths.
let mut config = toml::from_str::<Self>(&content)?;
config.apply_env_overrides();

// After: permissive parsing is observable and policy is centralized.
let (mut config, unknown_paths) = parse_config_toml(&content)?;
warn_unknown_paths(unknown_paths);
config.validate();
config.apply_env_overrides();
config.validate();
```

#### Why this is the right seam

`Config::load` is the only point every file configuration crosses, and
`apply_env_overrides` is the only precedence mutation point. Instrumenting
Serde's ignored fields there catches all nested unknown keys without a manually
maintained schema walker. Post-merge validation makes TOML and environment
values obey one policy, rather than attempting N partial deserializers or
making config loading brittle.

#### Blast radius

All configuration entry points (`load`, `load_strict`, direct parsing helper
used by tests, and config cache reload) must call the same parser/validator.
The impacted user-facing values are provider request settings, ACP/tools,
display policy, and the embedding selector. Preserve the existing public
serialized strings and defaults.

#### Validation

```bash
nix develop --command cargo test -p jcode-config-types
nix develop --command cargo test -p jcode-base --lib config_tests
nix develop --command cargo test -p jcode-base --lib default_file
nix develop --command cargo check -p jcode-base
```

#### Risk and rollback

Warnings can be noisy on legacy configs, and normalization changes a formerly
ignored bad value into an explicit default. This is intentional and documented.
Avoid hard failure, so rollback is simply reverting the parser/validation
commit. The config file is never rewritten during load.

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
   - Resolve an effective dimension as: explicit
     `agents.memory_embedding_dim`, otherwise a known-native dimension,
     otherwise emit one clear configuration warning and use a documented
     `unknown` identity component. Do not call an arbitrary gateway model
     1536-dimensional by fiat. The configuration template must tell custom
     endpoints such as bge-m3 to set `memory_embedding_dim = 1024`.
   - Extract request JSON construction to a pure helper. Include
     `"dimensions": configured_dim` only when the user explicitly supplied a
     dimension **and** the selected model is in the known dimensions-capable
     table. For a requested dimension on a known non-capable/unknown model,
     omit the unsupported field and warn once explaining that the value is an
     identity/sanity declaration, not a server truncation request.
   - Make the persisted `OpenAiEmbeddingBackend::model_id` include the normalized
     full base URL (scheme, host, port, and path), bare model, and effective
     dimension identity. Example:
     `openai:https://api.openai.com/v1|text-embedding-3-small|dim=1536`.
     Use the full normalized endpoint, not merely hostname, because two
     gateway paths at one host can be different vector services. Never include
     a key in this identifier.
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
   - Assert model IDs differ for a different normalized endpoint and for
     dimension 256 versus 1536, while local remains distinct.
   - Assert a custom `bge-m3` with explicit 1024 is tagged `dim=1024`, not
     `1536`; test missing custom dimension uses the `unknown` identity path.
   - Test that a missing selected remote credential returns local and flips the
     one-time warning state only once (provide a `#[cfg(test)]` reset helper).

#### Before/after sketch

```rust
// Before: same tag across services/dimensions and no truncation request.
model_id: format!("openai:{model}"),
json!({ "model": model, "input": inputs })

// After: vector space is identified by endpoint, model, and dimension.
model_id: embedding_model_id(&base_url, &model, effective_dim),
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

1. Commit the already-complete `memory_embedding_api_key_env` baseline first
   (or keep it in the WI-5 commit). Do not duplicate it.
2. Commit **WI-0** independently. Its Gemini/Antigravity/Copilot fork-isolation
   regression must pass before any consumer can call `set_model` on a fork.
3. Commit **WI-1** independently.
4. Commit **WI-2** independently.
5. Commit **WI-3** only after both WI-0 and WI-2.
6. Commit **WI-4** independently. It may be developed in parallel, but merge
   before WI-5.
7. Commit **WI-5** after the credential baseline and WI-4.
8. Final workspace validation:

```bash
nix develop --command cargo check -p jcode-provider-metadata -p jcode-provider-env -p jcode-provider-core -p jcode-config-types -p jcode-base -p jcode-app-core -p jcode-tui
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
