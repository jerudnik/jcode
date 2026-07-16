use anyhow::Result;
use chrono::Utc;
use jcode_session_types::{
    CorrelationIds, GitSnapshot, NodeSnapshot, PayloadSummary, SESSION_LOG_EVENT_SCHEMA_VERSION,
    SessionLogEvent, SessionLogEventKind,
};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::storage_paths::{session_evidence_path, session_evidence_path_from_snapshot};
use crate::storage;

#[derive(Debug, Clone)]
pub struct SessionEvidenceContext {
    pub parent_session_id: Option<String>,
    pub child_session_id: Option<String>,
    pub node: NodeSnapshot,
    pub git: Option<GitSnapshot>,
    pub correlation: CorrelationIds,
}

impl SessionEvidenceContext {
    pub fn local(working_dir: Option<String>, git: Option<GitSnapshot>) -> Self {
        Self {
            parent_session_id: None,
            child_session_id: None,
            node: local_node_snapshot(working_dir),
            git,
            correlation: CorrelationIds::default(),
        }
    }

    pub fn with_correlation(mut self, correlation: CorrelationIds) -> Self {
        self.correlation = correlation;
        self
    }
}

#[derive(Debug, Clone)]
pub struct SessionEvidenceWriter {
    session_id: String,
    path: PathBuf,
    next_sequence: u64,
    context: SessionEvidenceContext,
}

impl SessionEvidenceWriter {
    pub fn for_session(
        session_id: impl Into<String>,
        context: SessionEvidenceContext,
    ) -> Result<Self> {
        let session_id = session_id.into();
        let path = session_evidence_path(&session_id)?;
        Self::for_path(session_id, path, context)
    }

    pub fn for_snapshot_path(
        session_id: impl Into<String>,
        snapshot_path: &Path,
        context: SessionEvidenceContext,
    ) -> Result<Self> {
        let path = session_evidence_path_from_snapshot(snapshot_path);
        Self::for_path(session_id.into(), path, context)
    }

    pub fn for_path(
        session_id: impl Into<String>,
        path: PathBuf,
        context: SessionEvidenceContext,
    ) -> Result<Self> {
        let session_id = session_id.into();
        let next_sequence = next_sequence_for_evidence_path(&path)?;
        Ok(Self {
            session_id,
            path,
            next_sequence,
            context,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, kind: SessionLogEventKind) -> Result<SessionLogEvent> {
        let event = SessionLogEvent {
            schema_version: SESSION_LOG_EVENT_SCHEMA_VERSION,
            event_id: Uuid::new_v4().to_string(),
            sequence: self.next_sequence,
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            parent_session_id: self.context.parent_session_id.clone(),
            child_session_id: self.context.child_session_id.clone(),
            node: self.context.node.clone(),
            git: self.context.git.clone(),
            correlation: self.context.correlation.clone(),
            kind,
        };
        storage::append_json_line_fast(&self.path, &event)?;
        self.next_sequence += 1;
        Ok(event)
    }
}

pub fn read_session_evidence(session_id: &str) -> Result<Vec<SessionLogEvent>> {
    read_session_evidence_from_path(&session_evidence_path(session_id)?)
}

pub fn read_session_evidence_for_snapshot(path: &Path) -> Result<Vec<SessionLogEvent>> {
    read_session_evidence_from_path(&session_evidence_path_from_snapshot(path))
}

pub fn read_session_evidence_from_path(path: &Path) -> Result<Vec<SessionLogEvent>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<SessionLogEvent>(trimmed) {
            Ok(event) => events.push(event),
            Err(err) => {
                crate::logging::warn(&format!(
                    "Session evidence parse stopped at {} line {}: {}",
                    path.display(),
                    line_idx + 1,
                    err
                ));
                break;
            }
        }
    }
    events.sort_by_key(|event| event.sequence);
    Ok(events)
}

pub fn payload_summary_bytes(
    bytes: &[u8],
    media_type: Option<String>,
    artifact_path: Option<String>,
) -> PayloadSummary {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    PayloadSummary {
        sha256: hex::encode(hasher.finalize()),
        bytes: bytes.len() as u64,
        artifact_path,
        media_type,
    }
}

pub fn payload_summary_text(text: &str, media_type: Option<String>) -> PayloadSummary {
    payload_summary_bytes(text.as_bytes(), media_type, None)
}

