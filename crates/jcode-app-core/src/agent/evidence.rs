use super::*;
use jcode_session_types::{
    CorrelationIds, GitSnapshot, PayloadSummary, SessionLogEventKind, SessionLogStatus,
};
use std::fmt;
use uuid::Uuid;

impl Agent {
    pub(super) fn start_evidence_turn(
        &mut self,
        user_message: &str,
        image_count: usize,
        user_message_index: usize,
    ) {
        let turn_id = Uuid::new_v4().to_string();
        self.current_evidence_turn_id = Some(turn_id.clone());
        self.append_session_evidence_with_correlation(
            SessionLogEventKind::TurnStarted {
                user_message_index,
                image_count,
                input: Some(crate::session::payload_summary_text(
                    user_message,
                    Some("text/plain".to_string()),
                )),
            },
            CorrelationIds {
                turn_id: Some(turn_id),
                ..CorrelationIds::default()
            },
        );
    }

    pub(super) fn finish_evidence_turn(
        &mut self,
        result: &Result<impl Sized>,
        started_at: Instant,
        output: Option<&str>,
    ) {
        let status = status_for_result(result);
        let correlation = self.current_turn_evidence_correlation();
        self.append_session_evidence_with_correlation(
            SessionLogEventKind::TurnFinished {
                status,
                duration_ms: started_at.elapsed().as_millis() as u64,
                output: output.map(|text| {
                    crate::session::payload_summary_text(text, Some("text/plain".to_string()))
                }),
                error_class: result.as_ref().err().map(error_class_for_error),
            },
            correlation,
        );
        self.record_assistant_checkpoint(status);
        self.current_evidence_turn_id = None;
    }

    /// Auto-populate the assistant session's `last_checkpoint` from this
    /// in-process turn-end seam (AA-48 safe subset).
    ///
    /// Previously `AssistantSessionMeta.last_checkpoint` was inert: chrome and
    /// `/assistant status` only displayed it, nothing wrote it. Here we derive a
    /// deterministic recovery breadcrumb (UTC time, turn status, short git head)
    /// at the same seam that closes the evidence turn, and persist it. No-op for
    /// non-assistant sessions, so plain sessions are unchanged.
    fn record_assistant_checkpoint(&mut self, status: SessionLogStatus) {
        if self.session.assistant.is_none() {
            return;
        }
        let checkpoint = self.build_assistant_checkpoint(status);
        if let Some(meta) = self.session.assistant.as_mut() {
            meta.last_checkpoint = Some(checkpoint);
        }
        self.persist_session_best_effort("assistant turn checkpoint");
    }

    /// Build the deterministic checkpoint summary string. Kept separate so it is
    /// unit-testable without a running turn.
    fn build_assistant_checkpoint(&self, status: SessionLogStatus) -> String {
        let when = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let status_label = match status {
            SessionLogStatus::Ok => "ok",
            SessionLogStatus::Error => "error",
            SessionLogStatus::Cancelled => "cancelled",
            SessionLogStatus::Interrupted => "interrupted",
        };
        match self
            .current_git_snapshot()
            .and_then(|snapshot| snapshot.head)
        {
            Some(head) => {
                let short: String = head.chars().take(8).collect();
                format!("turn {status_label} @ {when} (git {short})")
            }
            None => format!("turn {status_label} @ {when}"),
        }
    }

    pub(super) fn append_session_evidence_with_correlation(
        &self,
        kind: SessionLogEventKind,
        correlation: CorrelationIds,
    ) {
        let mut writer = match crate::session::SessionEvidenceWriter::for_session(
            self.session.id.clone(),
            self.session_evidence_context(correlation),
        ) {
            Ok(writer) => writer,
            Err(err) => {
                logging::warn(&format!("Failed to prepare session evidence writer: {err}"));
                return;
            }
        };
        if let Err(err) = writer.append(kind) {
            logging::warn(&format!(
                "Failed to append session evidence to {}: {err}",
                writer.path().display()
            ));
        }
    }

    pub(super) fn current_turn_evidence_correlation(&self) -> CorrelationIds {
        CorrelationIds {
            turn_id: self.current_evidence_turn_id.clone(),
            ..CorrelationIds::default()
        }
    }

    pub(super) fn tool_evidence_correlation(&self, tool_call_id: &str) -> CorrelationIds {
        CorrelationIds {
            turn_id: self.current_evidence_turn_id.clone(),
            tool_call_id: Some(tool_call_id.to_string()),
            ..CorrelationIds::default()
        }
    }

