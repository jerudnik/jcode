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
        let correlation = self.current_turn_evidence_correlation();
        self.append_session_evidence_with_correlation(
            SessionLogEventKind::TurnFinished {
                status: status_for_result(result),
                duration_ms: started_at.elapsed().as_millis() as u64,
                output: output.map(|text| {
                    crate::session::payload_summary_text(text, Some("text/plain".to_string()))
                }),
                error_class: result.as_ref().err().map(|error| error_class(error)),
            },
            correlation,
        );
        self.current_evidence_turn_id = None;
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
