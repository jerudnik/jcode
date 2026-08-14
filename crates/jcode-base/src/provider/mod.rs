mod accessors;
mod account_failover;
pub mod activation;
mod active_handle;
pub mod anthropic;
pub mod antigravity;
pub mod bedrock;
mod catalog_routes;
pub mod claude;
pub mod copilot;
pub mod cursor;
mod dispatch;
pub mod external;
mod failover;
mod failover_runtime;
mod fingerprint;
pub mod gemini;
mod image_clamp;
pub mod jcode;
mod model_switching;
pub mod models;
mod multi_provider;
pub mod openai;
pub mod openai_request;
pub mod openrouter;
pub mod pricing;
mod profile_routes;
mod registry;
mod route_builders;
mod routes_memo;
mod routing;
mod selection;
mod startup;
mod state;

use routes_memo::RoutesMemoEntry;

use crate::auth;
use crate::message::{Message, ToolDefinition};
use account_failover::{
    account_usage_probe, active_account_label_for_provider, maybe_annotate_limit_summary,
    same_provider_account_candidates, same_provider_account_failover_enabled,
    set_account_override_for_provider,
};
use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
#[cfg(test)]
use jcode_provider_core::FailoverDecision;
use registry::ProviderRegistry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

pub use catalog_routes::{
    append_simplified_anthropic_model_routes, remote_current_openai_compatible_route_for_model,
    remote_model_is_server_copilot_only, remote_model_routes_fallback,
    remote_model_routes_lightweight_fallback, remote_model_should_offer_copilot_route,
    remote_openai_compatible_route_for_model, simplified_model_routes_for_picker,
};
pub use jcode_provider_core::attempt_tracker;
pub use jcode_provider_core::cli_provider_arg_for_session_key;
pub use jcode_provider_core::{
    ALL_CLAUDE_MODELS, ALL_OPENAI_MODELS, CHEAPNESS_REFERENCE_INPUT_TOKENS,
    CHEAPNESS_REFERENCE_OUTPUT_TOKENS, CredentialMode, DEFAULT_CONTEXT_LIMIT, EventStream,
    JCODE_USER_AGENT, ModelCapabilities, ModelCatalogRefreshSummary, ModelRoute,
    ModelRouteApiMethod, NativeCompactionResult, NativeToolResult, NativeToolResultSender,
    PremiumMode, Provider, RouteBillingKind, RouteCheapnessEstimate, RouteCostConfidence,
    RouteCostSource, RouteSelection, RuntimeKey, dedupe_model_routes,
    explicit_model_provider_prefix, fresh_transport_client, model_name_for_provider,
    normalize_copilot_model_name, provider_from_model_key, shared_http_client,
    summarize_model_catalog_refresh,
};
pub use jcode_provider_core::{
    FallbackPickOptions, error_looks_like_credential_failure, model_route_provider_labels_match,
    normalize_model_route_provider_label, pick_next_fallback_route,
    pick_next_fallback_route_with_options,
};
pub use jcode_provider_core::{ProviderFailoverPrompt, parse_failover_prompt_message};
pub use route_builders::{
    build_anthropic_oauth_route, build_copilot_route, build_openai_api_key_route,
    build_openai_oauth_route, build_openrouter_auto_route, build_openrouter_endpoint_route,
    build_openrouter_fallback_provider_route, is_listable_model_name,
    listable_model_names_from_routes, openrouter_catalog_model_id,
};
pub(crate) use routing::{
    anthropic_api_key_route_availability, anthropic_oauth_route_availability,
};

pub fn set_model_with_auth_refresh(provider: &dyn Provider, model: &str) -> Result<()> {
    match provider.set_model(model) {
        Ok(()) => Ok(()),
        Err(first_err) => {
            let first_message = first_err.to_string();
            crate::logging::auth_event(
                "auth_changed_retry_after_set_model_failure",
                provider.name(),
                &[("reason", first_message.as_str())],
            );
            // Use the preserve-current-provider variant: this is a retry for an
            // already-open session, so refreshing auth from disk must NOT swap a
            // user-defined named OpenAI-compatible profile slot for a generic
            // OpenRouter runtime (which would lose `profile_id` and re-introduce
            // the `<profile>:<model>` prefix on the wire). See #408.
            provider.on_auth_changed_preserve_current_provider();
            provider.set_model(model).map_err(|second_err| {
                anyhow::anyhow!(
                    "{} (retried after reloading auth from disk: {})",
                    first_message,
                    second_err
                )
            })
        }
    }
}

use self::dispatch::CompletionMode;
pub use self::models::{
    AccountModelAvailability, AccountModelAvailabilityState, AnthropicModelCatalog,
    ModelCatalogHttpStatus, OpenAIModelCatalog, ResolvedModelSpec,
    begin_anthropic_model_catalog_refresh, begin_openai_model_catalog_refresh,
    cached_anthropic_model_ids, cached_openai_model_ids,
    clear_all_model_unavailability_for_account, clear_all_provider_unavailability_for_account,
    clear_model_unavailable_for_account, clear_provider_unavailable_for_account,
    context_limit_for_model, context_limit_for_model_with_provider, fetch_anthropic_model_catalog,
    fetch_anthropic_model_catalog_oauth, fetch_openai_api_key_model_catalog,
    fetch_openai_context_limits, fetch_openai_model_catalog,
    finish_anthropic_model_catalog_refresh_for_scope, finish_openai_model_catalog_refresh,
    format_account_model_availability_detail, get_best_available_openai_model,
    is_model_available_for_account, known_anthropic_model_ids, known_openai_model_ids,
    model_availability_for_account, model_unavailability_detail_for_account,
    note_openai_model_catalog_refresh_attempt, persist_anthropic_model_catalog,
    persist_openai_model_catalog, populate_account_models, populate_anthropic_models,
    populate_context_limits, populate_context_limits_from_config,
    populate_context_limits_from_config_value, provider_unavailability_detail_for_account,
    record_model_unavailable_for_account, record_provider_unavailable_for_account,
    refresh_openai_model_catalog_in_background, resolve_current_model_spec,
    resolve_model_capabilities, resolve_model_spec, should_refresh_anthropic_model_catalog,
    should_refresh_openai_model_catalog,
};
pub use self::selection::DefaultModelSelection;
use self::selection::{ActiveProvider, ProviderAvailability};
use self::state::ProviderState;
pub use self::state::{ProviderModelSelectionSource, ProviderRuntimeState, ProviderStateEvent};
pub(crate) use active_handle::active_provider_generation;
pub use active_handle::{
    active_provider_fork, active_provider_fork_with_model_spec, set_active_provider,
    stores_reasoning_content_for_context, stream_idle_timeout,
};
pub(super) use profile_routes::{
    configured_standard_openrouter_profile_routes, direct_openai_compatible_profile_routes,
    standard_openrouter_profile_configured,
};