    pub(super) fn provider_evidence_correlation(&self) -> CorrelationIds {
        CorrelationIds {
            turn_id: self.current_evidence_turn_id.clone(),
            provider_request_id: Some(Uuid::new_v4().to_string()),
            ..CorrelationIds::default()
        }
    }

    pub(super) fn append_provider_error_response(
        &self,
        provider_name: &str,
        provider_model: String,
        started_at: Instant,
        _error: &anyhow::Error,
        error_class: EvidenceErrorClass,
        correlation: CorrelationIds,
    ) {
        self.append_session_evidence_with_correlation(
            SessionLogEventKind::ProviderResponse {
                provider: provider_name.to_string(),
                model: provider_model,
                status: SessionLogStatus::Error,
                duration_ms: started_at.elapsed().as_millis() as u64,
                output: None,
                usage: None,
                error_class: Some(error_class.as_str().to_string()),
            },
            correlation,
        );
    }

    pub(super) fn classified_evidence_error(
        error: anyhow::Error,
        error_class: EvidenceErrorClass,
    ) -> anyhow::Error {
        anyhow::Error::new(ClassifiedEvidenceError { error_class, error })
    }

    pub(super) fn interrupted_turn_error() -> anyhow::Error {
        anyhow::Error::new(TurnInterruptedError)
    }

    /// Test-only constructor so server-side fixtures can produce the real
    /// typed interruption without exposing the private marker type.
    #[cfg(test)]
    pub(crate) fn interrupted_turn_error_for_tests() -> anyhow::Error {
        Self::interrupted_turn_error()
    }

    /// Typed predicate for turn interruption, usable by server consumers
    /// without string comparison. Traverses the whole `anyhow` chain so
    /// wrapped interruptions (for example `context(...)` layers or
    /// `ClassifiedEvidenceError` wrappers) are still recognized, while
    /// lookalike string errors are not.
    pub(crate) fn error_is_turn_interruption(error: &anyhow::Error) -> bool {
        classify_evidence_error(error) == EvidenceErrorClass::TurnInterrupted
    }

    pub(super) fn evidence_payload_json<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
    ) -> Option<PayloadSummary> {
        serde_json::to_vec(value).ok().map(|bytes| {
            crate::session::payload_summary_bytes(
                &bytes,
                Some("application/json".to_string()),
                None,
            )
        })
    }

    fn session_evidence_context(
        &self,
        correlation: CorrelationIds,
    ) -> crate::session::SessionEvidenceContext {
        crate::session::SessionEvidenceContext::local(
            self.session.working_dir.clone(),
            self.current_git_snapshot(),
        )
        .with_correlation(correlation)
    }

    fn current_git_snapshot(&self) -> Option<GitSnapshot> {
        let dir = self.session.working_dir.as_deref()?;
        let state = super::environment::cached_git_state_for_dir(
            std::path::Path::new(dir),
            super::utils::git_state_for_dir,
        )?;
        Some(GitSnapshot {
            root: state.root,
            head: state.head,
            branch: state.branch,
            dirty: state.dirty,
        })
    }
}

fn status_for_result<T>(result: &Result<T>) -> SessionLogStatus {
    match result {
        Ok(_) => SessionLogStatus::Ok,
        Err(error) if error.downcast_ref::<TurnInterruptedError>().is_some() => {
            SessionLogStatus::Interrupted
        }
        Err(_) => SessionLogStatus::Error,
    }
}

#[derive(Debug)]
struct TurnInterruptedError;

impl fmt::Display for TurnInterruptedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("turn interrupted")
    }
}

impl std::error::Error for TurnInterruptedError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EvidenceErrorClass {
    ContextLimit,
    ProviderOpen,
    StreamTransport,
    StreamEvent,
    TurnInterrupted,
    Unknown,
}

impl EvidenceErrorClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::ContextLimit => "context_limit",
            Self::ProviderOpen => "provider_open_error",
            Self::StreamTransport => "stream_transport_error",
            Self::StreamEvent => "stream_error",
            Self::TurnInterrupted => "turn_interrupted",
            Self::Unknown => "unknown_error",
        }
    }
}

#[derive(Debug)]
struct ClassifiedEvidenceError {
    error_class: EvidenceErrorClass,
    error: anyhow::Error,
}

impl fmt::Display for ClassifiedEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.error)
    }
}

