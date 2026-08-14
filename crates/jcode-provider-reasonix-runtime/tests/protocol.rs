use futures::StreamExt;
use jcode_message_types::{Message, StreamEvent};
use jcode_provider_acp_runtime::{AcpProcessSpec, AcpProvider, AcpRuntimeConfig};
use jcode_provider_core::Provider;
use jcode_provider_reasonix_runtime::{DenyPermissionBroker, ReasonixPolicy, ReasonixProvider};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

fn fake_provider(
    log: &Path,
    scenario: &str,
    auth_methods: Value,
    config_options: Option<Value>,
) -> ReasonixProvider {
    let mut env = BTreeMap::from([
        ("JCODE_FAKE_ACP_LOG".to_string(), log.display().to_string()),
        ("JCODE_FAKE_ACP_SCENARIO".to_string(), scenario.to_string()),
        (
            "JCODE_FAKE_ACP_AUTH_METHODS".to_string(),
            auth_methods.to_string(),
        ),
    ]);
    if let Some(config_options) = config_options {
        env.insert(
            "JCODE_FAKE_ACP_CONFIG_OPTIONS".to_string(),
            config_options.to_string(),
        );
    }
    AcpProvider::with_engine(
        ReasonixPolicy::with_process(AcpProcessSpec {
            command: PathBuf::from(env!("CARGO_BIN_EXE_jcode-fake-reasonix-acp")),
            args: vec!["acp".to_string(), "-workspace-only".to_string()],
            env,
            cwd: None,
        }),
        AcpRuntimeConfig::default(),
        Some(Arc::new(DenyPermissionBroker)),
    )
}

fn model_options() -> Value {
    json!([
        {
            "id":"model",
            "name":"Model",
            "category":"model",
            "type":"select",
            "currentValue":"deepseek-chat",
            "options":[
                {"value":"deepseek-chat", "name":"DeepSeek Chat"},
                {"value":"deepseek-reasoner", "name":"DeepSeek Reasoner"}
            ]
        },
        {
            "id":"effort",
            "name":"Effort",
            "category":"thought_level",
            "type":"select",
            "currentValue":"high",
            "options":[{"value":"high", "name":"High"}]
        },
        {
            "id":"mode",
            "name":"Mode",
            "category":"mode",
            "type":"select",
            "currentValue":"pair",
            "options":[{"value":"pair", "name":"Pair"}]
        }
    ])
}

fn setup_methods() -> Value {
    json!([{
        "id":"reasonix-setup",
        "type":"terminal",
        "name":"Reasonix setup",
        "description":"Configure Reasonix providers and credentials in a terminal",
        "args":["setup"],
        "env":{}
    }])
}

fn read_log(path: &Path) -> Vec<Value> {
    let contents = std::fs::read_to_string(path).unwrap_or_default();
    let has_partial_last_row = !contents.is_empty() && !contents.ends_with('\n');
    let mut lines = contents.lines().peekable();
    let mut rows = Vec::new();
    while let Some(line) = lines.next() {
        match serde_json::from_str(line) {
            Ok(row) => rows.push(row),
            Err(_) if has_partial_last_row && lines.peek().is_none() => {}
            Err(error) => panic!("valid fake ACP log row: {error}"),
        }
    }
    rows
}

fn method_count(log: &[Value], method: &str) -> usize {
    log.iter()
        .filter(|entry| entry.get("method") == Some(&json!(method)))
        .count()
}

async fn collect(
    provider: &ReasonixProvider,
    messages: &[Message],
    resume: Option<&str>,
) -> anyhow::Result<Vec<StreamEvent>> {
    let mut stream = provider
        .complete(messages, &[], "outer-system", resume)
        .await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event?);
    }
    Ok(events)
}

#[tokio::test(flavor = "current_thread")]
async fn setup_auth_is_acknowledged_without_command_execution_or_secret_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("setup-auth.jsonl");
    let provider = fake_provider(&log, "happy", setup_methods(), None);

    provider.prefetch_models().await.unwrap();
    let rows = read_log(&log);
    assert_eq!(method_count(&rows, "authenticate"), 1);
    let authenticate = rows
        .iter()
        .find(|entry| entry.get("method") == Some(&json!("authenticate")))
        .unwrap();
    assert_eq!(
        authenticate.pointer("/params/methodId"),
        Some(&json!("reasonix-setup"))
    );
    assert_eq!(authenticate.pointer("/params/_meta"), Some(&json!({})));
    assert_eq!(
        rows[0].pointer("/fakeProcess/args"),
        Some(&json!(["acp", "-workspace-only"]))
    );
    let serialized = serde_json::to_string(&rows).unwrap();
    assert!(!serialized.contains("apiKey"));
    assert!(!serialized.contains("DEEPSEEK_API_KEY"));
}

