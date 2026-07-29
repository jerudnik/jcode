use super::*;

impl App {
    pub(super) fn track_pending_soft_interrupt(&mut self, request_id: u64, content: String) {
        let content_bytes = content.len();
        let content_chars = content.chars().count();
        self.pending_soft_interrupt_requests
            .push((request_id, content.clone()));
        self.pending_soft_interrupts.push(content);
        crate::logging::info(&format!(
            "REMOTE_SOFT_INTERRUPT_TRACK_PENDING id={} content_bytes={} content_chars={} pending_requests={} pending_messages={}",
            request_id,
            content_bytes,
            content_chars,
            self.pending_soft_interrupt_requests.len(),
            self.pending_soft_interrupts.len()
        ));
    }

    pub(super) fn acknowledge_pending_soft_interrupt(&mut self, request_id: u64) -> bool {
        if let Some(index) = self
            .pending_soft_interrupt_requests
            .iter()
            .position(|(id, _)| *id == request_id)
        {
            self.pending_soft_interrupt_requests.remove(index);
            crate::logging::info(&format!(
                "REMOTE_SOFT_INTERRUPT_ACK_MATCHED id={} pending_requests={} pending_messages={}",
                request_id,
                self.pending_soft_interrupt_requests.len(),
                self.pending_soft_interrupts.len()
            ));
            true
        } else {
            if !self.pending_soft_interrupt_requests.is_empty() {
                crate::logging::info(&format!(
                    "REMOTE_SOFT_INTERRUPT_ACK_UNMATCHED id={} pending_requests={} pending_messages={}",
                    request_id,
                    self.pending_soft_interrupt_requests.len(),
                    self.pending_soft_interrupts.len()
                ));
            }
            false
        }
    }

    pub(super) fn clear_pending_soft_interrupt_tracking(&mut self) {
        crate::logging::info(&format!(
            "REMOTE_SOFT_INTERRUPT_TRACKING_CLEAR pending_requests={} pending_messages={}",
            self.pending_soft_interrupt_requests.len(),
            self.pending_soft_interrupts.len()
        ));
        self.pending_soft_interrupts.clear();
        self.pending_soft_interrupt_requests.clear();
    }

    pub(super) fn mark_soft_interrupt_injected(&mut self, content: &str) {
        crate::logging::info(&format!(
            "REMOTE_SOFT_INTERRUPT_MARK_INJECTED content_bytes={} content_chars={} pending_requests={} pending_messages={}",
            content.len(),
            content.chars().count(),
            self.pending_soft_interrupt_requests.len(),
            self.pending_soft_interrupts.len()
        ));
        if self.mark_combined_soft_interrupt_injected(content) {
            return;
        }

        if let Some(index) = self
            .pending_soft_interrupts
            .iter()
            .position(|pending| pending == content)
        {
            self.pending_soft_interrupts.remove(index);
        }

        if let Some(index) = self
            .pending_soft_interrupt_requests
            .iter()
            .position(|(_, pending)| pending == content)
        {
            self.pending_soft_interrupt_requests.remove(index);
        }
    }

    fn mark_combined_soft_interrupt_injected(&mut self, content: &str) -> bool {
        let mut combined = String::new();
        for (index, pending) in self.pending_soft_interrupts.iter().enumerate() {
            if index > 0 {
                combined.push_str("\n\n");
            }
            combined.push_str(pending);

            if combined == content {
                let count = index + 1;
                let removed: Vec<String> = self.pending_soft_interrupts.drain(..count).collect();
                for removed_content in removed {
                    if let Some(request_index) = self
                        .pending_soft_interrupt_requests
                        .iter()
                        .position(|(_, pending)| pending == &removed_content)
                    {
                        self.pending_soft_interrupt_requests.remove(request_index);
                    }
                }
                return true;
            }

            if !content.starts_with(&combined) {
                break;
            }
        }

        false
    }
}

