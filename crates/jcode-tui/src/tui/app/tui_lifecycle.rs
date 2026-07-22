use super::state_ui::RestoredReloadInput;
use super::*;
use crate::tui::backend;

impl App {
    pub(super) fn apply_restored_reload_input(&mut self, restored: RestoredReloadInput) {
        self.input = restored.input;
        self.cursor_pos = restored.cursor;
        self.pending_images = restored.pending_images;
        self.submit_input_on_startup = restored.submit_on_restore
            && (!self.input.is_empty() || !self.pending_images.is_empty());
        crate::logging::info(&format!(
            "Startup input restored: submit_on_restore={} input_chars={} pending_images={} queued_messages={} hidden_system={} => submit_input_on_startup={}",
            restored.submit_on_restore,
            self.input.chars().count(),
            self.pending_images.len(),
            restored.queued_messages.len(),
            restored.hidden_queued_system_messages.len(),
            self.submit_input_on_startup,
        ));
        self.hidden_queued_system_messages = restored.hidden_queued_system_messages;
        if let Some(status_notice) = restored.startup_status_notice {
            self.set_status_notice(status_notice);
        } else if self.submit_input_on_startup {
            self.set_status_notice("Startup prompt queued");
        }
        if let Some((title, message)) = restored.startup_display_message {
            self.push_display_message(DisplayMessage::system(message).with_title(title));
        }
        self.interleave_message = None;
        self.rate_limit_pending_message = restored.rate_limit_pending_message;
        self.rate_limit_reset = restored.rate_limit_reset;
        self.observe_page_markdown = restored.observe_page_markdown;
        self.observe_page_updated_at_ms = restored.observe_page_updated_at_ms;
        self.set_observe_mode_enabled(restored.observe_mode_enabled, restored.observe_mode_enabled);
        self.set_split_view_enabled(restored.split_view_enabled, restored.split_view_enabled);
        self.set_todos_view_enabled(restored.todos_view_enabled, restored.todos_view_enabled);

        let mut queued_messages = restored.queued_messages;
        let mut recovered_followups = Vec::new();
        if let Some(interleave_message) = restored.interleave_message
            && !interleave_message.trim().is_empty()
        {
            recovered_followups.push(interleave_message);
        }
        let recovered_interrupts = restored
            .pending_soft_interrupt_resend
            .unwrap_or(restored.pending_soft_interrupts);
        if !recovered_interrupts.is_empty() {
            crate::logging::info(&format!(
                "Recovered {} pending soft interrupt(s) after reload; re-queueing them as normal follow-ups",
                recovered_interrupts.len()
            ));
            recovered_followups.extend(recovered_interrupts);
        }
        if !recovered_followups.is_empty() {
            let mut recovered_queue = recovered_followups;
            recovered_queue.append(&mut queued_messages);
            queued_messages = recovered_queue;
            self.set_status_notice("Recovered pending prompts after reload");
        }

        self.queued_messages = queued_messages;
        if self.has_queued_followups() {
            if self.is_remote {
                // Do not synthesize a processing turn for restored remote follow-ups.
                // After a reload, the server may still be running the previous turn;
                // the queue must remain a wait-until-turn-end queue until the history
                // bootstrap/Done event proves the remote turn is idle. The remote
                // post-connect/history/tick paths will dispatch once it is safe.
                self.set_status_notice("Restored queued follow-up after reload");
            } else {
                self.is_processing = true;
                self.status = ProcessingStatus::Sending;
                if self.processing_started.is_none() {
                    self.processing_started = Some(Instant::now());
                }
                self.pending_turn = true;
            }
        }
    }

    pub(super) async fn begin_remote_send(
        &mut self,
        remote: &mut backend::RemoteConnection,
        content: String,
        images: Vec<(String, String)>,
        is_system: bool,
    ) -> Result<u64> {
        remote::begin_remote_send(self, remote, content, images, is_system, None, false, 0).await
    }

