use super::*;
use jcode_session_types::{
    CorrelationIds, GitSnapshot, PayloadSummary, SessionLogEventKind, SessionLogStatus,
};
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
                error_class: result.as_ref().err().map(error_class),
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
    if result.is_ok() {
        SessionLogStatus::Ok
    } else {
        SessionLogStatus::Error
    }
}

fn error_class(error: &anyhow::Error) -> String {
    error
        .chain()
        .last()
        .map(|cause| {
            cause
                .to_string()
                .split(':')
                .next()
                .unwrap_or("error")
                .trim()
                .chars()
                .take(120)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "error".to_string())
}