/// Recover an in-flight queued continuation back into the queue.
///
/// A queued follow-up that was already taken from `queued_messages` and handed
/// to `begin_remote_send` lives only in `rate_limit_pending_message` while it
/// is in flight. That pending shape (`is_system` with `auto_retry == false`)
/// has no retry path: the tick resend requires a rate-limit reset timestamp
/// and the disconnect resend requires `auto_retry`. If the connection dies
/// before the turn completes (typically a server reload handoff racing the
/// dispatch), clearing the pending message silently drops the user's queued
/// message (issue #391). Instead, put it back at the front of the queue so it
/// is re-sent once the turn is proven idle after reconnect, which is the
/// queue's contract.
pub(super) fn recover_undelivered_queued_continuation(app: &mut App, reason: &str) -> bool {
    let is_recoverable = app
        .rate_limit_pending_message
        .as_ref()
        .is_some_and(|pending| {
            pending.is_system
                && !pending.auto_retry
                && (!pending.content.trim().is_empty() || pending.system_reminder.is_some())
        });
    if !is_recoverable {
        return false;
    }
    let Some(pending) = app.rate_limit_pending_message.take() else {
        return false;
    };
    app.rate_limit_reset = None;
    crate::logging::info(&format!(
        "Recovering in-flight queued continuation into queued follow-ups after {} (content_chars={}, has_reminder={})",
        reason,
        pending.content.chars().count(),
        pending.system_reminder.is_some()
    ));
    if let Some(reminder) = pending.system_reminder {
        app.hidden_queued_system_messages.insert(0, reminder);
    }
    if !pending.content.trim().is_empty() {
        app.queued_messages.insert(0, pending.content);
    }
    true
}

pub(super) fn recover_local_interleave_to_queue(app: &mut App, reason: &str) -> bool {
    let Some(interleave) = app.interleave_message.take() else {
        return false;
    };
    if interleave.trim().is_empty() {
        return false;
    }

    crate::logging::info(&format!(
        "Recovering unsent interleave into queued follow-ups after {}",
        reason
    ));
    app.queued_messages.insert(0, interleave);
    true
}

pub(super) async fn recover_stranded_soft_interrupts(
    app: &mut App,
    remote: &mut RemoteConnection,
) -> bool {
    if app.is_processing || app.pending_soft_interrupts.is_empty() {
        return false;
    }

    let recovered_interrupts = std::mem::take(&mut app.pending_soft_interrupts);
    if recovered_interrupts.is_empty() {
        return false;
    }

    if let Err(err) = remote.cancel_soft_interrupts().await {
        app.pending_soft_interrupts = recovered_interrupts;
        app.push_display_message(DisplayMessage::error(format!(
            "Failed to recover queued interleave message: {}",
            err
        )));
        app.set_status_notice("Queued interleave recovery failed");
        return false;
    }

    crate::logging::info(&format!(
        "Recovering {} stranded soft interrupt(s) into queued follow-ups after turn boundary",
        recovered_interrupts.len()
    ));
    app.pending_soft_interrupt_requests.clear();

    // Recovery must never introduce a *copy* of a message that is already
    // waiting in the queue. Under multi-client contention the same turn can be
    // interrupted repeatedly (each client's stall guard cancels the other's
    // turn), and each interrupt re-runs this recovery over soft interrupts that
    // were already recovered into `queued_messages` but not yet dispatched.
    // Blindly prepending them replays one user message N times (18x observed in
    // the 2026-07-20 multi-client incident).
    //
    // This dedups on *recovery provenance only*: a recovered interrupt is
    // dropped solely because an identical copy is already queued. Messages the
    // user genuinely typed twice are pushed onto `queued_messages` directly by
    // the input path and are never filtered here, so intentional repeats still
    // deliver twice.
    let already_queued = app.queued_messages.clone();
    let mut recovered_queue: Vec<String> = Vec::with_capacity(recovered_interrupts.len());
    let mut dropped = 0usize;
    for interrupt in recovered_interrupts {
        let duplicate_of_queued = already_queued.contains(&interrupt);
        let duplicate_of_recovered = recovered_queue.contains(&interrupt);
        if duplicate_of_queued || duplicate_of_recovered {
            dropped += 1;
            crate::logging::info(&format!(
                "REMOTE_SOFT_INTERRUPT_RECOVERY_DEDUP content_chars={} already_queued={} already_recovered={}",
                interrupt.chars().count(),
                duplicate_of_queued,
                duplicate_of_recovered
            ));
            continue;
        }
        recovered_queue.push(interrupt);
    }
    if dropped > 0 {
        crate::logging::warn(&format!(
            "Dropped {} duplicate stranded soft interrupt(s) during recovery; {} queued",
            dropped,
            recovered_queue.len()
        ));
    }
    recovered_queue.append(&mut app.queued_messages);
    app.queued_messages = recovered_queue;
    app.set_status_notice("Recovered queued interleave after turn finished");
    true
}

