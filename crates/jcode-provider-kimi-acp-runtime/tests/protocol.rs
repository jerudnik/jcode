use futures::StreamExt;
use jcode_message_types::{Message, StreamEvent};
use jcode_provider_acp_runtime::{
    AcpPermissionBroker, AcpPermissionDecision, AcpPermissionRequest, AcpProcessSpec, AcpProvider,
    AcpRuntimeConfig,
};
use jcode_provider_core::Provider;
use jcode_provider_kimi_acp_runtime::{KimiCodePolicy, KimiCodeProvider};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct SelectOpaqueBroker {
    requests: Mutex<Vec<AcpPermissionRequest>>,
}

impl SelectOpaqueBroker {
    fn new() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl AcpPermissionBroker for SelectOpaqueBroker {
    fn decide(
        &self,
        request: AcpPermissionRequest,
    ) -> Pin<Box<dyn Future<Output = AcpPermissionDecision> + Send + '_>> {
        self.requests.lock().unwrap().push(request);
        Box::pin(async {
            AcpPermissionDecision::Select {
                option_id: "plan.choice/opaque:β-17".to_string(),
            }
        })
    }
}

fn login_methods() -> Value {
    json!([{
        "id":"login",
        "type":"terminal",
        "name":"Login with Kimi account",
        "args":["--login"]
    }])
}

fn fake_provider(
    log: &Path,
    scenario: &str,
    config_options: Option<Value>,
    broker: Option<Arc<dyn AcpPermissionBroker>>,
) -> KimiCodeProvider {
    let mut env = BTreeMap::from([
        ("JCODE_FAKE_ACP_LOG".to_string(), log.display().to_string()),
        ("JCODE_FAKE_ACP_SCENARIO".to_string(), scenario.to_string()),
        (
            "JCODE_FAKE_ACP_AUTH_METHODS".to_string(),
            login_methods().to_string(),
        ),
        (
            "JCODE_FAKE_ACP_AGENT_CAPABILITIES".to_string(),
            json!({
                "loadSession":true,
                "promptCapabilities":{"image":true,"audio":false,"embeddedContext":true},
                "sessionCapabilities":{"list":{},"resume":{}}
            })
            .to_string(),
        ),
    ]);
    if let Some(config_options) = config_options {
        env.insert(
            "JCODE_FAKE_ACP_CONFIG_OPTIONS".to_string(),
            config_options.to_string(),
        );
    }
    AcpProvider::with_engine(
        KimiCodePolicy::with_process(AcpProcessSpec {
            command: PathBuf::from(env!("CARGO_BIN_EXE_jcode-fake-kimi-acp")),
            args: vec!["acp".to_string()],
            env,
            cwd: None,
        }),
        AcpRuntimeConfig::default(),
        broker,
    )
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

async fn drain(provider: &KimiCodeProvider) -> anyhow::Result<()> {
    let mut stream = provider
        .complete(&[Message::user("Kimi structured prompt")], &[], "", None)
        .await?;
    while let Some(event) = stream.next().await {
        event?;
    }
    Ok(())
}

async fn collect(provider: &KimiCodeProvider) -> anyhow::Result<Vec<StreamEvent>> {
    let mut stream = provider
        .complete(&[Message::user("Kimi structured prompt")], &[], "", None)
        .await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event?);
    }
    Ok(events)
}

#[tokio::test(flavor = "current_thread")]
async fn terminal_login_method_authenticates_by_rechecking_cached_token() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("terminal-auth.jsonl");
    let provider = fake_provider(&log, "happy", None, None);

    provider.prefetch_models().await.unwrap();

    let rows = read_log(&log);
    let authenticate = rows
        .iter()
        .find(|entry| entry.get("method") == Some(&json!("authenticate")))
        .expect("authenticate request");
    assert_eq!(
        authenticate.pointer("/params/methodId"),
        Some(&json!("login"))
    );
    assert_eq!(rows[0].pointer("/fakeProcess/args"), Some(&json!(["acp"])));
}

#[tokio::test(flavor = "current_thread")]
async fn unauthenticated_new_session_reports_auth_required() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("auth-required.jsonl");
    let provider = fake_provider(&log, "auth_required_new", None, None);

    let error = collect(&provider)
        .await
        .unwrap()
        .into_iter()
        .find_map(|event| match event {
            StreamEvent::Error { message, .. } => Some(message),
            _ => None,
        })
        .expect("auth-required stream error");
    assert!(error.contains("authentication required"));
    assert!(error.contains("kimi acp --login"));
    assert_eq!(method_count(&read_log(&log), "session/new"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn config_options_preserve_model_thinking_and_mode_axes() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("config-options.jsonl");
    let options = json!([
        {"id":"model","name":"Model","category":"model","type":"select","currentValue":"kimi-a","options":[{"value":"kimi-a","name":"A"},{"value":"kimi-b","name":"B"}]},
        {"id":"thinking","name":"Thinking","category":"thought_level","type":"select","currentValue":"high","options":[{"value":"off","name":"Off"},{"value":"high","name":"High"}]},
        {"id":"mode","name":"Mode","category":"mode","type":"select","currentValue":"plan","options":[{"value":"agent","name":"Agent"},{"value":"plan","name":"Plan"}]}
    ]);
    let provider = fake_provider(&log, "happy", Some(options), None);
    provider.set_model("kimi-b").unwrap();

    drain(&provider).await.unwrap();

    assert_eq!(provider.available_models_display(), ["kimi-a", "kimi-b"]);
    let rows = read_log(&log);
    let set = rows
        .iter()
        .find(|entry| entry.get("method") == Some(&json!("session/set_config_option")))
        .expect("set config option request");
    assert_eq!(set.pointer("/params/configId"), Some(&json!("model")));
    assert_eq!(set.pointer("/params/value"), Some(&json!("kimi-b")));
    assert_eq!(method_count(&rows, "session/set_model"), 0);
    let serialized = serde_json::to_string(&rows).unwrap();
    assert!(!serialized.contains("kimi-b/high/plan"));
}

#[tokio::test(flavor = "current_thread")]
async fn plan_question_option_id_round_trips_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("opaque-option.jsonl");
    let broker = Arc::new(SelectOpaqueBroker::new());
    let provider = fake_provider(
        &log,
        "permission",
        None,
        Some(Arc::clone(&broker) as Arc<dyn AcpPermissionBroker>),
    );

    drain(&provider).await.unwrap();

    let requests = broker.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].options[3].option_id, "plan.choice/opaque:β-17");
    drop(requests);
    let reply = read_log(&log)
        .into_iter()
        .find(|entry| entry.get("id") == Some(&json!(900)) && entry.get("result").is_some())
        .expect("permission response");
    assert_eq!(
        reply.pointer("/result/outcome/optionId"),
        Some(&json!("plan.choice/opaque:β-17"))
    );
}