pub(crate) const GROK_BUILD_PROFILE_ID: &str = "grok-build";
pub(crate) const KIMI_CODE_ACP_PROFILE_ID: &str = "kimi-code-acp";
pub(crate) const REASONIX_PROFILE_ID: &str = "reasonix";

/// MultiProvider wraps multiple providers and allows seamless model switching
pub struct MultiProvider {
    /// Claude Code CLI provider
    claude: RwLock<Option<Arc<dyn Provider>>>,
    /// Direct Anthropic API provider (no Python dependency)
    anthropic: RwLock<Option<Arc<dyn Provider>>>,
    openai: RwLock<Option<Arc<dyn Provider>>>,
    /// GitHub Copilot API provider (direct API, hot-swappable after login).
    /// Held as `dyn Provider`: the concrete runtime lives downstream in
    /// `jcode-provider-copilot-runtime` and is instantiated through
    /// `external::instantiate_external_provider`.
    copilot_api: RwLock<Option<Arc<dyn Provider>>>,
    /// Antigravity provider (direct HTTPS, hot-swappable after login). Held as
    /// `dyn Provider`: the concrete runtime lives downstream in
    /// `jcode-provider-antigravity-runtime` and is instantiated through
    /// `external::instantiate_external_provider`.
    antigravity: RwLock<Option<Arc<dyn Provider>>>,
    /// Gemini provider (hot-swappable after login). Held as `dyn Provider`:
    /// the concrete runtime lives downstream in `jcode-provider-gemini-runtime`
    /// and is instantiated through `external::instantiate_external_provider`.
    gemini: RwLock<Option<Arc<dyn Provider>>>,
    /// Cursor provider (native/direct API, hot-swappable after login). Held as
    /// `dyn Provider`: the concrete runtime lives downstream in
    /// `jcode-provider-cursor-runtime` and is instantiated through
    /// `external::instantiate_external_provider`.
    cursor: RwLock<Option<Arc<dyn Provider>>>,
    /// AWS Bedrock provider (native Converse/ConverseStream, IAM/SigV4)
    bedrock: RwLock<Option<Arc<bedrock::BedrockProvider>>>,
    /// OpenRouter API provider
    openrouter: RwLock<Option<Arc<dyn Provider>>>,
    /// Direct OpenAI-compatible runtimes keyed by profile id.
    ///
    /// These use the same wire protocol implementation as OpenRouter, but must
    /// not occupy the real OpenRouter slot. Keeping them separate prevents a
    /// compatible endpoint selection from corrupting later OpenRouter model
    /// switches, catalog display, or auth refresh handling.
    openai_compatible_profiles: RwLock<HashMap<String, Arc<dyn Provider>>>,
    active_openai_compatible_profile: RwLock<Option<String>>,
    active: RwLock<ActiveProvider>,
    /// Use Claude CLI instead of direct API (legacy mode)
    use_claude_cli: bool,
    /// Notifications generated during provider/account auto-selection.
    /// The TUI should drain and display these on session start.
    startup_notices: RwLock<Vec<String>>,
    /// Optional explicit provider lock set by CLI `--provider`.
    /// When present, cross-provider fallback is disabled.
    forced_provider: Option<ActiveProvider>,
    /// Short-TTL memo for the full route-catalog build.
    ///
    /// Building the catalog is expensive (per-route pricing lookups, endpoint
    /// cache reads, credential probes) and the shared server rebuilds it for
    /// every connection whenever a `ModelsUpdated` bus event fans out. During
    /// a burst of client spawns that multiplied into hundreds of builds within
    /// a couple of seconds, saturating every core. The memo collapses those
    /// into one build per TTL window; auth/model changes invalidate it
    /// explicitly so pickers never see stale routes after a switch.
    pub(super) routes_memo: Mutex<Option<RoutesMemoEntry>>,
}

