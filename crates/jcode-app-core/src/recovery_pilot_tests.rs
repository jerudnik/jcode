use crate::agent::Agent;
use crate::auth::{AuthReadinessLevel, AuthStatus};
use crate::message::{Message, StreamEvent, ToolDefinition};
use crate::protocol::{
    HandshakeCompatibility, PROTOCOL_VERSION, Request, ServerEvent, decode_request,
};
use crate::provider::{EventStream, Provider, RouteSelection, RuntimeKey};
use crate::subscription_catalog::{JcodeTier, SubscriptionTierFreshness};
use crate::tool::Registry;
use anyhow::Result;
use async_trait::async_trait;
use jcode_provider_core::ResolvedCredential;
use jcode_selfdev_types::RuntimeIdentityProjection;
use jcode_session_types::{
    SESSION_LOG_EVENT_SCHEMA_VERSION, SessionLogEvent, SessionLogEventKind, SessionLogStatus,
};
use std::collections::HashSet;
use std::ffi::OsString;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Clone)]
enum PilotProviderEvent {
    Event(StreamEvent),
}

#[derive(Clone)]
struct RecoveryPilotProvider {
    events: Vec<PilotProviderEvent>,
    selected_route: Arc<Mutex<Option<RouteSelection>>>,
    calls: Arc<Mutex<Vec<RecoveryPilotProviderCall>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecoveryPilotProviderCall {
    message_count: usize,
    tool_count: usize,
    model: String,
}

struct ScopedEnvVar {
    key: &'static str,
    previous: Option<OsString>,
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => crate::env::set_var(self.key, value),
            None => crate::env::remove_var(self.key),
        }
    }
}

