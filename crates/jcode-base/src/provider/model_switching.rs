use super::*;

impl MultiProvider {
    /// Bind (or reuse) the runtime for a named config provider profile and
    /// select `model` on it (issue #444).
    pub(super) fn set_model_on_named_provider_profile(
        &self,
        profile_name: &str,
        model: &str,
    ) -> Result<()> {
        let model = model.trim();
        if model.is_empty() {
            anyhow::bail!("Model cannot be empty");
        }
        let config = crate::config::config()
            .providers
            .get(profile_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Unknown provider profile '{}'", profile_name))?;

        let expected_api_method = format!("openai-compatible:{}", profile_name);
        let registry = ProviderRegistry::new(self);
        let provider = {
            let existing = registry
                .compatible_profile(profile_name)
                .filter(|provider| {
                    provider
                        .direct_openai_compatible_route_parts()
                        .map(|(_provider, api_method, _detail)| api_method == expected_api_method)
                        .unwrap_or(false)
                });
            if let Some(provider) = existing {
                provider
            } else {
                let provider = external::instantiate_openrouter_runtime(
                    external::OpenRouterRuntimeSpec::NamedProfile {
                        name: profile_name.to_string(),
                        config,
                    },
                )?;
                registry
                    .install_compatible_profile(profile_name.to_string(), Arc::clone(&provider));
                provider
            }
        };
        provider.set_model(model)?;
        registry.set_active_compatible_profile(profile_name.to_string());
        self.set_active_provider(ActiveProvider::OpenRouter);
        Ok(())
    }

    pub(super) fn ensure_provider_lock_allows_model_target(
        &self,
        target: ActiveProvider,
        requested_model: &str,
    ) -> Result<()> {
        let Some(forced) = self.forced_provider else {
            return Ok(());
        };
        if forced == target {
            return Ok(());
        }
        anyhow::bail!(
            "Model '{}' targets {} but --provider is locked to {}. Remove the provider-specific model prefix or use `--provider {}`.",
            requested_model,
            Self::provider_label(target),
            Self::provider_label(forced),
            Self::provider_key(target),
        );
    }

    pub(super) fn ensure_provider_lock_allows_openai_compatible_profile(
        &self,
        requested_model: &str,
    ) -> Result<()> {
        let Some(forced) = self.forced_provider else {
            return Ok(());
        };
        if forced == ActiveProvider::OpenRouter {
            return Ok(());
        }
        anyhow::bail!(
            "Model '{}' targets an OpenAI-compatible provider but --provider is locked to {}. Remove the provider-specific model prefix or use `--provider openai-compatible`.",
            requested_model,
            Self::provider_label(forced),
        );
    }

    pub(super) fn set_model_on_provider(
        &self,
        provider: ActiveProvider,
        model: &str,
    ) -> Result<()> {
        self.set_model_on_provider_with_credential_modes(provider, model, None, None)
    }

    pub(super) fn set_model_on_provider_with_credential_modes(
        &self,
        provider: ActiveProvider,
        model: &str,
        openai_credential_mode: Option<openai::OpenAICredentialMode>,
        anthropic_credential_mode: Option<anthropic::AnthropicCredentialMode>,
    ) -> Result<()> {
        let model = model.trim();
        if model.is_empty() {
            anyhow::bail!("Model cannot be empty");
        }

        self.reconcile_auth_if_provider_missing(provider);

        match provider {
            ActiveProvider::Claude => {
                let model = model_name_for_provider(provider, model);
                if let Some(anthropic) = self.anthropic_provider() {
                    if let Some(mode) = anthropic_credential_mode {
                        anthropic.set_credential_mode(mode)?;
                    }
                    anthropic.set_model(&model)?;
                } else if let Some(claude) = self.claude_provider() {
                    claude.set_model(&model)?;
                } else {
                    anyhow::bail!(
                        "Claude credentials not available. Run `jcode login --provider claude` first."
                    );
                }
                self.set_active_provider(ActiveProvider::Claude);
                Ok(())
            }
            ActiveProvider::OpenAI => {
                let Some(openai) = self.openai_provider() else {
                    // No OpenAI runtime: still run the same model-name
                    // validation the runtime itself would. A cross-provider
                    // model under a forced/locked OpenAI selection must report
                    // the real problem (wrong model family), not demand a
                    // login that would never make the model valid. Keeps the
                    // error independent of which credentials exist on disk.
                    if !known_openai_model_ids().iter().any(|known| known == model) {
                        anyhow::bail!(
                            "Unsupported OpenAI model '{}'. Use /model to choose from the models available to your account.",
                            model
                        );
                    }
                    anyhow::bail!(
                        "OpenAI credentials not available. Run `jcode login --provider openai` first."
                    );
                };
                if let Some(mode) = openai_credential_mode {
                    openai.set_credential_mode(mode)?;
                }
                openai.set_model(model)?;
                self.set_active_provider(ActiveProvider::OpenAI);
                Ok(())
            }
            ActiveProvider::Copilot => {
                let Some(copilot) = self.copilot_provider() else {
                    anyhow::bail!(
                        "GitHub Copilot credentials not available. Run `jcode login --provider copilot` first."
                    );
                };
                copilot.set_model(model)?;
                self.set_active_provider(ActiveProvider::Copilot);
                Ok(())
            }
            ActiveProvider::Antigravity => {
                let Some(antigravity) = self.antigravity_provider() else {
                    anyhow::bail!(
                        "Antigravity credentials not available. Run `jcode login --provider antigravity` first."
                    );
                };
                antigravity.set_model(model)?;
                self.set_active_provider(ActiveProvider::Antigravity);
                Ok(())
            }
            ActiveProvider::Gemini => {
                let Some(gemini) = self.gemini_provider() else {
                    anyhow::bail!(
                        "Gemini credentials not available. Run `jcode login --provider gemini` first."
                    );
                };
                gemini.set_model(model)?;
                self.set_active_provider(ActiveProvider::Gemini);
                Ok(())
            }
            ActiveProvider::Cursor => {
                let Some(cursor) = self.cursor_provider() else {
                    anyhow::bail!(
                        "Cursor credentials not available. Run `jcode login --provider cursor` first."
                    );
                };
                cursor.set_model(model)?;
                self.set_active_provider(ActiveProvider::Cursor);
                Ok(())
            }
            ActiveProvider::Bedrock => {
                let Some(bedrock) = self.bedrock_provider() else {
                    anyhow::bail!(
                        "AWS Bedrock credentials not available. Configure AWS credentials and region first."
                    );
                };
                bedrock.set_model(model)?;
                self.set_active_provider(ActiveProvider::Bedrock);
                Ok(())
            }
            ActiveProvider::OpenRouter => {
                self.clear_active_openai_compatible_profile();
                // Decide whether the slot must be rebound to the real
                // OpenRouter API-key runtime. Rebinding repairs a slot left
                // flavored as a *known catalog profile* runtime by startup
                // profile env (e.g. a Cerebras login applied globally, then
                // the slot was built as Cerebras), so an OpenRouter-targeted
                // switch reaches the real aggregator again. But a *custom*
                // OpenAI-compatible endpoint (generic profile or named config
                // profile) or a CLI `--provider` lock owns the slot
                // legitimately: its model IDs are provider-local and must not
                // be re-routed through OpenRouter (or fail outright because no
                // OPENROUTER_API_KEY is configured).
                let locked_to_slot = self.forced_provider == Some(ActiveProvider::OpenRouter);
                let needs_rebind = match self.openrouter_provider().as_deref() {
                    None => true,
                    Some(provider) => {
                        !provider.supports_provider_routing_features()
                            && !locked_to_slot
                            && provider
                                .direct_openai_compatible_route_parts()
                                .and_then(|(_provider, api_method, _detail)| {
                                    api_method
                                    .strip_prefix("openai-compatible:")
                                    .map(str::trim)
                                    .and_then(
                                        crate::provider_catalog::openai_compatible_profile_by_id,
                                    )
                                })
                                .map(|profile| {
                                    profile.id != crate::provider_catalog::OPENAI_COMPAT_PROFILE.id
                                })
                                .unwrap_or(false)
                    }
                };
                if needs_rebind {
                    let provider = external::instantiate_openrouter_runtime(
                        external::OpenRouterRuntimeSpec::OpenRouterApiKey,
                    )?;
                    *self
                        .openrouter
                        .write()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(provider);
                }

                let Some(openrouter) = self.openrouter_provider() else {
                    anyhow::bail!(
                        "OpenRouter/OpenAI-compatible credentials not available. Set the configured API key or run `jcode login --provider openrouter` first."
                    );
                };
                openrouter.set_model(model)?;
                self.set_active_provider(ActiveProvider::OpenRouter);
                Ok(())
            }
        }
    }

    pub(super) fn set_model_on_openai_compatible_profile(
        &self,
        profile: crate::provider_catalog::OpenAiCompatibleProfile,
        model: &str,
    ) -> Result<()> {
        let model = model.trim();
        if model.is_empty() {
            anyhow::bail!("Model cannot be empty");
        }
        let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
        if !crate::provider_catalog::openai_compatible_profile_is_configured(profile) {
            anyhow::bail!(
                "{} credentials not available. Run `jcode login --provider {}` first.",
                resolved.display_name,
                resolved.id,
            );
        }

        let profile_id = resolved.id.clone();
        let registry = ProviderRegistry::new(self);
        let provider = {
            let existing = registry.compatible_profile(&profile_id).filter(|provider| {
                provider
                    .direct_openai_compatible_route_parts()
                    .and_then(|(_provider, api_method, _detail)| {
                        api_method
                            .strip_prefix("openai-compatible:")
                            .map(|profile| profile.trim().to_string())
                    })
                    .as_deref()
                    == Some(profile_id.as_str())
            });
            if let Some(provider) = existing {
                provider
            } else {
                let provider = external::instantiate_openrouter_runtime(
                    external::OpenRouterRuntimeSpec::CompatibleProfile(profile),
                )?;
                registry.install_compatible_profile(profile_id.clone(), Arc::clone(&provider));
                provider
            }
        };
        provider.set_model(model)?;
        registry.set_active_compatible_profile(profile_id);
        self.set_active_provider(ActiveProvider::OpenRouter);
        Ok(())
    }

    fn should_replace_openrouter_after_auth_change(
        existing: &dyn Provider,
        candidate: &dyn Provider,
    ) -> bool {
        if existing.supports_provider_routing_features()
            != candidate.supports_provider_routing_features()
        {
            return false;
        }

        let existing_direct = existing
            .direct_openai_compatible_route_parts()
            .map(|(_provider, api_method, _detail)| api_method);
        let candidate_direct = candidate
            .direct_openai_compatible_route_parts()
            .map(|(_provider, api_method, _detail)| api_method);

        existing_direct == candidate_direct
    }

    pub(super) fn handle_auth_changed(&self, preserve_existing_openrouter_profile: bool) {
        crate::logging::auth_event("auth_changed_received", "multi-provider", &[]);
        // Credentials feed route availability/pricing, so every memoized
        // catalog in the process is stale the moment auth changes.
        self.invalidate_routes_memo_globally();
        // Auth just changed, so discard any stale full/fast snapshots before
        // using cheap local probes to hot-initialize newly configured providers.
        crate::auth::AuthStatus::invalidate_cache();

        if self.use_claude_cli {
            if self.claude_provider().is_none()
                && crate::auth::claude::load_credentials().is_ok()
                && let Some(claude) =
                    external::instantiate_expected_external_provider(external::CLAUDE_CLI_RUNTIME)
            {
                crate::logging::info("Hot-initialized Claude CLI provider after auth change");
                *self
                    .claude
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(claude);
            }
        } else if self.anthropic_provider().is_none()
            && (crate::auth::claude::load_credentials().is_ok()
                || crate::provider_catalog::load_api_key(
                    &crate::provider_catalog::ApiKeyCredentialSource::primary_only(
                        "ANTHROPIC_API_KEY",
                        "anthropic.env",
                    ),
                )
                .is_some())
        {
            crate::logging::info("Hot-initialized Anthropic provider after auth change");
            *self
                .anthropic
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                external::instantiate_expected_external_provider(external::ANTHROPIC_RUNTIME);
        }

        if let Some(openai) = self.openai_provider() {
            openai.reload_credentials();
        } else if crate::auth::codex::load_credentials().is_ok()
            && let Some(openai) =
                external::instantiate_expected_external_provider(external::OPENAI_RUNTIME)
        {
            crate::logging::info("Hot-initialized OpenAI provider after auth change");
            *self
                .openai
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(openai);
        }

        if openrouter::has_credentials() {
            match external::instantiate_openrouter_runtime(external::OpenRouterRuntimeSpec::Default)
            {
                Ok(provider) => {
                    let should_install = if preserve_existing_openrouter_profile {
                        self.openrouter_provider()
                            .as_deref()
                            .map(|existing| {
                                Self::should_replace_openrouter_after_auth_change(
                                    existing,
                                    provider.as_ref(),
                                )
                            })
                            .unwrap_or(true)
                    } else {
                        true
                    };
                    if should_install {
                        crate::logging::info(
                            "Hot-initialized OpenRouter/OpenAI-compatible provider after auth change",
                        );
                        *self
                            .openrouter
                            .write()
                            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(provider);
                    } else {
                        crate::logging::info(
                            "Preserved existing OpenRouter/OpenAI-compatible provider after unrelated auth change",
                        );
                    }
                }
                Err(e) => {
                    crate::logging::info(&format!(
                        "Failed to hot-initialize OpenRouter/OpenAI-compatible provider after auth change: {}",
                        e
                    ));
                }
            }
        }

        let already_has = self.copilot_provider().is_some();
        if !already_has {
            let status = crate::auth::AuthStatus::check_fast();
            // The composition-root factory schedules tier detection itself.
            if status.copilot_has_api_token
                && let Some(provider) =
                    external::instantiate_expected_external_provider(external::COPILOT_RUNTIME)
            {
                crate::logging::info("Hot-initialized Copilot API provider after login");
                *self
                    .copilot_api
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(provider);
            }
        }

        let already_has_antigravity = self.antigravity_provider().is_some();
        if !already_has_antigravity
            && crate::auth::antigravity::load_tokens().is_ok()
            && let Some(antigravity) =
                external::instantiate_expected_external_provider(external::ANTIGRAVITY_RUNTIME)
        {
            crate::logging::info("Hot-initialized Antigravity provider after login");
            *self
                .antigravity
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(antigravity);
        }

        let already_has_gemini = self.gemini_provider().is_some();
        if !already_has_gemini
            && crate::auth::gemini::load_tokens().is_ok()
            && let Some(gemini) =
                external::instantiate_expected_external_provider(external::GEMINI_RUNTIME)
        {
            crate::logging::info("Hot-initialized Gemini provider after login");
            *self
                .gemini
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(gemini);
        }

        let already_has_cursor = self.cursor_provider().is_some();
        if !already_has_cursor
            && crate::auth::AuthStatus::check_fast()
                .assessment_for_provider(crate::provider_catalog::CURSOR_LOGIN_PROVIDER)
                .is_available()
            && let Some(cursor) =
                external::instantiate_expected_external_provider(external::CURSOR_RUNTIME)
        {
            crate::logging::info("Hot-initialized Cursor provider after login");
            *self
                .cursor
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(cursor);
        }

        let already_has_bedrock = self.bedrock_provider().is_some();
        if !already_has_bedrock && bedrock::BedrockProvider::has_credentials() {
            crate::logging::info("Hot-initialized AWS Bedrock provider after login");
            *self
                .bedrock
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(Arc::new(bedrock::BedrockProvider::new()));
        }

        let registry = ProviderRegistry::new(self);
        if crate::auth::grok_build::is_available()
            && registry.compatible_profile(GROK_BUILD_PROFILE_ID).is_none()
            && let Some(grok) =
                external::instantiate_expected_external_provider(external::GROK_BUILD_RUNTIME)
        {
            crate::logging::info("Hot-initialized Grok Build provider after login");
            registry.install_compatible_profile(GROK_BUILD_PROFILE_ID, grok);
        }
        if crate::auth::kimi_code_acp::is_available()
            && registry
                .compatible_profile(KIMI_CODE_ACP_PROFILE_ID)
                .is_none()
            && let Some(kimi) =
                external::instantiate_expected_external_provider(external::KIMI_CODE_ACP_RUNTIME)
        {
            crate::logging::info("Hot-initialized Kimi Code ACP provider after login");
            registry.install_compatible_profile(KIMI_CODE_ACP_PROFILE_ID, kimi);
        }
        if crate::auth::reasonix::is_available()
            && registry.compatible_profile(REASONIX_PROFILE_ID).is_none()
            && let Some(reasonix) =
                external::instantiate_expected_external_provider(external::REASONIX_RUNTIME)
        {
            crate::logging::info("Hot-initialized Reasonix provider after setup");
            registry.install_compatible_profile(REASONIX_PROFILE_ID, reasonix);
        }

        if let Some(anthropic) = self.anthropic_provider() {
            Self::spawn_post_auth_model_refresh(anthropic, "Anthropic");
        }
        if let Some(claude) = self.claude_provider() {
            Self::spawn_post_auth_model_refresh(claude, "Claude");
        }
        if let Some(openai) = self.openai_provider() {
            Self::spawn_post_auth_model_refresh(openai, "OpenAI");
        }
        if let Some(antigravity) = self.antigravity_provider() {
            Self::spawn_post_auth_model_refresh(antigravity, "Antigravity");
        }
        if let Some(gemini) = self.gemini_provider() {
            Self::spawn_post_auth_model_refresh(gemini, "Gemini");
        }
        if let Some(cursor) = self.cursor_provider() {
            Self::spawn_post_auth_model_refresh(cursor, "Cursor");
        }
        if let Some(openrouter) = self.openrouter_provider() {
            Self::spawn_post_auth_model_refresh(openrouter, "OpenRouter");
        }
        if let Some(bedrock) = self.bedrock_provider() {
            Self::spawn_post_auth_model_refresh(bedrock, "AWS Bedrock");
        }
        if let Some(grok) = ProviderRegistry::new(self).compatible_profile(GROK_BUILD_PROFILE_ID) {
            Self::spawn_post_auth_model_refresh(grok, "Grok Build");
        }
        if let Some(kimi) = ProviderRegistry::new(self).compatible_profile(KIMI_CODE_ACP_PROFILE_ID)
        {
            Self::spawn_post_auth_model_refresh(kimi, "Kimi Code (official CLI)");
        }
        if let Some(reasonix) = ProviderRegistry::new(self).compatible_profile(REASONIX_PROFILE_ID)
        {
            Self::spawn_post_auth_model_refresh(reasonix, "Reasonix");
        }
        crate::logging::auth_event("auth_changed_completed", "multi-provider", &[]);
    }

    pub(super) fn set_config_default_model(
        &self,
        model: &str,
        default_provider: Option<&str>,
    ) -> Result<()> {
        let model = model.trim();
        if model.is_empty() {
            anyhow::bail!("Model cannot be empty");
        }

        // The model picker persists default_model as a full model spec that
        // may carry an explicit provider/credential prefix (e.g.
        // `claude-api:claude-fable-5`). Provider-local `set_model`
        // implementations validate bare model ids, so a prefixed spec must go
        // through the canonical prefix-aware path. Handing the raw spec to a
        // single provider would make it reject the id and silently keep its
        // fallback default model.
        if resolve_model_spec(model, crate::config::config())
            .explicit_prefix
            .is_some()
        {
            return self.set_model(model);
        }

        // A configured default_provider is a routing decision, not just a
        // startup hint. Treat default_model as provider-local when the config
        // names a concrete provider/profile so global model-name heuristics
        // cannot undo that decision. This is especially important for
        // OpenAI-compatible gateways whose model IDs often look like built-in
        // OpenAI, Anthropic, or OpenRouter models.
        if let Some(pref) = default_provider.and_then(|pref| {
            let trimmed = pref.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        }) && let Some(selection) =
            Self::resolve_config_provider_selection(pref, crate::config::config())
        {
            // A known OpenAI-compatible catalog profile (deepseek, zai, ...)
            // must be handled profile-locally. Its `active_provider()` maps to
            // the shared OpenRouter slot, but routing through the generic
            // OpenRouter path would trigger the OpenRouter rebind logic, which
            // replaces the profile runtime with a plain OpenRouter API-key
            // runtime and fails when OPENROUTER_API_KEY is not configured --
            // silently dropping the configured default (issue #448).
            if let selection::ConfigProviderSelection::OpenAiCompatibleProfile(profile_id) =
                &selection
                && let Some(profile) =
                    crate::provider_catalog::openai_compatible_profile_by_id(profile_id)
            {
                return self.set_model_on_openai_compatible_profile(profile, model);
            }

            // Same reasoning for user-defined named provider profiles from
            // config: bind the named profile runtime directly instead of the
            // generic OpenRouter slot path.
            if let selection::ConfigProviderSelection::NamedProfile(profile_name) = &selection {
                return self.set_model_on_named_provider_profile(profile_name, model);
            }

            // A dual-auth config provider key (`anthropic-api`, `claude-oauth`,
            // `openai-api`, ...) also pins the OAuth-vs-API credential. Carry
            // that through so the active credential -- and every surface that
            // reads it (header auth tag, model picker) -- matches the route the
            // user configured, instead of leaving the provider in Auto mode
            // (which prefers OAuth) and silently mislabeling an API default.
            //
            // Bare provider keys (`claude`, `anthropic`, `openai`) intentionally
            // do NOT pin a credential: they keep Auto mode (so an API-only user
            // with `default_provider = "claude"` still resolves their key
            // instead of failing to load absent OAuth credentials).
            let pinned = jcode_provider_core::AuthRoute::parse_explicit_credential_prefix(pref);
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
            return self.set_model_on_provider_with_credential_modes(
                selection.active_provider(),
                model,
                openai_credential_mode,
                anthropic_credential_mode,
            );
        }

        self.set_model(model)
    }

    pub(super) fn fork_model_switch_request(
        &self,
        active: ActiveProvider,
        current_model: &str,
    ) -> String {
        let prefix = match active {
            ActiveProvider::Claude => {
                if let Some(anthropic) = self.anthropic_provider() {
                    // OAuth/ApiKey emit their canonical model prefix; Auto keeps
                    // the bare provider key (route without pinning a credential).
                    anthropic
                        .credential_mode()
                        .auth_route(jcode_provider_core::DualAuthProvider::Anthropic)
                        .map(|route| route.model_prefix())
                        .unwrap_or("claude")
                } else {
                    "claude"
                }
            }
            ActiveProvider::OpenAI => {
                if let Some(openai) = self.openai_provider() {
                    openai
                        .credential_mode()
                        .auth_route(jcode_provider_core::DualAuthProvider::OpenAI)
                        .map(|route| route.model_prefix())
                        .unwrap_or("openai")
                } else {
                    "openai"
                }
            }
            ActiveProvider::Copilot => "copilot",
            ActiveProvider::Antigravity => "antigravity",
            ActiveProvider::Gemini => "gemini",
            ActiveProvider::Cursor => "cursor",
            ActiveProvider::Bedrock => "bedrock",
            ActiveProvider::OpenRouter => {
                if ProviderRegistry::new(self)
                    .active_compatible_profile_id()
                    .as_deref()
                    == Some(GROK_BUILD_PROFILE_ID)
                {
                    return format!("grok-build:{current_model}");
                }
                if ProviderRegistry::new(self)
                    .active_compatible_profile_id()
                    .as_deref()
                    == Some(KIMI_CODE_ACP_PROFILE_ID)
                {
                    return format!("kimi-code-acp:{current_model}");
                }
                if ProviderRegistry::new(self)
                    .active_compatible_profile_id()
                    .as_deref()
                    == Some(REASONIX_PROFILE_ID)
                {
                    return format!("reasonix:{current_model}");
                }
                if let Some(openrouter) = self.active_openrouter_execution_provider()
                    && let Some((_provider, api_method, _detail)) =
                        openrouter.direct_openai_compatible_route_parts()
                    && let Some(profile_id) = api_method
                        .strip_prefix("openai-compatible:")
                        .map(str::trim)
                        .filter(|profile_id| !profile_id.is_empty())
                {
                    return format!("{profile_id}:{current_model}");
                }
                if let Some(openrouter) = self.openrouter_provider()
                    && let Some(provider_pin) = openrouter.explicit_provider_pin_for_current_model()
                {
                    return format!("openrouter:{current_model}@{provider_pin}");
                }
                "openrouter"
            }
        };
        format!("{prefix}:{current_model}")
    }
}
