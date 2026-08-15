use futures::StreamExt;
use jcode_message_types::{Message, StreamEvent};
use jcode_provider_acp_runtime::{AcpProcessSpec, AcpProvider, AcpRuntimeConfig};
use jcode_provider_core::Provider;
use jcode_provider_grok_build_runtime::{DenyPermissionBroker, GrokBuildPolicy, GrokBuildProvider};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

fn fake_provider(
    log: &Path,
    scenario: &str,
    auth_methods: Value,
    model_state: Option<Value>,
) -> GrokBuildProvider {
    let mut env = BTreeMap::from([
        ("JCODE_FAKE_ACP_LOG".to_string(), log.display().to_string()),
        ("JCODE_FAKE_ACP_SCENARIO".to_string(), scenario.to_string()),
        (
            "JCODE_FAKE_ACP_AUTH_METHODS".to_string(),
            auth_methods.to_string(),
        ),
    ]);
    if let Some(model_state) = model_state {
        env.insert(
            "JCODE_FAKE_ACP_MODEL_STATE".to_string(),
            model_state.to_string(),
        );
    }
    AcpProvider::with_engine(
        GrokBuildPolicy::with_process(AcpProcessSpec {
            command: PathBuf::from(env!("CARGO_BIN_EXE_jcode-fake-grok-build-acp")),
            args: vec!["agent".to_string(), "stdio".to_string()],
            env,
            cwd: None,
        }),
        AcpRuntimeConfig::default(),
        Some(Arc::new(DenyPermissionBroker)),
    )
}

fn subscription_methods() -> Value {
    json!([
        {"id":"xai.api_key", "name":"xAI API key"},
        {"id":"grok.com", "name":"Grok.com"},
        {"id":"cached_token", "name":"Cached token"}
    ])
}

fn read_log(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("valid fake ACP log row"))
        .collect()
}

fn method_count(log: &[Value], method: &str) -> usize {
    log.iter()
        .filter(|entry| entry.get("method") == Some(&json!(method)))
        .count()
}

async fn drain(provider: &GrokBuildProvider, messages: &[Message], resume: Option<&str>) {
    let mut stream = provider
        .complete(messages, &[], "outer-system", resume)
        .await
        .expect("start fake Grok turn");
    while let Some(event) = stream.next().await {
        event.expect("fake Grok event");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn shared_fake_rejects_api_key_only_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("api-key-only.jsonl");
    let provider = fake_provider(
        &log,
        "happy",
        json!([
            {"id":"xai.api_key", "name":"xAI API key"},
            {"id":"api-key", "name":"API key"}
        ]),
        None,
    );

    let error = provider.prefetch_models().await.unwrap_err().to_string();
    assert!(error.contains("cached subscription authentication method"));
    assert_eq!(method_count(&read_log(&log), "authenticate"), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn shared_fake_prefers_cached_token_and_preserves_headless_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("auth.jsonl");
    let provider = fake_provider(&log, "happy", subscription_methods(), None);

    provider.prefetch_models().await.unwrap();

    let rows = read_log(&log);
    let authenticate = rows
        .iter()
        .find(|entry| entry.get("method") == Some(&json!("authenticate")))
        .expect("authenticate request");
    assert_eq!(
        authenticate.pointer("/params/methodId"),
        Some(&json!("cached_token"))
    );
    assert_eq!(
        authenticate.pointer("/params/_meta/headless"),
        Some(&json!(true))
    );
    assert_eq!(
        rows[0].pointer("/fakeProcess/args"),
        Some(&json!(["agent", "stdio"]))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn shared_fake_discovers_mixed_model_state_and_sets_only_explicit_selection() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("models.jsonl");
    let provider = fake_provider(
        &log,
        "happy",
        subscription_methods(),
        Some(json!({
            "currentModelId":"model-a",
            "availableModels":["model-a", {"id":"model-b"}, {"name":"model-c"}]
        })),
    );

    provider.prefetch_models().await.unwrap();
    assert_eq!(
        provider.available_models_display(),
        ["model-a", "model-b", "model-c"]
    );
    drain(&provider, &[Message::user("no selection")], None).await;
    assert_eq!(method_count(&read_log(&log), "session/set_model"), 0);

    provider.set_model("model-b").unwrap();
    drain(&provider, &[Message::user("explicit selection")], None).await;
    assert_eq!(method_count(&read_log(&log), "session/set_model"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn shared_fake_resumes_without_history_replay_or_redundant_model_reset() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("resume.jsonl");
    let provider = fake_provider(&log, "happy", subscription_methods(), None);
    provider.prefetch_models().await.unwrap();
    provider.set_model("model-b").unwrap();

    drain(
        &provider,
        &[
            Message::user("old prompt"),
            Message::assistant_text("old answer"),
            Message::user("new prompt"),
        ],
        Some("existing-session"),
    )
    .await;

    let rows = read_log(&log);
    assert_eq!(method_count(&rows, "session/resume"), 1);
    assert_eq!(method_count(&rows, "session/set_model"), 0);
    let serialized = serde_json::to_string(&rows).unwrap();
    assert!(serialized.contains("new prompt"));
    assert!(!serialized.contains("old answer"));
}

#[tokio::test(flavor = "current_thread")]
async fn shared_fake_permission_request_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("permission.jsonl");
    let provider = fake_provider(&log, "permission", subscription_methods(), None);

    drain(&provider, &[Message::user("request permission")], None).await;

    let rows = read_log(&log);
    let reply = rows
        .iter()
        .find(|entry| entry.get("id") == Some(&json!(900)) && entry.get("result").is_some())
        .expect("permission reply");
    assert_eq!(
        reply.pointer("/result/outcome/optionId"),
        Some(&json!("reject_once"))
    );
    assert_ne!(
        reply.pointer("/result/outcome/optionId"),
        Some(&json!("allow_once"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_shared_fake_stream_cancels_the_provider_session() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("cancel.jsonl");
    let provider = fake_provider(&log, "prompt_hang", subscription_methods(), None);
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