#[async_trait]
impl Provider for RecoveryPilotProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        self.calls
            .lock()
            .expect("pilot call lock")
            .push(RecoveryPilotProviderCall {
                message_count: messages.len(),
                tool_count: tools.len(),
                model: self.model(),
            });

        let (tx, rx) = mpsc::channel::<Result<StreamEvent>>(8);
        let events = self.events.clone();
        tokio::spawn(async move {
            for event in events {
                let PilotProviderEvent::Event(event) = event;
                let _ = tx.send(Ok(event)).await;
            }
        });
        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    fn name(&self) -> &str {
        "jcode"
    }

    fn model(&self) -> String {
        self.selected_route
            .lock()
            .expect("pilot route lock")
            .as_ref()
            .map(|selection| selection.model.clone())
            .unwrap_or_else(|| "unselected".to_string())
    }

    fn active_resolved_credential(&self) -> Option<ResolvedCredential> {
        Some(ResolvedCredential::Oauth)
    }

    fn set_route_selection(&self, selection: &RouteSelection) -> Result<()> {
        *self.selected_route.lock().expect("pilot route lock") = Some(selection.clone());
        Ok(())
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

fn provider_terminal_counts(
    events: &[SessionLogEvent],
) -> (usize, usize, usize, Vec<SessionLogStatus>) {
    let mut requests = 0;
    let mut responses = 0;
    let mut finishes = 0;
    let mut response_statuses = Vec::new();
    for event in events {
        match &event.kind {
            SessionLogEventKind::ProviderRequest { .. } => requests += 1,
            SessionLogEventKind::ProviderResponse { status, .. } => {
                responses += 1;
                response_statuses.push(*status);
            }
            SessionLogEventKind::TurnFinished { .. } => finishes += 1,
            _ => {}
        }
    }
    (requests, responses, finishes, response_statuses)
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_pilot_one_fixture_route_subscribe_turn_evidence() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::tempdir().expect("temp JCODE_HOME");
    let _home = ScopedEnvVar::set("JCODE_HOME", temp_home.path());
    let _telemetry = ScopedEnvVar::set("JCODE_NO_TELEMETRY", "1");
    let _api_key = ScopedEnvVar::set(
        crate::subscription_catalog::JCODE_API_KEY_ENV,
        "fixture-key",
    );
    let _ambient_tier = ScopedEnvVar::set(crate::subscription_catalog::JCODE_TIER_ENV, "flagship");
    AuthStatus::invalidate_cache();
    crate::subscription_catalog::store_cached_tier(None).expect("clear cached tier");

    let auth_before = AuthStatus::check()
        .assessment_for_provider(crate::provider_catalog::JCODE_LOGIN_PROVIDER)
        .readiness;
    assert_eq!(auth_before, AuthReadinessLevel::CredentialPresent);

    let subscription = crate::subscription_api::apply_subscription_me_fixture(
        r#"{
            "account_id": "acct_fixture",
            "email": "fixture@example.invalid",
            "tier": "plus",
            "status": "active",
            "usage": {"used_usd": 0.0, "budget_usd": 100.0}
        }"#,
    )
    .expect("apply offline subscription fixture");
    let tier_truth = subscription.tier_truth();
    assert_eq!(subscription.account_id, "acct_fixture");
    assert_eq!(tier_truth.parsed_tier, Some(JcodeTier::Plus));
    assert_eq!(tier_truth.freshness, SubscriptionTierFreshness::Live);
    assert_eq!(
        crate::subscription_catalog::cached_tier(),
        Some(JcodeTier::Plus)
    );
    assert_eq!(
        crate::subscription_catalog::effective_tier(),
        JcodeTier::Plus
    );
    assert!(crate::subscription_catalog::is_model_allowed_for_current_tier("gpt-5.5"));
    assert!(!crate::subscription_catalog::is_model_allowed_for_current_tier("claude-fable-5"));

    AuthStatus::invalidate_cache();
    let auth_after = AuthStatus::check()
        .assessment_for_provider(crate::provider_catalog::JCODE_LOGIN_PROVIDER)
        .readiness;
    assert_eq!(auth_after, AuthReadinessLevel::RequestValid);
    assert!(!crate::telemetry::is_enabled());
    assert!(!temp_home.path().join("telemetry_share_content").exists());

    let client_runtime_identity = RuntimeIdentityProjection {
        version_label: "fixture-client-version".to_string(),
        source_fingerprint: Some("fixture-client-fingerprint".to_string()),
        source_dirty: Some(true),
        source_hash: Some("fixture-client-hash".to_string()),
        source_full_hash: Some("fixture-client-full-hash".to_string()),
        activation_channel: "fixture-client".to_string(),
        resolved_executable_payload: "/fixture/client/jcode".into(),
    };
    let subscribe = Request::Subscribe {
        id: 1,
        working_dir: None,
        selfdev: None,
        target_session_id: None,
        client_instance_id: None,
        client_has_local_history: false,
        allow_session_takeover: false,
        terminal_env: Vec::new(),
        protocol_version: Some(PROTOCOL_VERSION),
        build_hash: Some(jcode_build_meta::GIT_HASH.to_string()),
        runtime_identity: Some(client_runtime_identity.clone()),
        spawn_swarm_id: None,
        spawn_session_id: None,
        client_pid: None,
    };
    let encoded_subscribe = serde_json::to_string(&subscribe).expect("encode Subscribe");
    let decoded_subscribe = decode_request(&encoded_subscribe).expect("decode Subscribe");
    let Request::Subscribe {
        id,
        protocol_version,
        build_hash,
        runtime_identity,
        ..
    } = decoded_subscribe
    else {
        panic!("expected Subscribe");
    };
    assert_eq!(runtime_identity, Some(client_runtime_identity.clone()));

    let (compatibility, detail) = HandshakeCompatibility::evaluate(
        protocol_version,
        build_hash.as_deref(),
        PROTOCOL_VERSION,
        Some(jcode_build_meta::GIT_HASH),
    );
    assert_eq!(compatibility, HandshakeCompatibility::Compatible);
    let server_runtime_identity =
        crate::build::current_runtime_identity_projection("shared-server");
    assert_ne!(client_runtime_identity, server_runtime_identity);
    let verdict = ServerEvent::HandshakeVerdict {
        id,
        compatibility,
        server_protocol_version: PROTOCOL_VERSION,
        server_build_hash: Some(jcode_build_meta::GIT_HASH.to_string()),
        server_runtime_identity: Some(server_runtime_identity.clone()),
        detail,
    };
    match verdict {
        ServerEvent::HandshakeVerdict {
            compatibility,
            server_runtime_identity: Some(projection),
            ..
        } => {
            assert_eq!(compatibility, HandshakeCompatibility::Compatible);
            assert_eq!(projection, server_runtime_identity);
        }
        other => panic!("expected typed HandshakeVerdict, got {other:?}"),
    }

    let selected_route = RouteSelection {
        model: "gpt-5.5".to_string(),
        runtime_key: RuntimeKey::Other("jcode-subscription".to_string()),
        api_method: "jcode-subscription".to_string(),
        provider_label: "jcode".to_string(),
        detail: String::new(),
    };
    let selected_route_state = Arc::new(Mutex::new(None));
    let provider_calls = Arc::new(Mutex::new(Vec::new()));
    let provider: Arc<dyn Provider> = Arc::new(RecoveryPilotProvider {
        events: vec![
            PilotProviderEvent::Event(StreamEvent::TextDelta("fixture answer".to_string())),
            PilotProviderEvent::Event(StreamEvent::TokenUsage {
                input_tokens: Some(7),
                output_tokens: Some(3),
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            }),
            PilotProviderEvent::Event(StreamEvent::MessageEnd {
                stop_reason: Some("end_turn".to_string()),
            }),
        ],
        selected_route: Arc::clone(&selected_route_state),
        calls: Arc::clone(&provider_calls),
    });
    let registry = Registry::new(Arc::clone(&provider)).await;
    let session = crate::session::Session::create(None, None);
    let mut agent = Agent::new_with_session(provider, registry, session, Some(HashSet::new()));
    agent.set_memory_enabled(false);
    assert!(!agent.memory_enabled());
    agent
        .set_route_selection(&selected_route)
        .expect("apply structured subscription route");
    assert_eq!(
        agent.provider_handle().active_resolved_credential(),
        Some(ResolvedCredential::Oauth)
    );
    assert_eq!(
        selected_route_state
            .lock()
            .expect("pilot route lock")
            .as_ref(),
        Some(&selected_route)
    );

    let session_id = agent.session_id().to_string();
    let output = agent
        .run_once_capture("bounded recovery pilot")
        .await
        .expect("one no-tool fixture turn should succeed");
    assert_eq!(output, "fixture answer");
    let calls = provider_calls.lock().expect("pilot call lock").clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_count, 0);
    assert_eq!(calls[0].model, "gpt-5.5");
    assert!(calls[0].message_count > 0);

    let evidence_path = crate::session::session_evidence_path(&session_id).unwrap();
    let raw_evidence = std::fs::read_to_string(&evidence_path).expect("read raw evidence");
    assert!(!raw_evidence.contains("fixture-key"));
    let events = crate::session::read_session_evidence(&session_id).unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert!(
        events
            .iter()
            .all(|event| event.schema_version == SESSION_LOG_EVENT_SCHEMA_VERSION)
    );
    assert!(matches!(
        &events[0].kind,
        SessionLogEventKind::TurnStarted { .. }
    ));
    assert!(matches!(
        &events[1].kind,
        SessionLogEventKind::ProviderRequest { .. }
    ));
    assert!(matches!(
        &events[2].kind,
        SessionLogEventKind::ProviderResponse { .. }
    ));
    assert!(matches!(
        &events[3].kind,
        SessionLogEventKind::TurnFinished { .. }
    ));

    let turn_id = events[0].correlation.turn_id.as_deref().expect("turn id");
    assert!(events.iter().all(|event| {
        event.correlation.turn_id.as_deref() == Some(turn_id)
            && event.correlation.tool_call_id.is_none()
    }));
    let provider_request_id = events[1]
        .correlation
        .provider_request_id
        .as_deref()
        .expect("provider request id");
    assert_eq!(
        events[2].correlation.provider_request_id.as_deref(),
        Some(provider_request_id)
    );
    assert!(events[0].correlation.provider_request_id.is_none());
    assert!(events[3].correlation.provider_request_id.is_none());

    match &events[1].kind {
        SessionLogEventKind::ProviderRequest {
            provider,
            model,
            route,
            tool_count,
            ..
        } => {
            assert_eq!(provider, "jcode");
            assert_eq!(model, "gpt-5.5");
            assert_eq!(route.as_deref(), Some("jcode-subscription"));
            assert_eq!(*tool_count, 0);
        }
        other => panic!("expected ProviderRequest, got {other:?}"),
    }
    match &events[2].kind {
        SessionLogEventKind::ProviderResponse {
            provider,
            model,
            status,
            usage,
            error_class,
            ..
        } => {
            assert_eq!(provider, "jcode");
            assert_eq!(model, "gpt-5.5");
            assert_eq!(*status, SessionLogStatus::Ok);
            let usage = usage.as_ref().expect("deterministic usage");
            assert_eq!(usage.input_tokens, Some(7));
            assert_eq!(usage.output_tokens, Some(3));
            assert_eq!(usage.total_tokens, Some(10));
            assert!(error_class.is_none());
        }
        other => panic!("expected ProviderResponse, got {other:?}"),
    }
    match &events[3].kind {
        SessionLogEventKind::TurnFinished { status, .. } => {
            assert_eq!(*status, SessionLogStatus::Ok)
        }
        other => panic!("expected TurnFinished, got {other:?}"),
    }
    assert_eq!(
        provider_terminal_counts(&events),
        (1, 1, 1, vec![SessionLogStatus::Ok])
    );

    std::fs::OpenOptions::new()
        .append(true)
        .open(&evidence_path)
        .unwrap()
        .write_all(b"{\"event_id\":")
        .unwrap();
    let replayed = crate::session::read_session_evidence_from_path(&evidence_path).unwrap();
    assert_eq!(replayed, events);

    let logs_dir = temp_home.path().join("logs");
    if logs_dir.exists() {
        let mut pending = vec![logs_dir];
        while let Some(path) = pending.pop() {
            for entry in std::fs::read_dir(path).expect("read log directory") {
                let entry = entry.expect("read log entry");
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.is_file() {
                    let contents = std::fs::read_to_string(path).unwrap_or_default();
                    assert!(!contents.contains("begin telemetry session"));
                }
            }
        }
    }

    println!(
        "\nPILOT_OBSERVATION {}",
        serde_json::json!({
            "account_id": "acct_fixture",
            "auth_after": "request_valid",
            "auth_before": "credential_present",
            "credential": "oauth",
            "evidence_events": 4,
            "handshake": "compatible",
            "memory_enabled": false,
            "model": "gpt-5.5",
            "provider": "jcode",
            "replay_events": 4,
            "route": "jcode-subscription",
            "runtime_projection_distinct": true,
            "terminal_counts": {"finish": 1, "request": 1, "response": 1},
            "telemetry_enabled": false,
            "tier": "plus",
            "tier_freshness": "live",
            "tool_count": 0,
            "usage": {"input": 7, "output": 3, "total": 10}
        })
    );
    AuthStatus::invalidate_cache();
}
