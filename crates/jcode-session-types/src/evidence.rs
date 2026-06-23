use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SESSION_LOG_EVENT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionLogEvent {
    #[serde(default = "default_session_log_event_schema_version")]
    pub schema_version: u16,
    pub event_id: String,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_session_id: Option<String>,
    pub node: NodeSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitSnapshot>,
    #[serde(default)]
    pub correlation: CorrelationIds,
    pub kind: SessionLogEventKind,
}

fn default_session_log_event_schema_version() -> u16 {
    SESSION_LOG_EVENT_SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSnapshot {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    pub process_id: u32,
    pub jcode_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jcode_git_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitSnapshot {
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorrelationIds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayloadSummary {
    pub sha256: String,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsageSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionLogEventKind {
    TurnStarted {
        user_message_index: usize,
        #[serde(default)]
        image_count: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<PayloadSummary>,
    },
    TurnFinished {
        status: SessionLogStatus,
        duration_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<PayloadSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_class: Option<String>,
    },
    ProviderRequest {
        provider: String,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route: Option<String>,
        message_count: usize,
        tool_count: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<PayloadSummary>,
    },
    ProviderResponse {
        provider: String,
        model: String,
        status: SessionLogStatus,
        duration_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<PayloadSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage: Option<TokenUsageSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_class: Option<String>,
    },
    ToolStarted {
        tool_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<PayloadSummary>,
    },
    ToolFinished {
        tool_name: String,
        status: SessionLogStatus,
        duration_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<PayloadSummary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_class: Option<String>,
    },
    RouteSelected {
        provider_key: String,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        api_method: Option<String>,
        source: RouteSelectionSource,
    },
    MemoryInjected {
        memory_count: usize,
        age_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<PayloadSummary>,
    },
    ChildSessionStarted {
        child_session_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task: Option<PayloadSummary>,
    },
    PolicyDecision {
        policy: String,
        decision: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        attributes: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionLogStatus {
    Ok,
    Error,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteSelectionSource {
    User,
    Config,
    Auth,
    Auto,
    Restore,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> NodeSnapshot {
        NodeSnapshot {
            node_id: "node-1".to_string(),
            hostname: Some("host".to_string()),
            process_id: 123,
            jcode_version: "dev".to_string(),
            jcode_git_hash: Some("abc123".to_string()),
            runtime: Some("tui".to_string()),
            working_dir: Some("/tmp/project".to_string()),
        }
    }

    fn base_event(kind: SessionLogEventKind, sequence: u64) -> SessionLogEvent {
        SessionLogEvent {
            schema_version: SESSION_LOG_EVENT_SCHEMA_VERSION,
            event_id: format!("event-{sequence}"),
            sequence,
            timestamp: DateTime::parse_from_rfc3339("2026-06-23T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            session_id: "session-1".to_string(),
            parent_session_id: Some("parent".to_string()),
            child_session_id: None,
            node: node(),
            git: Some(GitSnapshot {
                root: "/tmp/project".to_string(),
                head: Some("deadbeef".to_string()),
                branch: Some("main".to_string()),
                dirty: Some(false),
            }),
            correlation: CorrelationIds {
                turn_id: Some("turn-1".to_string()),
                provider_request_id: Some("req-1".to_string()),
                tool_call_id: Some("tool-1".to_string()),
                task_id: None,
            },
            kind,
        }
    }

    fn payload(label: &str) -> PayloadSummary {
        PayloadSummary {
            sha256: format!("sha256-{label}"),
            bytes: 42,
            artifact_path: Some(format!("artifacts/{label}.json")),
            media_type: Some("application/json".to_string()),
        }
    }

    #[test]
    fn all_v1_event_kinds_round_trip() {
        let mut attrs = BTreeMap::new();
        attrs.insert("tool".to_string(), "bash".to_string());
        let kinds = vec![
            SessionLogEventKind::TurnStarted {
                user_message_index: 0,
                image_count: 1,
                input: Some(payload("turn-input")),
            },
            SessionLogEventKind::TurnFinished {
                status: SessionLogStatus::Ok,
                duration_ms: 12,
                output: Some(payload("turn-output")),
                error_class: None,
            },
            SessionLogEventKind::ProviderRequest {
                provider: "openai".to_string(),
                model: "gpt-test".to_string(),
                route: Some("openai-api".to_string()),
                message_count: 3,
                tool_count: 2,
                prompt: Some(payload("provider-request")),
            },
            SessionLogEventKind::ProviderResponse {
                provider: "openai".to_string(),
                model: "gpt-test".to_string(),
                status: SessionLogStatus::Ok,
                duration_ms: 34,
                output: Some(payload("provider-response")),
                usage: Some(TokenUsageSummary {
                    input_tokens: Some(10),
                    output_tokens: Some(20),
                    total_tokens: Some(30),
                }),
                error_class: None,
            },
            SessionLogEventKind::ToolStarted {
                tool_name: "bash".to_string(),
                input: Some(payload("tool-input")),
            },
            SessionLogEventKind::ToolFinished {
                tool_name: "bash".to_string(),
                status: SessionLogStatus::Error,
                duration_ms: 56,
                output: Some(payload("tool-output")),
                error_class: Some("exit_status".to_string()),
            },
            SessionLogEventKind::RouteSelected {
                provider_key: "openai".to_string(),
                model: "gpt-test".to_string(),
                api_method: Some("openai-api".to_string()),
                source: RouteSelectionSource::User,
            },
            SessionLogEventKind::MemoryInjected {
                memory_count: 2,
                age_ms: 78,
                prompt: Some(payload("memory")),
            },
            SessionLogEventKind::ChildSessionStarted {
                child_session_id: "child".to_string(),
                task: Some(payload("task")),
            },
            SessionLogEventKind::PolicyDecision {
                policy: "tools".to_string(),
                decision: "allow".to_string(),
                attributes: attrs,
            },
        ];

        for (idx, kind) in kinds.into_iter().enumerate() {
            let event = base_event(kind, idx as u64 + 1);
            let json = serde_json::to_string(&event).unwrap();
            assert!(!json.contains("raw_payload"));
            let decoded: SessionLogEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, event);
            assert_eq!(decoded.schema_version, SESSION_LOG_EVENT_SCHEMA_VERSION);
        }
    }

    #[test]
    fn omitted_schema_version_defaults_to_v1() {
        let json = r#"{
            "event_id":"event-1",
            "sequence":1,
            "timestamp":"2026-06-23T00:00:00Z",
            "session_id":"session-1",
            "node":{"node_id":"node-1","process_id":123,"jcode_version":"dev"},
            "correlation":{},
            "kind":{
                "event":"route_selected",
                "provider_key":"openai",
                "model":"gpt-test",
                "source":"config"
            }
        }"#;
        let decoded: SessionLogEvent = serde_json::from_str(json).unwrap();
        assert_eq!(decoded.schema_version, SESSION_LOG_EVENT_SCHEMA_VERSION);
    }
}