#[cfg(test)]
mod tests {
    use crate::provider::Provider;
    use std::sync::Arc;

    struct MockProvider;

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        async fn complete(
            &self,
            _messages: &[crate::message::Message],
            _tools: &[crate::message::ToolDefinition],
            _system: &str,
            _resume_session_id: Option<&str>,
        ) -> anyhow::Result<crate::provider::EventStream> {
            Err(anyhow::anyhow!(
                "mock provider must not stream in queue recovery tests"
            ))
        }

        fn name(&self) -> &str {
            "mock"
        }

        fn fork(&self) -> Arc<dyn Provider> {
            Arc::new(Self)
        }
    }

    fn test_app() -> crate::tui::app::App {
        let provider: Arc<dyn Provider> = Arc::new(MockProvider);
        crate::tui::app::test_support::create_test_app_with(provider, |app| {
            app.queue_mode = false;
            app.onboarding_flow = None;
        })
    }

    /// R05 gate: "N identical queued user messages deliver once, not N times."
    ///
    /// Under multi-client contention the same turn is interrupted repeatedly,
    /// and each interrupt re-runs stranded-soft-interrupt recovery over content
    /// that a previous recovery already placed on `queued_messages`. Recovery
    /// must not append another copy (18x replay observed in the 2026-07-20
    /// incident); the message is delivered once at the turn boundary.
    #[test]
    fn recovery_does_not_requeue_a_message_already_queued() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let _guard = rt.enter();
        let mut app = test_app();
        let mut remote = crate::tui::backend::RemoteConnection::dummy();

        // A prior recovery already moved this content onto the queue.
        app.queued_messages = vec!["retry the failing test".to_string()];
        // The same content is still tracked as a stranded soft interrupt, as it
        // is after a cancel that raced the server-side ack.
        app.pending_soft_interrupts = vec!["retry the failing test".to_string()];
        app.is_processing = false;

        let recovered = rt.block_on(super::recover_stranded_soft_interrupts(
            &mut app,
            &mut remote,
        ));

        assert!(
            recovered,
            "recovery should run and consume the stranded item"
        );
        assert_eq!(
            app.queued_messages,
            vec!["retry the failing test".to_string()],
            "recovery must not add a second copy of an already-queued message"
        );
        assert!(
            app.pending_soft_interrupts.is_empty(),
            "stranded tracking must be drained"
        );
    }

    /// Repeated interrupts must converge, not accumulate: running recovery many
    /// times over the same content yields exactly one queued copy.
    #[test]
    fn repeated_recovery_cycles_do_not_accumulate_duplicates() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let _guard = rt.enter();
        let mut app = test_app();
        let mut remote = crate::tui::backend::RemoteConnection::dummy();
        app.is_processing = false;

        for _ in 0..18 {
            app.pending_soft_interrupts = vec!["keep going".to_string()];
            let _ = rt.block_on(super::recover_stranded_soft_interrupts(
                &mut app,
                &mut remote,
            ));
        }

        assert_eq!(
            app.queued_messages,
            vec!["keep going".to_string()],
            "18 recovery cycles must leave exactly one copy queued"
        );
    }

    /// The collapse keys on recovery provenance only. A message the user
    /// genuinely typed twice is pushed onto `queued_messages` by the input
    /// path, never filtered here, so intentional repeats still deliver twice.
    #[test]
    fn user_typed_duplicates_are_not_collapsed() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let _guard = rt.enter();
        let mut app = test_app();
        let mut remote = crate::tui::backend::RemoteConnection::dummy();
        app.is_processing = false;

        // The user deliberately queued the same prompt twice.
        app.queued_messages = vec!["ping".to_string(), "ping".to_string()];
        // An unrelated stranded interrupt triggers a recovery pass.
        app.pending_soft_interrupts = vec!["and then stop".to_string()];

        let _ = rt.block_on(super::recover_stranded_soft_interrupts(
            &mut app,
            &mut remote,
        ));

        assert_eq!(
            app.queued_messages,
            vec![
                "and then stop".to_string(),
                "ping".to_string(),
                "ping".to_string()
            ],
            "recovery must not collapse messages the user intentionally repeated"
        );
    }
}
