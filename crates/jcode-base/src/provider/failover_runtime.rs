use super::*;

impl MultiProvider {
    #[cfg(test)]
    pub(crate) fn same_provider_account_candidates(provider: ActiveProvider) -> Vec<String> {
        account_failover::same_provider_account_candidates(provider)
    }

    pub(super) async fn complete_with_failover(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        mode: CompletionMode<'_>,
        resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        crate::logging::info("PRESTREAM: failover enter");
        self.spawn_anthropic_catalog_refresh_if_needed();
        self.spawn_openai_catalog_refresh_if_needed();

        // Downscale any images whose pixel dimensions exceed provider per-image
        // limits before they reach the wire. Resuming a session with >20 large
        // screenshots otherwise trips Anthropic's many-image 2000px cap and the
        // whole turn is rejected (#381). Only clones when a clamp is required.
        let clamped_messages = image_clamp::clamp_outbound_images(messages);
        let messages: &[Message] = clamped_messages.as_deref().unwrap_or(messages);

        let detected_active = self.active_provider();
        let active = if let Some(forced) = self.forced_provider {
            if detected_active != forced {
                crate::logging::warn(&format!(
                    "Provider lock corrected active provider from {} to {} before request",
                    Self::provider_label(detected_active),
                    Self::provider_label(forced),
                ));
                self.set_active_provider(forced);
            }
            forced
        } else {
            detected_active
        };
        let sequence = Self::fallback_sequence_for(active, self.forced_provider);
        crate::logging::info(&format!(
            "PRESTREAM: failover dispatch (active={:?})",
            active
        ));
        let mut notes: Vec<String> = Vec::new();
        let mut failover_reason: Option<String> = None;
        let (estimated_input_chars, estimated_input_tokens) =
            Self::estimate_request_input(messages, tools, mode);

        for candidate in sequence {
            let label = Self::provider_label(candidate);
            let key = Self::provider_key(candidate);

            if candidate != active && failover_reason.is_some() {
                let prompt = self.build_failover_prompt(
                    active,
                    candidate,
                    failover_reason
                        .clone()
                        .unwrap_or_else(|| "provider unavailable".to_string()),
                    estimated_input_chars,
                    estimated_input_tokens,
                );
                return Err(anyhow::anyhow!(prompt.to_error_message()));
            }

            if !self.provider_is_configured(candidate) {
                let note = format!("{}: not configured", label);
                if candidate == active {
                    crate::logging::warn(&format!(
                        "Failover{}: skipping active provider {} (not configured)",
                        mode.log_suffix(),
                        label
                    ));
                }
                notes.push(note);
                continue;
            }

            if let Some(detail) = provider_unavailability_detail_for_account(key) {
                let note = format!("{}: {}", label, detail);
                if candidate == active {
                    crate::logging::warn(&format!(
                        "Failover{}: skipping active provider {} - {}",
                        mode.log_suffix(),
                        label,
                        detail
                    ));
                    failover_reason = Some(detail.clone());
                }
                notes.push(note);
                continue;
            }

            if let Some(reason) = self.provider_precheck_unavailable_reason(candidate) {
                let note = format!("{}: {}", label, reason);
                if candidate == active {
                    crate::logging::warn(&format!(
                        "Failover{}: skipping active provider {} - {}",
                        mode.log_suffix(),
                        label,
                        reason
                    ));
                    failover_reason = Some(reason.clone());
                }
                notes.push(note);
                record_provider_unavailable_for_account(key, &reason);
                continue;
            }

            let attempt = match mode {
                CompletionMode::Unified { system } => {
                    self.complete_on_provider(candidate, messages, tools, system, resume_session_id)
                        .await
                }
                CompletionMode::Split {
                    system_static,
                    system_dynamic,
                } => {
                    self.complete_split_on_provider(
                        candidate,
                        messages,
                        tools,
                        system_static,
                        system_dynamic,
                        resume_session_id,
                    )
                    .await
                }
            };

            match attempt {
                Ok(stream) => {
                    clear_provider_unavailable_for_account(key);
                    self.record_provider_activity(candidate);
                    if candidate != active {
                        self.set_active_provider(candidate);
                        let from_label = Self::provider_label(active);
                        let to_label = Self::provider_label(candidate);
                        crate::logging::info(&format!(
                            "{}: switched from {} to {}",
                            mode.switch_log_prefix(),
                            from_label,
                            to_label
                        ));
                        self.startup_notices
                            .write()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .push(format!(
                                "⚡ Auto-fallback: {} unavailable, switched to {}",
                                from_label, to_label
                            ));
                    }
                    return Ok(stream);
                }
                Err(err) => {
                    let summary =
                        maybe_annotate_limit_summary(candidate, Self::summarize_error(&err));
                    let decision = Self::classify_failover_error(&err);
                    crate::logging::info(&format!(
                        "Provider {} failed{}: {} (failover={} decision={})",
                        label,
                        mode.log_suffix(),
                        summary,
                        decision.should_failover(),
                        decision.as_str()
                    ));
                    notes.push(format!("{}: {}", label, summary));
                    if decision.should_failover() {
                        if decision.should_mark_provider_unavailable() {
                            record_provider_unavailable_for_account(key, &summary);
                        }
                        if candidate == active
                            && let Some(stream) = self
                                .try_same_provider_account_failover(
                                    candidate, messages, tools, mode, &summary, &mut notes,
                                )
                                .await?
                        {
                            return Ok(stream);
                        }
                        if candidate == active {
                            failover_reason = Some(summary);
                        }
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Err(self.no_provider_available_error(&notes))
    }

    /// Record which login/credential just served a request in the
    /// cross-provider activity ledger (drives `/usage` recency sorting).
    /// Spawned off-thread: the ledger does file IO and a request was already
    /// accepted, so this must never block or fail the completion path.
    fn record_provider_activity(&self, provider: ActiveProvider) {
        let source_key = self.activity_source_key(provider);
        tokio::task::spawn_blocking(move || {
            crate::provider_activity::record_use(&source_key);
        });
    }

    /// Ledger source key for the credential `provider` will use right now.
    /// Mirrors `active_resolved_credential` for the dual-auth providers and
    /// the runtime profile resolution for the OpenRouter slot, but resolves
    /// against the *passed* provider so failover candidates attribute
    /// correctly even before `set_active_provider` runs.
    fn activity_source_key(&self, provider: ActiveProvider) -> String {
        match provider {
            ActiveProvider::Claude => {
                let uses_api_key = self
                    .anthropic_provider()
                    .map(|anthropic| match anthropic.credential_mode() {
                        anthropic::AnthropicCredentialMode::ApiKey => true,
                        anthropic::AnthropicCredentialMode::OAuth => false,
                        anthropic::AnthropicCredentialMode::Auto => {
                            crate::auth::claude::load_credentials().is_err()
                        }
                    })
                    .unwrap_or(false);
                if uses_api_key {
                    "claude:api-key".to_string()
                } else {
                    let label = crate::auth::claude::active_account_label()
                        .unwrap_or_else(|| "default".to_string());
                    format!("claude:oauth:{}", label)
                }
            }
            ActiveProvider::OpenAI => {
                let uses_api_key = self
                    .openai_provider()
                    .map(|openai| match openai.credential_mode() {
                        openai::OpenAICredentialMode::ApiKey => true,
                        openai::OpenAICredentialMode::OAuth => false,
                        openai::OpenAICredentialMode::Auto => {
                            crate::auth::codex::load_oauth_credentials().is_err()
                        }
                    })
                    .unwrap_or(false);
                if uses_api_key {
                    "openai:api-key".to_string()
                } else {
                    let label = crate::auth::codex::active_account_label()
                        .unwrap_or_else(|| "default".to_string());
                    format!("openai:oauth:{}", label)
                }
            }
            ActiveProvider::OpenRouter => {
                // The OpenRouter slot multiplexes the public aggregator, the
                // jcode subscription, and direct OpenAI-compatible profiles.
                let label = self
                    .active_openrouter_execution_provider()
                    .map(|execution| execution.runtime_display_name())
                    .unwrap_or_else(|| "OpenRouter".to_string());
                let runtime = std::env::var("JCODE_RUNTIME_PROVIDER").ok();
                crate::provider_activity::source_key_for_provider_label(&label, runtime.as_deref())
            }
            other => Self::provider_key(other).to_string(),
        }
    }
}