    pub(super) fn schedule_pending_remote_retry(&mut self, reason: &str) -> bool {
        self.schedule_pending_remote_retry_with_limit(reason, Self::AUTO_RETRY_MAX_ATTEMPTS)
    }

    pub(super) fn schedule_pending_remote_network_wait(&mut self, reason: &str) -> bool {
        self.schedule_pending_remote_network_wait_with_force(reason, false)
    }

    /// Hold the in-flight remote turn until the network recovers, then resume it.
    ///
    /// Connectivity failures (DNS, connection reset, no route, transient TLS,
    /// timeouts) are always transient: the request never reached the provider,
    /// so resending after the network comes back is both safe and correct. When
    /// `force` is set we wait regardless of the pending message's `auto_retry`
    /// flag and promote it to auto-retry so the tick-based resume re-sends it.
    /// This prevents a transient disconnect from being misclassified as a
    /// permanent, non-retryable failure that stops auto-poke.
    pub(super) fn schedule_pending_remote_network_wait_with_force(
        &mut self,
        reason: &str,
        force: bool,
    ) -> bool {
        let Some(pending) = self.rate_limit_pending_message.as_mut() else {
            return false;
        };
        if !pending.auto_retry {
            if force {
                pending.auto_retry = true;
            } else {
                return false;
            }
        }

        let plan = crate::network_retry::wait_plan();
        let retry_at = Instant::now() + Duration::from_secs(5);
        pending.retry_at = Some(retry_at);
        self.rate_limit_reset = Some(retry_at);
        self.status = ProcessingStatus::WaitingForNetwork {
            listener: plan.listener_summary.clone(),
        };
        self.status_detail = Some("offline; waiting for network before retry".to_string());

        let content = format!(
            "📡 Network appears offline - waiting to retry automatically. {} - {}",
            plan.listener_summary,
            reason.trim().trim_end_matches('.')
        );
        if let Some(idx) = self.display_messages.iter().rposition(|message| {
            message.role == "system"
                && (message.title.as_deref() == Some("Connection")
                    || message.content.starts_with("📡 Network appears offline"))
        }) {
            self.replace_display_message_title_and_content(
                idx,
                Some("Connection".to_string()),
                content,
            );
        } else {
            self.push_display_message(DisplayMessage {
                role: "system".to_string(),
                content,
                tool_calls: Vec::new(),
                duration_secs: None,
                title: Some("Connection".to_string()),
                tool_data: None,
            });
        }
        true
    }

    pub(super) fn schedule_pending_remote_retry_with_limit(
        &mut self,
        reason: &str,
        max_attempts: u8,
    ) -> bool {
        let Some(pending) = self.rate_limit_pending_message.as_mut() else {
            return false;
        };
        if !pending.auto_retry {
            return false;
        }
        let outcome = {
            let current_attempts = pending.retry_attempts;
            if current_attempts >= max_attempts {
                Err(current_attempts)
            } else {
                pending.retry_attempts += 1;
                let retry_attempts = pending.retry_attempts;
                let backoff_secs = Self::AUTO_RETRY_BASE_DELAY_SECS * u64::from(retry_attempts);
                let retry_at = Instant::now() + Duration::from_secs(backoff_secs);
                pending.retry_at = Some(retry_at);
                Ok((retry_attempts, backoff_secs, retry_at))
            }
        };

        match outcome {
            Err(current_attempts) => {
                self.rate_limit_pending_message = None;
                self.rate_limit_reset = None;
                self.push_display_message(DisplayMessage::error(format!(
                    "{} Auto-retry limit reached after {} attempt{}. Use `/poke` again to retry manually.",
                    reason,
                    current_attempts,
                    if current_attempts == 1 { "" } else { "s" }
                )));
                false
            }
            Ok((retry_attempts, backoff_secs, retry_at)) => {
                self.rate_limit_reset = Some(retry_at);
                let content = format!(
                    "⚡ Connection lost - retrying (attempt {}/{}, in {}s) - {}",
                    retry_attempts,
                    max_attempts,
                    backoff_secs,
                    reason
                        .trim()
                        .trim_start_matches("⚡ ")
                        .trim_start_matches("Connection lost")
                        .trim_start_matches('(')
                        .trim_end_matches('.')
                        .trim()
                );
                if let Some(idx) = self.display_messages.iter().rposition(|message| {
                    message.role == "system"
                        && (message.title.as_deref() == Some("Connection")
                            || message
                                .content
                                .starts_with("⚡ Server reload in progress - waiting for handoff")
                            || message.content.starts_with("⚡ Connection lost"))
                }) {
                    self.replace_display_message_title_and_content(
                        idx,
                        Some("Connection".to_string()),
                        content,
                    );
                } else {
                    self.push_display_message(DisplayMessage {
                        role: "system".to_string(),
                        content,
                        tool_calls: Vec::new(),
                        duration_secs: None,
                        title: Some("Connection".to_string()),
                        tool_data: None,
                    });
                }
                true
            }
        }
    }