impl Default for MultiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for MultiProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: &str,
        resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        self.complete_with_failover(
            messages,
            tools,
            CompletionMode::Unified { system },
            resume_session_id,
        )
        .await
    }

    /// Split system prompt completion - delegates to underlying provider for better caching
    async fn complete_split(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_static: &str,
        system_dynamic: &str,
        resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        self.complete_with_failover(
            messages,
            tools,
            CompletionMode::Split {
                system_static,
                system_dynamic,
            },
            resume_session_id,
        )
        .await
    }

    fn name(&self) -> &str {
        match self.active_provider() {
            ActiveProvider::Claude => "Claude",
            ActiveProvider::OpenAI => "OpenAI",
            ActiveProvider::Copilot => "Copilot",
            ActiveProvider::Antigravity => "Antigravity",
            ActiveProvider::Gemini => "Gemini",
            ActiveProvider::Cursor => "Cursor",
            ActiveProvider::Bedrock => "Bedrock",
            ActiveProvider::OpenRouter => "OpenRouter",
        }
    }

    fn provider_identity(&self) -> String {
        match self.active_provider() {
            ActiveProvider::Claude => self
                .anthropic_provider()
                .or_else(|| self.claude_provider())
                .map(|provider| provider.provider_identity()),
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|provider| provider.provider_identity()),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|provider| provider.provider_identity()),
            ActiveProvider::Antigravity => self
                .antigravity_provider()
                .map(|provider| provider.provider_identity()),
            ActiveProvider::Gemini => self
                .gemini_provider()
                .map(|provider| provider.provider_identity()),
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|provider| provider.provider_identity()),
            ActiveProvider::Bedrock => self
                .bedrock_provider()
                .map(|provider| provider.provider_identity()),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|provider| provider.provider_identity()),
        }
        .unwrap_or_else(|| self.name().trim().to_ascii_lowercase())
    }

    fn capabilities(&self) -> jcode_provider_core::ProviderCapabilities {
        match self.active_provider() {
            ActiveProvider::Claude => self
                .anthropic_provider()
                .or_else(|| self.claude_provider())
                .map(|provider| provider.capabilities()),
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|provider| provider.capabilities()),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|provider| provider.capabilities()),
            ActiveProvider::Antigravity => self
                .antigravity_provider()
                .map(|provider| provider.capabilities()),
            ActiveProvider::Gemini => self
                .gemini_provider()
                .map(|provider| provider.capabilities()),
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|provider| provider.capabilities()),
            ActiveProvider::Bedrock => self
                .bedrock_provider()
                .map(|provider| provider.capabilities()),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|provider| provider.capabilities()),
        }
        .unwrap_or_default()
    }

    fn display_name(&self) -> String {
        // The OpenRouter slot multiplexes the public aggregator and every
        // direct OpenAI-compatible profile (NVIDIA NIM, DeepSeek, ...). Ask the
        // active execution runtime for its own label so the UI reflects the
        // profile selected at runtime rather than the fixed "OpenRouter" name.
        if matches!(self.active_provider(), ActiveProvider::OpenRouter)
            && let Some(execution) = self.active_openrouter_execution_provider()
        {
            return execution.runtime_display_name();
        }
        self.name().to_string()
    }

    fn model(&self) -> String {
        match self.active_provider() {
            ActiveProvider::Claude => {
                // Prefer anthropic if available
                if let Some(anthropic) = self.anthropic_provider() {
                    anthropic.model()
                } else if let Some(claude) = self.claude_provider() {
                    claude.model()
                } else {
                    "claude-opus-4-5-20251101".to_string()
                }
            }
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|o| o.model())
                .unwrap_or_else(|| "gpt-5.5".to_string()),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|o| o.model())
                .unwrap_or_else(|| "claude-sonnet-4".to_string()),
            ActiveProvider::Antigravity => self
                .antigravity_provider()
                .map(|o| o.model())
                .unwrap_or_else(|| "default".to_string()),
            ActiveProvider::Gemini => self
                .gemini_provider()
                .map(|o| o.model())
                .unwrap_or_else(|| "gemini-2.5-pro".to_string()),
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|o| o.model())
                .unwrap_or_else(|| "composer-2.5".to_string()),
            ActiveProvider::Bedrock => self
                .bedrock_provider()
                .map(|o| o.model())
                .unwrap_or_else(|| "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string()),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|o| o.model())
                .unwrap_or_else(|| "anthropic/claude-sonnet-4".to_string()),
        }
    }

    fn active_resolved_credential(&self) -> Option<jcode_provider_core::ResolvedCredential> {
        use jcode_provider_core::ResolvedCredential;
        match self.active_provider() {
            ActiveProvider::Claude => {
                let anthropic = self.anthropic_provider()?;
                Some(match anthropic.credential_mode() {
                    anthropic::AnthropicCredentialMode::OAuth => ResolvedCredential::Oauth,
                    anthropic::AnthropicCredentialMode::ApiKey => ResolvedCredential::ApiKey,
                    // Auto prefers OAuth (Claude subscription) when available,
                    // otherwise falls back to the API key. Mirror that exactly.
                    anthropic::AnthropicCredentialMode::Auto => {
                        if crate::auth::claude::load_credentials().is_ok() {
                            ResolvedCredential::Oauth
                        } else {
                            ResolvedCredential::ApiKey
                        }
                    }
                })
            }
            ActiveProvider::OpenAI => {
                let openai = self.openai_provider()?;
                Some(match openai.credential_mode() {
                    openai::OpenAICredentialMode::OAuth => ResolvedCredential::Oauth,
                    openai::OpenAICredentialMode::ApiKey => ResolvedCredential::ApiKey,
                    // Auto resolves to OAuth first when available, otherwise API key.
                    openai::OpenAICredentialMode::Auto => {
                        if crate::auth::codex::load_oauth_credentials().is_ok() {
                            ResolvedCredential::Oauth
                        } else {
                            ResolvedCredential::ApiKey
                        }
                    }
                })
            }
            _ => None,
        }
    }

    fn credential_mode(&self) -> CredentialMode {
        let active = self
            .forced_provider
            .unwrap_or_else(|| self.active_provider());
        match active {
            ActiveProvider::Claude => self
                .anthropic_provider()
                .map(|provider| provider.credential_mode())
                .unwrap_or(CredentialMode::Auto),
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|provider| provider.credential_mode())
                .unwrap_or(CredentialMode::Auto),
            _ => CredentialMode::Auto,
        }
    }

    fn set_credential_mode(&self, mode: CredentialMode) -> Result<()> {
        let active = self
            .forced_provider
            .unwrap_or_else(|| self.active_provider());
        match active {
            ActiveProvider::Claude => self
                .anthropic_provider()
                .ok_or_else(|| anyhow!("Anthropic provider is not configured"))?
                .set_credential_mode(mode)?,
            ActiveProvider::OpenAI => self
                .openai_provider()
                .ok_or_else(|| anyhow!("OpenAI provider is not configured"))?
                .set_credential_mode(mode)?,
            _ if mode == CredentialMode::Auto => return Ok(()),
            _ => anyhow::bail!(
                "Provider {} does not support OAuth/API-key credential selection",
                Self::provider_label(active)
            ),
        }
        self.set_active_provider(active);
        Ok(())
    }

    fn active_explicit_credential(&self) -> Option<jcode_provider_core::ResolvedCredential> {
        use jcode_provider_core::ResolvedCredential;
        // Only report an *explicit* in-memory pin. Auto mode returns `None` so
        // callers fall back to their cheaper cached heuristic without forcing
        // a disk read on every frame. This stays in lockstep with
        // `active_resolved_credential`'s explicit arms above.
        match self.active_provider() {
            ActiveProvider::Claude => match self.anthropic_provider()?.credential_mode() {
                anthropic::AnthropicCredentialMode::OAuth => Some(ResolvedCredential::Oauth),
                anthropic::AnthropicCredentialMode::ApiKey => Some(ResolvedCredential::ApiKey),
                anthropic::AnthropicCredentialMode::Auto => None,
            },
            ActiveProvider::OpenAI => match self.openai_provider()?.credential_mode() {
                openai::OpenAICredentialMode::OAuth => Some(ResolvedCredential::Oauth),
                openai::OpenAICredentialMode::ApiKey => Some(ResolvedCredential::ApiKey),
                openai::OpenAICredentialMode::Auto => None,
            },
            _ => None,
        }
    }

    fn supports_image_input(&self) -> bool {
        match self.active_provider() {
            ActiveProvider::Claude => self
                .anthropic_provider()
                .map(|provider| provider.supports_image_input())
                .or_else(|| {
                    self.claude_provider()
                        .map(|provider| provider.supports_image_input())
                })
                .unwrap_or(false),
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|provider| provider.supports_image_input())
                .unwrap_or(false),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|provider| provider.supports_image_input())
                .unwrap_or(false),
            ActiveProvider::Antigravity => self
                .antigravity_provider()
                .map(|provider| provider.supports_image_input())
                .unwrap_or(false),
            ActiveProvider::Gemini => self
                .gemini_provider()
                .map(|provider| provider.supports_image_input())
                .unwrap_or(false),
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|provider| provider.supports_image_input())
                .unwrap_or(false),
            ActiveProvider::Bedrock => self
                .bedrock_provider()
                .map(|provider| provider.supports_image_input())
                .unwrap_or(false),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|provider| provider.supports_image_input())
                .unwrap_or(false),
        }
    }

    fn set_model(&self, model: &str) -> Result<()> {
        self.spawn_anthropic_catalog_refresh_if_needed();
        self.spawn_openai_catalog_refresh_if_needed();
        // Model/profile switches change route availability details; rebuild
        // the catalog on next read instead of serving the memoized copy.
        self.invalidate_routes_memo();

        let requested_model = model.trim();
        if requested_model.is_empty() {
            anyhow::bail!("Model cannot be empty");
        }

        if let Some(target_model) = requested_model.strip_prefix("grok-build:") {
            let target_model = target_model.trim();
            if target_model.is_empty() {
                anyhow::bail!("Grok Build model cannot be empty");
            }
            let registry = ProviderRegistry::new(self);
            let provider = registry
                .compatible_profile(GROK_BUILD_PROFILE_ID)
                .or_else(|| {
                    external::instantiate_expected_external_provider(external::GROK_BUILD_RUNTIME)
                })
                .ok_or_else(|| anyhow!("Grok Build is not authenticated"))?;
            provider.set_model(target_model)?;
            registry.install_compatible_profile(GROK_BUILD_PROFILE_ID, provider);
            registry.set_active_compatible_profile(GROK_BUILD_PROFILE_ID);
            self.set_active_provider(ActiveProvider::OpenRouter);
            return Ok(());
        }

        if let Some(target_model) = requested_model.strip_prefix("kimi-code-acp:") {
            let target_model = target_model.trim();
            if target_model.is_empty() {
                anyhow::bail!("Kimi Code model cannot be empty");
            }
            let registry = ProviderRegistry::new(self);
            let provider = registry
                .compatible_profile(KIMI_CODE_ACP_PROFILE_ID)
                .or_else(|| {
                    external::instantiate_expected_external_provider(
                        external::KIMI_CODE_ACP_RUNTIME,
                    )
                })
                .ok_or_else(|| anyhow!("Kimi Code CLI is not authenticated"))?;
            provider.set_model(target_model)?;
            registry.install_compatible_profile(KIMI_CODE_ACP_PROFILE_ID, provider);
            registry.set_active_compatible_profile(KIMI_CODE_ACP_PROFILE_ID);
            self.set_active_provider(ActiveProvider::OpenRouter);
            return Ok(());
        }

        if let Some(target_model) = requested_model.strip_prefix("reasonix:") {
            let target_model = target_model.trim();
            if target_model.is_empty() {
                anyhow::bail!("Reasonix model cannot be empty");
            }
            let registry = ProviderRegistry::new(self);
            let provider = registry
                .compatible_profile(REASONIX_PROFILE_ID)
                .or_else(|| {
                    external::instantiate_expected_external_provider(external::REASONIX_RUNTIME)
                })
                .ok_or_else(|| anyhow!("Reasonix is not configured"))?;
            provider.set_model(target_model)?;
            registry.install_compatible_profile(REASONIX_PROFILE_ID, provider);
            registry.set_active_compatible_profile(REASONIX_PROFILE_ID);
            self.set_active_provider(ActiveProvider::OpenRouter);
            return Ok(());
        }

        let cfg = crate::config::config();
        let resolved = resolve_model_spec(requested_model, cfg);

        // Provider-prefixed model names are explicit routing directives. They
        // must never silently fall through to another provider when the target
        // is unavailable or when --provider locks a different backend.
        if let Some(prefix) = resolved.explicit_prefix.as_deref()
            && let Some(target) = resolved
                .provider_key
                .as_deref()
                .and_then(ActiveProvider::from_key_or_alias)
        {
            self.ensure_provider_lock_allows_model_target(target, requested_model)?;
            // The single canonical parser decides whether this prefix pins a
            // dual-auth credential (and which provider/mode). Bare `claude:` /
            // `openai:` prefixes route without pinning a credential.
            let pinned = jcode_provider_core::AuthRoute::parse_explicit_credential_prefix(prefix);
            let openai_credential_mode = pinned.and_then(|route| {
                matches!(
                    route.provider,
                    jcode_provider_core::DualAuthProvider::OpenAI
                )
                .then(|| match route.mode {
                    jcode_provider_core::AuthMode::ApiKey => openai::OpenAICredentialMode::ApiKey,
                    jcode_provider_core::AuthMode::Oauth => openai::OpenAICredentialMode::OAuth,
                })
            });
            let anthropic_credential_mode = pinned.and_then(|route| {
                matches!(
                    route.provider,
                    jcode_provider_core::DualAuthProvider::Anthropic
                )
                .then(|| match route.mode {
                    jcode_provider_core::AuthMode::ApiKey => {
                        anthropic::AnthropicCredentialMode::ApiKey
                    }
                    jcode_provider_core::AuthMode::Oauth => {
                        anthropic::AnthropicCredentialMode::OAuth
                    }
                })
            });
            if openai_credential_mode.is_some() || anthropic_credential_mode.is_some() {
                return self.set_model_on_provider_with_credential_modes(
                    target,
                    &resolved.bare_model,
                    openai_credential_mode,
                    anthropic_credential_mode,
                );
            }
            return self.set_model_on_provider(target, &resolved.bare_model);
        }

        if let Some(profile) = resolved
            .provider_key
            .as_deref()
            .and_then(crate::provider_catalog::openai_compatible_profile_by_id)
            .filter(|_| resolved.explicit_prefix.is_some())
        {
            self.ensure_provider_lock_allows_openai_compatible_profile(requested_model)?;
            return self.set_model_on_openai_compatible_profile(profile, &resolved.bare_model);
        }

        // User-defined named provider profiles from config (`[providers.<name>]`).
        // The model picker emits `<name>:<model>` specs for their routes
        // (issue #444), so the switch must bind that profile's runtime instead
        // of falling through to global model-name heuristics.
        if let Some(profile_name) = resolved.provider_key.as_deref().filter(|profile| {
            resolved.explicit_prefix.is_some() && cfg.providers.contains_key(*profile)
        }) {
            self.ensure_provider_lock_allows_openai_compatible_profile(requested_model)?;
            return self.set_model_on_named_provider_profile(profile_name, &resolved.bare_model);
        }

        // A CLI --provider lock means the model string is provider-local. Do
        // not apply global Claude/OpenAI/OpenRouter heuristics here: custom
        // OpenAI-compatible endpoints often use model IDs that look like other
        // providers' IDs, and GitHub Copilot uses Claude-looking dotted names.
        if let Some(forced) = self.forced_provider {
            return self.set_model_on_provider(forced, requested_model);
        }

        // Normalize Copilot-style model names (dots -> hyphens) to canonical form.
        // e.g. "claude-opus-4.6" -> "claude-opus-4-6" so Anthropic accepts it.
        let model = if let Some(canonical) = normalize_copilot_model_name(requested_model) {
            canonical
        } else {
            requested_model
        };

        if resolved.provider_key.as_deref() == Some(ActiveProvider::OpenRouter.key())
            && let Some((base_model, provider_pin)) = model.rsplit_once('@')
            && !provider_pin.trim().is_empty()
            && let Some(openrouter_model) = openrouter_catalog_model_id(base_model)
        {
            return self.set_model_on_provider(
                ActiveProvider::OpenRouter,
                &format!("{}@{}", openrouter_model, provider_pin),
            );
        }

        // Detect which provider this model belongs to when no explicit
        // --provider lock was requested.
        let target_provider = resolve_model_spec(model, cfg).provider_key;
        if let Some(target_provider) = target_provider
            && let Some(target) = provider_from_model_key(&target_provider)
        {
            self.set_model_on_provider(target, model)
        } else {
            // Unknown model - try current provider.
            self.set_model_on_provider(self.active_provider(), model)
        }
    }

    fn set_route_selection(&self, selection: &RouteSelection) -> Result<()> {
        if selection.model.trim().is_empty() {
            anyhow::bail!("Model cannot be empty");
        }

        // Routing-prefix policy lives once in RouteSelection::routed_model_spec
        // so this orchestrator and every single-runtime provider agree on the
        // spec string. set_model then dispatches it to the right sub-provider.
        self.set_model(&selection.routed_model_spec())
    }

    fn available_models(&self) -> Vec<&'static str> {
        let mut models = Vec::new();
        models.extend_from_slice(ALL_CLAUDE_MODELS);
        models.extend_from_slice(ALL_OPENAI_MODELS);
        models
    }

    fn available_models_for_switching(&self) -> Vec<String> {
        match self.active_provider() {
            ActiveProvider::Claude => {
                if let Some(anthropic) = self.anthropic_provider() {
                    anthropic.available_models_for_switching()
                } else if let Some(claude) = self.claude_provider() {
                    claude.available_models_for_switching()
                } else {
                    Vec::new()
                }
            }
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|openai| openai.available_models_for_switching())
                .unwrap_or_default(),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|copilot| copilot.available_models_for_switching())
                .unwrap_or_default(),
            ActiveProvider::Antigravity => self
                .antigravity_provider()
                .map(|antigravity| antigravity.available_models_for_switching())
                .unwrap_or_default(),
            ActiveProvider::Gemini => self
                .gemini_provider()
                .map(|gemini| gemini.available_models_for_switching())
                .unwrap_or_default(),
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|cursor| cursor.available_models_for_switching())
                .unwrap_or_default(),
            ActiveProvider::Bedrock => self
                .bedrock_provider()
                .map(|bedrock| bedrock.available_models_for_switching())
                .unwrap_or_default(),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|openrouter| openrouter.available_models_for_switching())
                .unwrap_or_default(),
        }
    }

    fn available_models_display(&self) -> Vec<String> {
        self.fresh_routes_memo_entry().listable_models
    }

    fn available_providers_for_model(&self, model: &str) -> Vec<String> {
        if let Some(model) = openrouter_catalog_model_id(model)
            && let Some(openrouter) = self.openrouter_provider()
        {
            return openrouter.available_providers_for_model(&model);
        }
        Vec::new()
    }

    fn provider_details_for_model(&self, model: &str) -> Vec<(String, String)> {
        if let Some(model) = openrouter_catalog_model_id(model)
            && let Some(openrouter) = self.openrouter_provider()
        {
            return openrouter.provider_details_for_model(&model);
        }
        Vec::new()
    }

    fn preferred_provider(&self) -> Option<String> {
        if let Some(openrouter) = self.openrouter_provider()
            && matches!(
                *self
                    .active
                    .read()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
                ActiveProvider::OpenRouter
            )
        {
            return openrouter.preferred_provider();
        }
        None
    }

    fn model_routes(&self) -> Vec<ModelRoute> {
        self.fresh_routes_memo_entry().routes
    }

    async fn prefetch_models(&self) -> Result<()> {
        let anthropic = self.anthropic_provider();
        let claude = self.claude_provider();
        let openai = self.openai_provider();
        let openrouter = self.openrouter_provider();
        let copilot = self
            .copilot_api
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let antigravity = self.antigravity_provider();
        let gemini = self.gemini_provider();
        let cursor = self.cursor_provider();
        let bedrock = self.bedrock_provider();

        let (
            anthropic_result,
            claude_result,
            openai_result,
            openrouter_result,
            copilot_result,
            antigravity_result,
            gemini_result,
            cursor_result,
            bedrock_result,
        ) = tokio::join!(
            async {
                match anthropic {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
            async {
                match claude {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
            async {
                match openai {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
            async {
                match openrouter {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
            async {
                match copilot {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
            async {
                match antigravity {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
            async {
                match gemini {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
            async {
                match cursor {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
            async {
                match bedrock {
                    Some(provider) => provider.prefetch_models().await,
                    None => Ok(()),
                }
            },
        );

        let active_provider = self.active_provider();
        let mut errors = Vec::new();
        let mut optional_errors = Vec::new();
        for (provider_name, result) in [
            ("anthropic", anthropic_result),
            ("claude", claude_result),
            ("openai", openai_result),
            ("openrouter", openrouter_result),
            ("copilot", copilot_result),
            ("antigravity", antigravity_result),
            ("gemini", gemini_result),
            ("cursor", cursor_result),
            ("bedrock", bedrock_result),
        ] {
            if let Err(err) = result {
                let is_active = matches!(
                    (active_provider, provider_name),
                    (ActiveProvider::Claude, "anthropic" | "claude")
                        | (ActiveProvider::OpenAI, "openai")
                        | (ActiveProvider::OpenRouter, "openrouter")
                        | (ActiveProvider::Copilot, "copilot")
                        | (ActiveProvider::Antigravity, "antigravity")
                        | (ActiveProvider::Gemini, "gemini")
                        | (ActiveProvider::Cursor, "cursor")
                        | (ActiveProvider::Bedrock, "bedrock")
                );
                if !is_active || matches!(provider_name, "bedrock") {
                    optional_errors.push(format!("{provider_name}: {err}"));
                } else {
                    errors.push(format!("{provider_name}: {err}"));
                }
            }
        }

        if !optional_errors.is_empty() {
            crate::logging::warn(&format!(
                "Optional model catalog refresh failed: {}",
                optional_errors.join("; ")
            ));
        }

        if !errors.is_empty() {
            return Err(anyhow!("{}", errors.join("; ")));
        }

        // Fresh catalogs may have arrived; retire every memoized copy.
        self.invalidate_routes_memo_globally();
        Ok(())
    }

    fn on_auth_changed(&self) {
        self.handle_auth_changed(false);
    }

    fn on_auth_changed_preserve_current_provider(&self) {
        self.handle_auth_changed(true);
    }

    async fn invalidate_credentials(&self) {
        if let Some(anthropic) = self.anthropic_provider() {
            anthropic.invalidate_credentials().await;
        }
        if let Some(openai) = self.openai_provider() {
            openai.invalidate_credentials().await;
        }
    }

    fn handles_tools_internally(&self) -> bool {
        match self.active_provider() {
            ActiveProvider::Claude => {
                // Direct API does NOT handle tools internally - jcode executes them
                if self.anthropic_provider().is_some() {
                    false
                } else {
                    self.claude_provider()
                        .map(|c| c.handles_tools_internally())
                        .unwrap_or(false)
                }
            }
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|o| o.handles_tools_internally())
                .unwrap_or(false),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|o| o.handles_tools_internally())
                .unwrap_or(false),
            ActiveProvider::Antigravity => false,
            ActiveProvider::Gemini => false,
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|o| o.handles_tools_internally())
                .unwrap_or(false),
            ActiveProvider::Bedrock => false, // jcode executes Bedrock tool calls
            ActiveProvider::OpenRouter => ProviderRegistry::new(self)
                .active_openrouter_execution()
                .map(|provider| provider.handles_tools_internally())
                .unwrap_or(false),
        }
    }

    fn reasoning_effort(&self) -> Option<String> {
        match self.active_provider() {
            ActiveProvider::Claude => {
                if self.use_claude_cli {
                    None
                } else {
                    self.anthropic_provider()
                        .and_then(|provider| provider.reasoning_effort())
                }
            }
            ActiveProvider::OpenAI => self.openai_provider().and_then(|o| o.reasoning_effort()),
            ActiveProvider::Copilot => None,
            ActiveProvider::Antigravity => None,
            ActiveProvider::Gemini => None,
            ActiveProvider::Cursor => None,
            ActiveProvider::Bedrock => None,
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .and_then(|o| o.reasoning_effort()),
        }
    }

    fn set_reasoning_effort(&self, effort: &str) -> Result<()> {
        match self.active_provider() {
            ActiveProvider::Claude if !self.use_claude_cli => self
                .anthropic_provider()
                .ok_or_else(|| anyhow::anyhow!("Anthropic provider not available"))?
                .set_reasoning_effort(effort),
            ActiveProvider::OpenAI => self
                .openai_provider()
                .ok_or_else(|| anyhow::anyhow!("OpenAI provider not available"))?
                .set_reasoning_effort(effort),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .ok_or_else(|| anyhow::anyhow!("OpenAI-compatible provider not available"))?
                .set_reasoning_effort(effort),
            _ => Err(anyhow::anyhow!(
                "Reasoning effort is only supported for OpenAI, Anthropic, and compatible reasoning models"
            )),
        }
    }

    fn available_efforts(&self) -> Vec<&'static str> {
        match self.active_provider() {
            ActiveProvider::Claude if !self.use_claude_cli => self
                .anthropic_provider()
                .map(|provider| provider.available_efforts())
                .unwrap_or_default(),
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|o| o.available_efforts())
                .unwrap_or_default(),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|o| o.available_efforts())
                .unwrap_or_default(),
            ActiveProvider::Copilot => vec![],
            ActiveProvider::Antigravity => vec![],
            ActiveProvider::Gemini => vec![],
            ActiveProvider::Cursor => vec![],
            _ => vec![],
        }
    }

    fn service_tier(&self) -> Option<String> {
        match self.active_provider() {
            ActiveProvider::Claude if !self.use_claude_cli => {
                self.anthropic_provider().and_then(|a| a.service_tier())
            }
            ActiveProvider::OpenAI => self.openai_provider().and_then(|o| o.service_tier()),
            _ => None,
        }
    }

    fn set_service_tier(&self, service_tier: &str) -> Result<()> {
        match self.active_provider() {
            ActiveProvider::Claude if !self.use_claude_cli => self
                .anthropic_provider()
                .ok_or_else(|| anyhow::anyhow!("Anthropic provider not available"))?
                .set_service_tier(service_tier),
            ActiveProvider::OpenAI => self
                .openai_provider()
                .ok_or_else(|| anyhow::anyhow!("OpenAI provider not available"))?
                .set_service_tier(service_tier),
            _ => Err(anyhow::anyhow!(
                "Service tier switching is only supported for OpenAI models and Claude Opus 4.8"
            )),
        }
    }

    fn available_service_tiers(&self) -> Vec<&'static str> {
        match self.active_provider() {
            ActiveProvider::Claude if !self.use_claude_cli => self
                .anthropic_provider()
                .map(|a| a.available_service_tiers())
                .unwrap_or_default(),
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|o| o.available_service_tiers())
                .unwrap_or_default(),
            _ => vec![],
        }
    }

    fn native_compaction_mode(&self) -> Option<String> {
        match self.active_provider() {
            ActiveProvider::OpenAI => self
                .openai_provider()
                .and_then(|o| o.native_compaction_mode()),
            _ => None,
        }
    }

    fn native_compaction_threshold_tokens(&self) -> Option<usize> {
        match self.active_provider() {
            ActiveProvider::OpenAI => self
                .openai_provider()
                .and_then(|o| o.native_compaction_threshold_tokens()),
            _ => None,
        }
    }

    fn transport(&self) -> Option<String> {
        match self.active_provider() {
            ActiveProvider::OpenAI => self.openai_provider().and_then(|o| o.transport()),
            _ => None,
        }
    }

    fn set_transport(&self, transport: &str) -> Result<()> {
        match self.active_provider() {
            ActiveProvider::OpenAI => self
                .openai_provider()
                .ok_or_else(|| anyhow::anyhow!("OpenAI provider not available"))?
                .set_transport(transport),
            _ => Err(anyhow::anyhow!(
                "Transport switching is only supported for OpenAI models"
            )),
        }
    }

    fn available_transports(&self) -> Vec<&'static str> {
        match self.active_provider() {
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|o| o.available_transports())
                .unwrap_or_default(),
            ActiveProvider::Gemini => vec![],
            ActiveProvider::Cursor => vec![],
            _ => vec![],
        }
    }

    fn supports_compaction(&self) -> bool {
        match self.active_provider() {
            ActiveProvider::Claude => {
                if self.anthropic_provider().is_some() {
                    true
                } else {
                    self.claude_provider()
                        .map(|c| c.supports_compaction())
                        .unwrap_or(false)
                }
            }
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|o| o.supports_compaction())
                .unwrap_or(false),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|o| o.supports_compaction())
                .unwrap_or(false),
            ActiveProvider::Antigravity => self
                .antigravity_provider()
                .map(|o| o.supports_compaction())
                .unwrap_or(false),
            ActiveProvider::Gemini => self
                .gemini_provider()
                .map(|o| o.supports_compaction())
                .unwrap_or(false),
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|o| o.supports_compaction())
                .unwrap_or(false),
            ActiveProvider::Bedrock => self
                .bedrock_provider()
                .map(|o| o.uses_jcode_compaction())
                .unwrap_or(false),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|o| o.supports_compaction())
                .unwrap_or(false),
        }
    }

    fn uses_jcode_compaction(&self) -> bool {
        match self.active_provider() {
            ActiveProvider::Claude => {
                if self.anthropic_provider().is_some() {
                    true
                } else {
                    self.claude_provider()
                        .map(|c| c.uses_jcode_compaction())
                        .unwrap_or(false)
                }
            }
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|o| o.uses_jcode_compaction())
                .unwrap_or(false),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|o| o.uses_jcode_compaction())
                .unwrap_or(false),
            ActiveProvider::Antigravity => self
                .antigravity_provider()
                .map(|o| o.uses_jcode_compaction())
                .unwrap_or(false),
            ActiveProvider::Gemini => self
                .gemini_provider()
                .map(|o| o.uses_jcode_compaction())
                .unwrap_or(false),
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|o| o.uses_jcode_compaction())
                .unwrap_or(false),
            ActiveProvider::Bedrock => false,
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|o| o.uses_jcode_compaction())
                .unwrap_or(false),
        }
    }

    async fn native_compact(
        &self,
        messages: &[Message],
        existing_summary_text: Option<&str>,
        existing_openai_encrypted_content: Option<&str>,
    ) -> Result<NativeCompactionResult> {
        match self.active_provider() {
            ActiveProvider::Claude => {
                if let Some(anthropic) = self.anthropic_provider() {
                    anthropic
                        .native_compact(
                            messages,
                            existing_summary_text,
                            existing_openai_encrypted_content,
                        )
                        .await
                } else if let Some(claude) = self.claude_provider() {
                    claude
                        .native_compact(
                            messages,
                            existing_summary_text,
                            existing_openai_encrypted_content,
                        )
                        .await
                } else {
                    Err(anyhow::anyhow!("Claude provider unavailable"))
                }
            }
            ActiveProvider::OpenAI => {
                if let Some(openai) = self.openai_provider() {
                    openai
                        .native_compact(
                            messages,
                            existing_summary_text,
                            existing_openai_encrypted_content,
                        )
                        .await
                } else {
                    Err(anyhow::anyhow!("OpenAI provider unavailable"))
                }
            }
            ActiveProvider::Copilot => {
                let provider = self.copilot_provider();
                if let Some(copilot) = provider {
                    copilot
                        .native_compact(
                            messages,
                            existing_summary_text,
                            existing_openai_encrypted_content,
                        )
                        .await
                } else {
                    Err(anyhow::anyhow!("Copilot provider unavailable"))
                }
            }
            ActiveProvider::Antigravity => Err(anyhow::anyhow!(
                "Antigravity does not support native compaction"
            )),
            ActiveProvider::Gemini => {
                let provider = self.gemini_provider();
                if let Some(gemini) = provider {
                    gemini
                        .native_compact(
                            messages,
                            existing_summary_text,
                            existing_openai_encrypted_content,
                        )
                        .await
                } else {
                    Err(anyhow::anyhow!("Gemini provider unavailable"))
                }
            }
            ActiveProvider::Cursor => {
                let provider = self.cursor_provider();
                if let Some(cursor) = provider {
                    cursor
                        .native_compact(
                            messages,
                            existing_summary_text,
                            existing_openai_encrypted_content,
                        )
                        .await
                } else {
                    Err(anyhow::anyhow!("Cursor provider unavailable"))
                }
            }
            ActiveProvider::Bedrock => Err(anyhow::anyhow!(
                "AWS Bedrock does not support native compaction"
            )),
            ActiveProvider::OpenRouter => {
                let provider = self.active_openrouter_execution_provider();
                if let Some(openrouter) = provider {
                    openrouter
                        .native_compact(
                            messages,
                            existing_summary_text,
                            existing_openai_encrypted_content,
                        )
                        .await
                } else {
                    Err(anyhow::anyhow!("OpenRouter provider unavailable"))
                }
            }
        }
    }

    fn set_premium_mode(&self, mode: PremiumMode) {
        if let Some(copilot) = self.copilot_provider() {
            copilot.set_premium_mode(mode);
        }
    }

    fn premium_mode(&self) -> PremiumMode {
        if let Some(copilot) = self.copilot_provider() {
            copilot.premium_mode()
        } else {
            PremiumMode::Normal
        }
    }

    fn drain_startup_notices(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .startup_notices
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    fn context_window(&self) -> usize {
        match self.active_provider() {
            ActiveProvider::Claude => {
                if let Some(anthropic) = self.anthropic_provider() {
                    anthropic.context_window()
                } else if let Some(claude) = self.claude_provider() {
                    claude.context_window()
                } else {
                    DEFAULT_CONTEXT_LIMIT
                }
            }
            ActiveProvider::OpenAI => self
                .openai_provider()
                .map(|o| o.context_window())
                .unwrap_or(DEFAULT_CONTEXT_LIMIT),
            ActiveProvider::Copilot => self
                .copilot_provider()
                .map(|o| o.context_window())
                .unwrap_or(DEFAULT_CONTEXT_LIMIT),
            ActiveProvider::Antigravity => self
                .antigravity_provider()
                .map(|o| o.context_window())
                .unwrap_or(DEFAULT_CONTEXT_LIMIT),
            ActiveProvider::Gemini => self
                .gemini_provider()
                .map(|o| o.context_window())
                .unwrap_or(DEFAULT_CONTEXT_LIMIT),
            ActiveProvider::Cursor => self
                .cursor_provider()
                .map(|o| o.context_window())
                .unwrap_or(DEFAULT_CONTEXT_LIMIT),
            ActiveProvider::Bedrock => self
                .bedrock_provider()
                .map(|o| o.context_window())
                .unwrap_or(DEFAULT_CONTEXT_LIMIT),
            ActiveProvider::OpenRouter => self
                .active_openrouter_execution_provider()
                .map(|o| o.context_window())
                .unwrap_or(DEFAULT_CONTEXT_LIMIT),
        }
    }

    fn fork(&self) -> Arc<dyn Provider> {
        let current_model = self.model();
        let active = self.active_provider();

        let claude = if matches!(active, ActiveProvider::Claude) && self.claude_provider().is_some()
        {
            external::instantiate_expected_external_provider(external::CLAUDE_CLI_RUNTIME)
        } else {
            None
        };
        let anthropic = if self.anthropic_provider().is_some() {
            external::instantiate_expected_external_provider(external::ANTHROPIC_RUNTIME)
        } else {
            None
        };
        let openai = if self.openai_provider().is_some() {
            external::instantiate_expected_external_provider(external::OPENAI_RUNTIME)
        } else {
            None
        };
        let copilot_api = {
            let live = self
                .copilot_api
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            live.map(|provider| provider.fork())
        };
        let antigravity_provider = {
            let live = self
                .antigravity
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            live.map(|provider| provider.fork())
        };
        let gemini_provider = {
            let live = self
                .gemini
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            live.map(|provider| provider.fork())
        };
        let cursor_provider = if self
            .cursor
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
        {
            external::instantiate_expected_external_provider(external::CURSOR_RUNTIME)
        } else {
            None
        };
        let bedrock_provider = if self.bedrock_provider().is_some() {
            Some(Arc::new(bedrock::BedrockProvider::new()))
        } else {
            None
        };
        let openrouter = if self
            .openrouter
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
        {
            external::instantiate_openrouter_runtime(external::OpenRouterRuntimeSpec::Default).ok()
        } else {
            None
        };

        let provider = Self {
            claude: RwLock::new(claude),
            anthropic: RwLock::new(anthropic),
            openai: RwLock::new(openai),
            copilot_api: RwLock::new(copilot_api),
            antigravity: RwLock::new(antigravity_provider),
            gemini: RwLock::new(gemini_provider),
            cursor: RwLock::new(cursor_provider),
            bedrock: RwLock::new(bedrock_provider),
            openrouter: RwLock::new(openrouter),
            openai_compatible_profiles: RwLock::new(HashMap::new()),
            active_openai_compatible_profile: RwLock::new(None),
            active: RwLock::new(active),
            use_claude_cli: self.use_claude_cli,
            startup_notices: RwLock::new(Vec::new()),
            forced_provider: self.forced_provider,
            routes_memo: Mutex::new(None),
        };

        provider.spawn_anthropic_catalog_refresh_if_needed();
        provider.spawn_openai_catalog_refresh_if_needed();
        let switch_request = self.fork_model_switch_request(active, &current_model);
        let _ = provider.set_model(&switch_request);
        Arc::new(provider)
    }

    fn fork_with_model_spec(&self, model_spec: &str) -> Result<Arc<dyn Provider>> {
        // Fork first (WI-0 guarantees each external runtime slot is forked into
        // an independent instance), then pin the requested model on that
        // isolated instance only. The live agent's selection is never touched.
        // `MultiProvider::set_model` parses any explicit auth-route prefix and
        // applies the pinned credential mode, so the full original spec must be
        // passed through unchanged.
        let fork = self.fork();
        fork.set_model(model_spec).with_context(|| {
            format!(
                "Failed to pin sidecar model '{}' on forked {} provider",
                model_spec,
                self.display_name()
            )
        })?;
        Ok(fork)
    }

    fn fork_for_new_session(&self) -> Arc<dyn Provider> {
        // A shared server can outlive config changes made by a model picker in a
        // client process. Ordinary `fork()` intentionally preserves this
        // template's current selection, which is correct for resumed sessions and
        // in-flight helper work but stale for a brand-new session. Reconstruct the
        // orchestrator so the reloadable config cache and current auth state choose
        // the provider/model again.
        let provider = Self::new_fast();

        // Explicit CLI provider/model overrides remain stronger than config. The
        // forced provider is represented in process env, while the CLI model only
        // lives on the template, so restore that exact route after reconstruction.
        if self.forced_provider.is_some() {
            let active = self.active_provider();
            let current_model = self.model();
            let switch_request = self.fork_model_switch_request(active, &current_model);
            if let Err(error) = provider.set_model(&switch_request) {
                crate::logging::warn(&format!(
                    "Failed to preserve forced provider model '{}' for new session: {}",
                    switch_request, error
                ));
            }
        }

        Arc::new(provider)
    }

    fn native_result_sender(&self) -> Option<NativeToolResultSender> {
        match self.active_provider() {
            // Direct API doesn't use native result sender
            ActiveProvider::Claude => {
                if self.anthropic_provider().is_some() {
                    None
                } else {
                    self.claude_provider()
                        .and_then(|c| c.native_result_sender())
                }
            }
            ActiveProvider::OpenAI => None,
            ActiveProvider::Copilot => None,
            ActiveProvider::Antigravity => None,
            ActiveProvider::Gemini => None,
            ActiveProvider::Cursor => None,
            ActiveProvider::Bedrock => None,
            ActiveProvider::OpenRouter => None,
        }
    }

    fn switch_active_provider_to(&self, provider: &str) -> Result<()> {
        let target = Self::parse_provider_hint(provider)
            .ok_or_else(|| anyhow::anyhow!("Unknown provider `{}`", provider))?;
        if !self.provider_is_configured(target) {
            anyhow::bail!(
                "Provider `{}` is not configured in this session",
                Self::provider_key(target)
            );
        }
        self.set_active_provider(target);
        self.auto_select_multi_account_for_provider(target);
        Ok(())
    }
}

/// Get the prompt cache TTL in seconds for a given provider name.
/// Returns None if the provider doesn't support prompt caching or TTL is unknown.
pub fn cache_ttl_for_provider(provider: &str) -> Option<u64> {
    cache_ttl_for_provider_model(provider, None)
}

/// Get the prompt cache TTL in seconds for a given provider/model pair.
///
/// This is provider cache-retention policy: it depends only on provider
/// families (anthropic/openai/...) and their model capabilities, so it lives
/// in `provider` rather than the UI layer.
pub fn cache_ttl_for_provider_model(provider: &str, model: Option<&str>) -> Option<u64> {
    match provider.to_lowercase().as_str() {
        "anthropic" | "claude" => Some(if anthropic::is_cache_ttl_1h() {
            60 * 60
        } else {
            300
        }),
        "openai" => {
            if model
                .map(openai::supports_extended_prompt_cache_retention)
                .unwrap_or(false)
            {
                Some(24 * 60 * 60)
            } else {
                Some(300)
            }
        }
        "openrouter" => Some(300),
        "jcode subscription" => Some(300),
        "gemini" => Some(300),
        "copilot" => None,
        "cursor" => None,
        "antigravity" => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests;