impl std::error::Error for ClassifiedEvidenceError {
    /// Expose the wrapped error so `anyhow` chain traversal and downcast
    /// walks (for example `StreamError::retry_after_secs` recovery) keep
    /// working through the classification wrapper (W7a).
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

pub(super) fn error_class_for_error(error: &anyhow::Error) -> String {
    classify_evidence_error(error).as_str().to_string()
}

fn classify_evidence_error(error: &anyhow::Error) -> EvidenceErrorClass {
    for cause in error.chain() {
        if cause.downcast_ref::<TurnInterruptedError>().is_some() {
            return EvidenceErrorClass::TurnInterrupted;
        }
        if let Some(error) = cause.downcast_ref::<ClassifiedEvidenceError>() {
            return error.error_class;
        }
        if cause.downcast_ref::<StreamError>().is_some() {
            return EvidenceErrorClass::StreamEvent;
        }
    }
    EvidenceErrorClass::Unknown
}

#[cfg(test)]
mod w7a_error_semantics_tests {
    use super::*;
    use anyhow::Context as _;

    /// A lookalike error whose display text matches the interruption marker
    /// but whose type does not. The typed predicate must reject it.
    #[derive(Debug)]
    struct LookalikeInterruption;

    impl fmt::Display for LookalikeInterruption {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("turn interrupted")
        }
    }

    impl std::error::Error for LookalikeInterruption {}

    #[test]
    fn interruption_predicate_accepts_direct_marker() {
        let error = Agent::interrupted_turn_error();
        assert!(Agent::error_is_turn_interruption(&error));
    }

    #[test]
    fn interruption_predicate_accepts_context_wrapped_marker() {
        let error = Agent::interrupted_turn_error()
            .context("while streaming")
            .context("outer turn layer");
        assert!(
            Agent::error_is_turn_interruption(&error),
            "predicate must traverse the anyhow context chain"
        );
    }

    #[test]
    fn interruption_predicate_accepts_classified_wrapped_marker() {
        let error = Agent::classified_evidence_error(
            Agent::interrupted_turn_error(),
            EvidenceErrorClass::TurnInterrupted,
        );
        assert!(Agent::error_is_turn_interruption(&error));
    }

    #[test]
    fn interruption_predicate_rejects_lookalike_string_error() {
        let error = anyhow::Error::new(LookalikeInterruption);
        assert_eq!(error.to_string(), "turn interrupted");
        assert!(
            !Agent::error_is_turn_interruption(&error),
            "string-equal display text must not satisfy the typed predicate"
        );
    }

    #[test]
    fn interruption_predicate_rejects_plain_and_stream_errors() {
        assert!(!Agent::error_is_turn_interruption(&anyhow::anyhow!(
            "turn interrupted"
        )));
        let stream = anyhow::Error::new(StreamError::new("stream broke".into(), Some(7)));
        assert!(!Agent::error_is_turn_interruption(&stream));
    }

    #[test]
    fn classified_error_source_exposes_inner_chain() {
        let inner = anyhow::Error::new(StreamError::new("rate limited".into(), Some(42)));
        let wrapped = Agent::classified_evidence_error(inner, EvidenceErrorClass::StreamEvent);

        let classified = wrapped
            .downcast_ref::<ClassifiedEvidenceError>()
            .expect("outer classified wrapper");
        let source = std::error::Error::source(classified)
            .expect("W7a: ClassifiedEvidenceError must expose its inner error");
        assert_eq!(source.to_string(), "rate limited");
    }

    #[test]
    fn classified_error_chain_preserves_retry_after_secs() {
        let inner = anyhow::Error::new(StreamError::new("rate limited".into(), Some(42)));
        let wrapped = Agent::classified_evidence_error(inner, EvidenceErrorClass::StreamEvent);

        let retry = wrapped
            .chain()
            .find_map(|cause| cause.downcast_ref::<StreamError>())
            .and_then(|stream_error| stream_error.retry_after_secs);
        assert_eq!(
            retry,
            Some(42),
            "retry metadata must survive classification wrapping via source()"
        );
    }

    #[test]
    fn classification_still_reads_class_through_wrapping() {
        let inner = anyhow::Error::new(StreamError::new("boom".into(), None));
        let wrapped = Agent::classified_evidence_error(inner, EvidenceErrorClass::ProviderOpen)
            .context("outer layer");
        assert_eq!(error_class_for_error(&wrapped), "provider_open_error");
    }
}