    pub(super) fn clear_pending_remote_retry(&mut self) {
        self.rate_limit_pending_message = None;
        self.rate_limit_reset = None;
    }

    /// Track a failed turn for the credential-failure circuit breaker.
    ///
    /// Returns `true` when the error classifies as a credential/auth failure
    /// AND the consecutive-failure count has reached the breaker threshold,
    /// meaning the caller must stop all automatic resend paths. Non-credential
    /// errors reset the streak (the breaker only guards against retrying a
    /// dead credential, not mixed transient failures).
    pub(super) fn note_error_for_credential_breaker(&mut self, message: &str) -> bool {
        if crate::provider::error_looks_like_credential_failure(message) {
            self.consecutive_credential_failures =
                self.consecutive_credential_failures.saturating_add(1);
            self.consecutive_credential_failures >= Self::CREDENTIAL_FAILURE_BREAKER_THRESHOLD
        } else {
            self.consecutive_credential_failures = 0;
            false
        }
    }

    /// Reset the credential-failure streak. Called when a turn completes
    /// successfully or the user changes auth (login, provider/model switch),
    /// so a fixed credential gets a fresh retry budget.
    pub(super) fn reset_credential_failure_breaker(&mut self) {
        self.consecutive_credential_failures = 0;
    }

    /// Hard-stop every automatic resend path because the session has hit
    /// repeated credential/auth failures. Retrying the identical request
    /// against a dead credential can never succeed; before this breaker,
    /// auto-poke/queued-retry loops logged thousands of 401s in a single
    /// session (one failed turn per resend) until the user noticed.
    pub(super) fn trip_credential_failure_breaker(&mut self, message: &str) {
        let failures = self.consecutive_credential_failures;
        self.clear_pending_remote_retry();
        let cleared_pokes = if self.auto_poke_incomplete_todos {
            super::commands::disable_auto_poke(self)
        } else {
            0
        };
        self.overnight_auto_poke = None;

        // Surface the streak in telemetry as an explicit auth_failed event so
        // the dashboard can distinguish "breaker tripped on a dead credential"
        // from one-off auth blips.
        let reason = crate::auth::login_diagnostics::classify_auth_failure_message(message);
        let provider = self.provider_name().to_string();
        crate::telemetry::record_auth_failed_reason(&provider, "session", reason.label());

        self.push_display_message(DisplayMessage::error(format!(
            "🛑 Stopped automatic retries: {failures} consecutive credential/auth failures. \
             The current login or API key for {provider} is not working, so resending the same \
             request cannot succeed.{} Run /login to re-authenticate (or /model to switch to a \
             working route), then send again.",
            if cleared_pokes == 0 {
                String::new()
            } else {
                format!(" Cleared {cleared_pokes} queued auto-poke follow-up(s).")
            }
        )));
        self.set_status_notice("Stopped: repeated auth failures");
        self.restore_failed_input_to_box();
        self.consecutive_credential_failures = 0;
    }
}