fn next_sequence_for_evidence_path(path: &Path) -> Result<u64> {
    Ok(read_session_evidence_from_path(path)?
        .into_iter()
        .map(|event| event.sequence)
        .max()
        .map_or(0, |sequence| sequence.saturating_add(1)))
}

fn local_node_snapshot(working_dir: Option<String>) -> NodeSnapshot {
    NodeSnapshot {
        node_id: local_node_id(),
        hostname: std::env::var("HOSTNAME")
            .ok()
            .filter(|value| !value.trim().is_empty()),
        process_id: std::process::id(),
        jcode_version: jcode_build_meta::VERSION.to_string(),
        jcode_git_hash: Some(jcode_build_meta::GIT_HASH.to_string())
            .filter(|value| !value.trim().is_empty()),
        runtime: Some("jcode".to_string()),
        working_dir,
    }
}

fn local_node_id() -> String {
    std::env::var("JCODE_NODE_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "local".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcode_session_types::{RouteSelectionSource, SessionLogStatus};
    use std::io::Write;

    struct EnvGuard {
        key: &'static str,
        old: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let old = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, old }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                if let Some(old) = &self.old {
                    std::env::set_var(self.key, old);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    fn context() -> SessionEvidenceContext {
        SessionEvidenceContext::local(Some("/tmp/project".to_string()), None)
    }

    #[test]
    fn evidence_path_sits_next_to_session_snapshot() {
        let snapshot = Path::new("/tmp/jcode/sessions/session-1.json");
        assert_eq!(
            session_evidence_path_from_snapshot(snapshot),
            PathBuf::from("/tmp/jcode/sessions/session-1.evidence.jsonl")
        );
    }

    #[test]
    fn writer_appends_and_reader_orders_events() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("JCODE_HOME", temp.path());

        let mut writer = SessionEvidenceWriter::for_session("session-1", context()).unwrap();
        let first = writer
            .append(SessionLogEventKind::RouteSelected {
                provider_key: "openai".to_string(),
                model: "gpt-test".to_string(),
                api_method: Some("openai-api".to_string()),
                source: RouteSelectionSource::User,
            })
            .unwrap();
        let second = writer
            .append(SessionLogEventKind::TurnFinished {
                status: SessionLogStatus::Ok,
                duration_ms: 10,
                output: None,
                error_class: None,
            })
            .unwrap();

        assert_eq!(first.sequence, 0);
        assert_eq!(second.sequence, 1);
        let events = read_session_evidence("session-1").unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 0);
        assert_eq!(events[1].sequence, 1);
    }

    #[test]
    fn writer_resumes_sequence_after_existing_log() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("JCODE_HOME", temp.path());

        let mut writer = SessionEvidenceWriter::for_session("session-1", context()).unwrap();
        writer
            .append(SessionLogEventKind::ToolStarted {
                tool_name: "bash".to_string(),
                input: None,
            })
            .unwrap();
        let mut writer = SessionEvidenceWriter::for_session("session-1", context()).unwrap();
        let event = writer
            .append(SessionLogEventKind::ToolFinished {
                tool_name: "bash".to_string(),
                status: SessionLogStatus::Error,
                duration_ms: 20,
                output: None,
                error_class: Some("exit".to_string()),
            })
            .unwrap();

        assert_eq!(event.sequence, 1);
    }

    #[test]
    fn reader_tolerates_empty_and_truncated_trailing_rows() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("session.evidence.jsonl");
        assert!(read_session_evidence_from_path(&path).unwrap().is_empty());

        let mut writer =
            SessionEvidenceWriter::for_path("session-1", path.clone(), context()).unwrap();
        writer
            .append(SessionLogEventKind::ToolStarted {
                tool_name: "bash".to_string(),
                input: Some(payload_summary_text(
                    "secret command",
                    Some("text/plain".to_string()),
                )),
            })
            .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"event_id\":")
            .unwrap();

        let events = read_session_evidence_from_path(&path).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 0);
    }

    #[test]
    fn payload_summary_hashes_without_raw_payload() {
        let summary = payload_summary_text("hello", Some("text/plain".to_string()));
        assert_eq!(summary.bytes, 5);
        assert_eq!(
            summary.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("hello"));
    }
}