#[tokio::test(flavor = "current_thread")]
async fn configured_runtime_streams_standard_updates_after_auth_acknowledgment() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("stream.jsonl");
    let provider = fake_provider(&log, "happy", setup_methods(), Some(model_options()));

    let events = collect(&provider, &[Message::user("hello")], None)
        .await
        .unwrap();

    assert_eq!(method_count(&read_log(&log), "authenticate"), 1);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::ThinkingDelta(text) if text == "thinking"))
    );
    assert!(events.iter().any(|event| matches!(event, StreamEvent::StatusDetail { detail } if detail == "provider tool running")));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::TextDelta(text) if text == "ACP_TEST_OK"))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::MessageEnd { .. }))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn session_setup_failure_points_to_reasonix_setup() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("setup-required.jsonl");
    let provider = fake_provider(&log, "auth_required_new", setup_methods(), None);

    let error = collect(&provider, &[Message::user("hello")], None)
        .await
        .unwrap()
        .into_iter()
        .find_map(|event| match event {
            StreamEvent::Error { message, .. } => Some(message),
            _ => None,
        })
        .expect("setup-required stream error");

    assert!(error.contains("authentication required"));
    assert!(error.contains("reasonix setup"));
    assert_eq!(method_count(&read_log(&log), "session/new"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn model_selection_uses_only_the_model_config_axis() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("model.jsonl");
    let provider = fake_provider(&log, "happy", setup_methods(), Some(model_options()));
    provider.set_model("deepseek-reasoner").unwrap();

    collect(&provider, &[Message::user("use reasoner")], None)
        .await
        .unwrap();

    assert_eq!(
        provider.available_models_display(),
        ["deepseek-chat", "deepseek-reasoner"]
    );
    let rows = read_log(&log);
    let mutations = rows
        .iter()
        .filter(|entry| entry.get("method") == Some(&json!("session/set_config_option")))
        .collect::<Vec<_>>();
    assert_eq!(mutations.len(), 1);
    assert_eq!(
        mutations[0].pointer("/params/configId"),
        Some(&json!("model"))
    );
    assert_eq!(
        mutations[0].pointer("/params/value"),
        Some(&json!("deepseek-reasoner"))
    );
    assert_eq!(method_count(&rows, "session/set_model"), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn resumed_session_reuses_provider_state_without_history_or_config_replay() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("resume.jsonl");
    let provider = fake_provider(&log, "happy", setup_methods(), Some(model_options()));
    provider.set_model("deepseek-reasoner").unwrap();

    collect(
        &provider,
        &[
            Message::user("old prompt"),
            Message::assistant_text("old answer"),
            Message::user("new prompt"),
        ],
        Some("existing-reasonix-session"),
    )
    .await
    .unwrap();

    let rows = read_log(&log);
    assert_eq!(method_count(&rows, "session/resume"), 1);
    assert_eq!(method_count(&rows, "session/new"), 0);
    assert_eq!(method_count(&rows, "session/set_config_option"), 0);
    let serialized = serde_json::to_string(&rows).unwrap();
    assert!(serialized.contains("new prompt"));
    assert!(!serialized.contains("old answer"));
}

#[tokio::test(flavor = "current_thread")]
async fn permission_request_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("permission.jsonl");
    let provider = fake_provider(&log, "permission", setup_methods(), None);

    collect(&provider, &[Message::user("request permission")], None)
        .await
        .unwrap();

    let reply = read_log(&log)
        .into_iter()
        .find(|entry| entry.get("id") == Some(&json!(900)) && entry.get("result").is_some())
        .expect("permission reply");
    assert_eq!(
        reply.pointer("/result/outcome/optionId"),
        Some(&json!("reject_once"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_stream_cancels_reasonix_session() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("cancel.jsonl");
    let provider = fake_provider(&log, "prompt_hang", setup_methods(), None);
    let mut stream = provider
        .complete(&[Message::user("wait")], &[], "", None)
        .await
        .unwrap();
    let session = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("session setup timed out")
        .expect("stream ended before session id")
        .unwrap();
    assert!(matches!(session, StreamEvent::SessionId(_)));
    drop(stream);

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if method_count(&read_log(&log), "session/cancel") > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("stream drop did not send session/cancel");
}
