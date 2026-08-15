use agent_client_protocol as acp;
use anyhow::{Result, bail};
use futures::StreamExt;
use jcode_message_types::{ContentBlock as JcodeContentBlock, Message, StreamEvent};
use jcode_provider_acp_runtime::{
    AcpAuthAction, AcpPermissionBroker, AcpPermissionDecision, AcpPermissionRequest,
    AcpProcessSpec, AcpPromptInput, AcpProvider, AcpProviderPolicy, AcpRuntimeConfig,
    AcpSessionMutation, AcpSessionState, DiscoveredModels, TerminalAuthSpec,
};
use jcode_provider_core::Provider;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[derive(Clone, Copy)]
enum AuthMode {
    Cached,
    None,
    Terminal,
}

#[derive(Clone)]
struct TestPolicy {
    scenario: String,
    process: AcpProcessSpec,
    log_path: PathBuf,
    auth_mode: AuthMode,
    mutate_config: bool,
    auth_methods_seen: Arc<Mutex<Vec<Vec<String>>>>,
    _tempdir: Arc<TempDir>,
}

impl TestPolicy {
    fn new(scenario: &str) -> Self {
        let tempdir = Arc::new(tempfile::tempdir().expect("create fake ACP tempdir"));
        let log_path = tempdir.path().join("protocol.jsonl");
        let env = BTreeMap::from([
            ("JCODE_FAKE_ACP_SCENARIO".to_string(), scenario.to_string()),
            (
                "JCODE_FAKE_ACP_LOG".to_string(),
                log_path.display().to_string(),
            ),
            ("JCODE_FAKE_MARKER".to_string(), "literal-env".to_string()),
        ]);
        Self {
            scenario: scenario.to_string(),
            process: AcpProcessSpec {
                command: PathBuf::from(env!("CARGO_BIN_EXE_jcode-fake-acp")),
                args: vec!["literal;not-a-shell".to_string()],
                env,
                cwd: Some(tempdir.path().to_path_buf()),
            },
            log_path,
            auth_mode: AuthMode::Cached,
            mutate_config: false,
            auth_methods_seen: Arc::new(Mutex::new(Vec::new())),
            _tempdir: tempdir,
        }
    }

    fn auth_mode(mut self, auth_mode: AuthMode) -> Self {
        self.auth_mode = auth_mode;
        self
    }

    /// Drop the policy-pinned cwd, matching production workspace-scoped
    /// runtimes (Reasonix/Kimi/Grok Build all set `cwd: None`).
    fn no_process_cwd(mut self) -> Self {
        self.process.cwd = None;
        self
    }

    fn mutate_config(mut self) -> Self {
        self.mutate_config = true;
        self
    }
}

impl AcpProviderPolicy for TestPolicy {
    fn provider_id(&self) -> &'static str {
        "test-acp"
    }

    fn display_name(&self) -> &'static str {
        "Test ACP"
    }

    fn process(&self) -> AcpProcessSpec {
        self.process.clone()
    }

    fn initialize_request(&self) -> acp::InitializeRequest {
        acp::InitializeRequest::new(acp::ProtocolVersion::V1)
            .meta(object(json!({"client":{"opaque":[1,true,null]}})))
    }

    fn choose_auth(&self, initialized: &acp::InitializeResponse) -> Result<AcpAuthAction> {
        self.auth_methods_seen.lock().unwrap().push(
            initialized
                .auth_methods
                .iter()
                .map(|method| method.id().0.to_string())
                .collect(),
        );
        match self.auth_mode {
            AuthMode::None => Ok(AcpAuthAction::None),
            AuthMode::Terminal => Ok(AcpAuthAction::TerminalLoginRequired(TerminalAuthSpec {
                command: Some(PathBuf::from("test-acp-login")),
                args: vec!["login".to_string()],
                meta: Some(json!({"terminal":"opaque"})),
            })),
            AuthMode::Cached => {
                let Some(method) = initialized
                    .auth_methods
                    .iter()
                    .find(|method| method.id().0.as_ref() == "cached")
                else {
                    bail!("cached authentication is required")
                };
                Ok(AcpAuthAction::Authenticate {
                    method_id: method.id().clone(),
                    meta: object(json!({"auth":"opaque"})),
                })
            }
        }
    }

    fn discover_models(
        &self,
        initialized: &acp::InitializeResponse,
        session: Option<&AcpSessionState>,
    ) -> DiscoveredModels {
        if let Some(models) = session.and_then(|state| state.models.as_ref()) {
            return normalize_models(models);
        }
        if let Some(models) = initialized
            .meta
            .as_ref()
            .and_then(|meta| meta.get("modelState"))
            .cloned()
            .and_then(|value| serde_json::from_value::<acp::SessionModelState>(value).ok())
        {
            return normalize_models(&models);
        }
        session
            .and_then(|state| state.config_options.as_deref())
            .and_then(models_from_config)
            .unwrap_or_default()
    }

    fn prompt_blocks(&self, input: AcpPromptInput<'_>) -> Result<Vec<acp::ContentBlock>> {
        let latest = input
            .messages
            .last()
            .and_then(message_text)
            .unwrap_or_default();
        Ok(vec![acp::ContentBlock::Text(
            acp::TextContent::new(format!("resumed={};latest={latest}", input.resumed))
                .meta(object(json!({"prompt":"opaque"}))),
        )])
    }

    fn session_setup(&self, state: &AcpSessionState) -> Result<Vec<AcpSessionMutation>> {
        let mut mutations = Vec::new();
        if let Some(selected) = state.selected_model.as_ref()
            && state
                .models
                .as_ref()
                .map(|models| models.current_model_id.0.as_ref())
                != Some(selected.as_str())
        {
            mutations.push(AcpSessionMutation::SetModel {
                model_id: selected.clone(),
                meta: Some(object(json!({"model":"opaque"}))),
            });
        }
        if self.mutate_config
            && state
                .config_options
                .as_ref()
                .is_some_and(|options| !options.is_empty())
        {
            mutations.push(AcpSessionMutation::SetConfigOption {
                config_id: "thinking".to_string(),
                value: "high".to_string(),
                meta: Some(object(json!({"config":"opaque"}))),
            });
        }
        Ok(mutations)
    }

    fn map_update(&self, update: acp::SessionUpdate) -> Vec<StreamEvent> {
        match update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => text_from_chunk(chunk)
                .map(StreamEvent::TextDelta)
                .into_iter()
                .collect(),
            acp::SessionUpdate::AgentThoughtChunk(chunk) => text_from_chunk(chunk)
                .map(StreamEvent::ThinkingDelta)
                .into_iter()
                .collect(),
            acp::SessionUpdate::ToolCall(call) => {
                vec![StreamEvent::StatusDetail { detail: call.title }]
            }
            acp::SessionUpdate::ToolCallUpdate(update) => update
                .fields
                .title
                .map(|detail| StreamEvent::StatusDetail { detail })
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    fn login_hint(&self, error: &anyhow::Error) -> String {
        format!("test login hint for {}: {error}", self.scenario)
    }
}

#[derive(Clone)]
struct RecordingBroker {
    decision: AcpPermissionDecision,
    delay: Duration,
    requests: Arc<Mutex<Vec<AcpPermissionRequest>>>,
}

impl RecordingBroker {
    fn new(decision: AcpPermissionDecision) -> Self {
        Self {
            decision,
            delay: Duration::ZERO,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

impl AcpPermissionBroker for RecordingBroker {
    fn decide(
        &self,
        request: AcpPermissionRequest,
    ) -> Pin<Box<dyn Future<Output = AcpPermissionDecision> + Send + '_>> {
        self.requests.lock().unwrap().push(request);
        let decision = self.decision.clone();
        let delay = self.delay;
        Box::pin(async move {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            decision
        })
    }
}

fn object(value: Value) -> Map<String, Value> {
    value.as_object().expect("JSON object").clone()
}

fn normalize_models(models: &acp::SessionModelState) -> DiscoveredModels {
    let mut seen = HashSet::new();
    let available = models
        .available_models
        .iter()
        .map(|model| model.model_id.0.to_string())
        .filter(|id| !id.trim().is_empty() && seen.insert(id.clone()))
        .collect();
    DiscoveredModels {
        current: Some(models.current_model_id.0.to_string()),
        available,
    }
}

fn models_from_config(options: &[acp::SessionConfigOption]) -> Option<DiscoveredModels> {
    let option = options.iter().find(|option| {
        option.id.0.as_ref() == "model"
            || matches!(
                option.category,
                Some(acp::SessionConfigOptionCategory::Model)
            )
    })?;
    let acp::SessionConfigKind::Select(select) = &option.kind else {
        return None;
    };
    let mut available = Vec::new();
    match &select.options {
        acp::SessionConfigSelectOptions::Ungrouped(options) => {
            available.extend(options.iter().map(|option| option.value.0.to_string()));
        }
        acp::SessionConfigSelectOptions::Grouped(groups) => {
            available.extend(
                groups
                    .iter()
                    .flat_map(|group| group.options.iter())
                    .map(|option| option.value.0.to_string()),
            );
        }
        _ => {}
    }
    Some(DiscoveredModels {
        current: Some(select.current_value.0.to_string()),
        available,
    })
}

fn message_text(message: &Message) -> Option<&str> {
    message.content.iter().rev().find_map(|block| match block {
        JcodeContentBlock::Text { text, .. } => Some(text.as_str()),
        _ => None,
    })
}

fn text_from_chunk(chunk: acp::ContentChunk) -> Option<String> {
    match chunk.content {
        acp::ContentBlock::Text(text) => Some(text.text),
        _ => None,
    }
}

fn test_provider(
    policy: TestPolicy,
    broker: Option<Arc<dyn AcpPermissionBroker>>,
    request_timeout: Duration,
    prompt_timeout: Duration,
) -> AcpProvider<TestPolicy> {
    test_provider_with_buffer(policy, broker, request_timeout, prompt_timeout, 16)
}

fn test_provider_with_buffer(
    policy: TestPolicy,
    broker: Option<Arc<dyn AcpPermissionBroker>>,
    request_timeout: Duration,
    prompt_timeout: Duration,
    event_buffer: usize,
) -> AcpProvider<TestPolicy> {
    AcpProvider::with_engine(
        policy,
        AcpRuntimeConfig {
            request_timeout,
            prompt_timeout,
            stderr_limit: 64,
            event_buffer,
        },
        broker,
    )
}

async fn collect_turn(
    provider: &AcpProvider<TestPolicy>,
    resume_session_id: Option<&str>,
) -> Vec<StreamEvent> {
    let messages = [Message::user("old history"), Message::user("latest prompt")];
    let mut stream = provider
        .complete(&messages, &[], "system", resume_session_id)
        .await
        .expect("start ACP turn");
    tokio::time::timeout(Duration::from_secs(3), async move {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event.expect("ACP stream item"));
        }
        events
    })
    .await
    .expect("ACP turn completed")
}

fn read_log(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .expect("read fake ACP log")
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n') && !line.trim().is_empty())
        .map(|line| serde_json::from_str(line.trim_end()).expect("parse fake ACP log line"))
        .collect()
}

fn methods(log: &[Value]) -> Vec<&str> {
    log.iter()
        .filter_map(|entry| entry.get("method").and_then(Value::as_str))
        .collect()
}

#[cfg(unix)]
fn process_is_alive(pid: u64) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn error_text(events: &[StreamEvent]) -> Option<&str> {
    events.iter().find_map(|event| match event {
        StreamEvent::Error { message, .. } => Some(message.as_str()),
        _ => None,
    })
}

fn permission_reply(log: &[Value]) -> &Value {
    log.iter()
        .find(|entry| entry.get("id") == Some(&json!(900)) && entry.get("result").is_some())
        .expect("permission reply")
}

#[tokio::test(flavor = "current_thread")]
async fn process_init_auth_catalog_and_new_resume_lifecycle_work() {
    let previous_secret = std::env::var_os("JCODE_ACP_PARENT_SECRET");
    // Test-only env mutation: isolated to this process and restored on drop.
    unsafe {
        std::env::set_var("JCODE_ACP_PARENT_SECRET", "must-not-leak");
    }
    struct RestoreEnv(Option<std::ffi::OsString>);
    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            unsafe {
                match self.0.take() {
                    Some(value) => std::env::set_var("JCODE_ACP_PARENT_SECRET", value),
                    None => std::env::remove_var("JCODE_ACP_PARENT_SECRET"),
                }
            }
        }
    }
    let _restore = RestoreEnv(previous_secret);
    let policy = TestPolicy::new("happy").mutate_config();
    let log_path = policy.log_path.clone();
    let cwd = policy.process.cwd.clone().unwrap();
    let auth_seen = Arc::clone(&policy.auth_methods_seen);
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));

    provider.prefetch_models().await.expect("prefetch models");
    assert_eq!(provider.model(), "model-a");
    assert_eq!(
        provider.available_models_display(),
        vec!["model-a", "model-b"]
    );
    assert_eq!(
        auth_seen.lock().unwrap().as_slice(),
        &[vec!["other".to_string(), "cached".to_string()]]
    );

    let prefetched = read_log(&log_path);
    let process = prefetched
        .iter()
        .find_map(|entry| entry.get("fakeProcess"))
        .expect("process record");
    assert_eq!(process["args"], json!(["literal;not-a-shell"]));
    assert_eq!(process["cwd"], json!(fs::canonicalize(cwd).unwrap()));
    assert_eq!(process["marker"], json!("literal-env"));
    assert_eq!(
        process["sawParentSecret"],
        json!(false),
        "ACP child must not inherit parent secrets outside the allowlist"
    );
    let initialize = prefetched
        .iter()
        .find(|entry| entry.get("method") == Some(&json!("initialize")))
        .expect("initialize call");
    assert_eq!(
        initialize.pointer("/params/clientCapabilities"),
        Some(&json!({
            "auth":{"terminal":false},
            "fs":{"readTextFile":false,"writeTextFile":false},
            "terminal":false
        }))
    );
    assert_eq!(
        initialize.pointer("/params/_meta/client/opaque"),
        Some(&json!([1, true, null]))
    );

    provider.set_model("model-b").expect("select model");
    let first = collect_turn(&provider, None).await;
    assert!(
        first
            .iter()
            .any(|event| matches!(event, StreamEvent::SessionId(id) if id == "fake-session-new"))
    );
    assert!(
        first
            .iter()
            .any(|event| matches!(event, StreamEvent::TextDelta(text) if text == "ACP_TEST_OK"))
    );
    assert!(
        first
            .iter()
            .any(|event| matches!(event, StreamEvent::ThinkingDelta(text) if text == "thinking"))
    );
    assert!(first.iter().any(|event| matches!(event, StreamEvent::StatusDetail { detail } if detail == "provider tool running")));
    assert_eq!(
        first
            .iter()
            .filter(|event| matches!(event, StreamEvent::MessageEnd { .. }))
            .count(),
        1
    );
    assert!(
        error_text(&first).is_none(),
        "unexpected first-turn error: {:?}",
        error_text(&first)
    );

    let resumed = collect_turn(&provider, Some("persisted-session")).await;
    assert!(
        resumed
            .iter()
            .any(|event| matches!(event, StreamEvent::SessionId(id) if id == "persisted-session"))
    );
    assert_eq!(
        resumed
            .iter()
            .filter(|event| matches!(event, StreamEvent::MessageEnd { .. }))
            .count(),
        1
    );
    assert!(
        error_text(&resumed).is_none(),
        "unexpected resume error: {:?}",
        error_text(&resumed)
    );

    let log = read_log(&log_path);
    let calls = methods(&log);
    assert_eq!(
        calls
            .iter()
            .filter(|method| **method == "session/set_model")
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|method| **method == "session/set_config_option")
            .count(),
        2
    );
    assert!(calls.contains(&"session/new"));
    assert!(calls.contains(&"session/resume"));
    assert!(!calls.contains(&"session/load"));
    assert!(log.iter().any(|entry| {
        entry.get("method") == Some(&json!("authenticate"))
            && entry.pointer("/params/_meta/auth") == Some(&json!("opaque"))
    }));
    assert!(log.iter().any(|entry| {
        entry.get("method") == Some(&json!("session/set_model"))
            && entry.pointer("/params/_meta/model") == Some(&json!("opaque"))
    }));
    assert!(log.iter().any(|entry| {
        entry.get("method") == Some(&json!("session/set_config_option"))
            && entry.pointer("/params/_meta/config") == Some(&json!("opaque"))
    }));
    let prompts: Vec<_> = log
        .iter()
        .filter(|entry| entry.get("method") == Some(&json!("session/prompt")))
        .collect();
    assert_eq!(prompts.len(), 2);
    assert_eq!(
        prompts[0].pointer("/params/prompt/0/_meta/prompt"),
        Some(&json!("opaque"))
    );
    assert_eq!(
        prompts[0].pointer("/params/prompt/0/text"),
        Some(&json!("resumed=false;latest=latest prompt"))
    );
    assert_eq!(
        prompts[1].pointer("/params/prompt/0/text"),
        Some(&json!("resumed=true;latest=latest prompt"))
    );
    assert!(!prompts[1].to_string().contains("old history"));
}

/// Cursor security review on PR #147: with `cwd: None` (the production
/// workspace-scoped runtime shape), the ACP session root must come from the
/// host session's working directory bound via
/// `Provider::set_session_working_dir`, never silently from the daemon
/// process cwd.
#[tokio::test(flavor = "current_thread")]
async fn session_new_and_resume_cwd_come_from_bound_session_working_dir() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_path = workspace.path().to_path_buf();
    let policy = TestPolicy::new("happy").no_process_cwd();
    let log_path = policy.log_path.clone();
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));
    provider.set_session_working_dir(Some(workspace.path()));

    let first = collect_turn(&provider, None).await;
    assert!(
        error_text(&first).is_none(),
        "unexpected first-turn error: {:?}",
        error_text(&first)
    );
    let resumed = collect_turn(&provider, Some("persisted-session")).await;
    assert!(
        error_text(&resumed).is_none(),
        "unexpected resume error: {:?}",
        error_text(&resumed)
    );

    let log = read_log(&log_path);
    let expected = json!(workspace_path.display().to_string());
    let new_session = log
        .iter()
        .find(|entry| entry.get("method") == Some(&json!("session/new")))
        .expect("session/new request");
    assert_eq!(
        new_session.pointer("/params/cwd"),
        Some(&expected),
        "session/new cwd must be the bound session workspace"
    );
    let resume = log
        .iter()
        .find(|entry| entry.get("method") == Some(&json!("session/resume")))
        .expect("session/resume request");
    assert_eq!(
        resume.pointer("/params/cwd"),
        Some(&expected),
        "session/resume cwd must be the bound session workspace"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn config_options_can_supply_the_model_catalog() {
    let policy = TestPolicy::new("config_catalog");
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));
    let events = collect_turn(&provider, None).await;
    assert!(
        error_text(&events).is_none(),
        "unexpected error: {:?}",
        error_text(&events)
    );
    assert_eq!(provider.model(), "model-b");
    assert_eq!(
        provider.available_models_display(),
        vec!["model-a", "model-b"]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn permission_payload_and_opaque_choice_round_trip_without_loss() {
    let policy = TestPolicy::new("permission");
    let log_path = policy.log_path.clone();
    let broker = Arc::new(RecordingBroker::new(AcpPermissionDecision::Select {
        option_id: "plan.choice/opaque:β-17".to_string(),
    }));
    let requests = Arc::clone(&broker.requests);
    let provider = test_provider(
        policy,
        Some(broker),
        Duration::from_secs(2),
        Duration::from_secs(1),
    );
    let events = collect_turn(&provider, None).await;
    assert!(
        error_text(&events).is_none(),
        "unexpected error: {:?}",
        error_text(&events)
    );

    let request = requests
        .lock()
        .unwrap()
        .first()
        .cloned()
        .expect("permission request");
    assert_eq!(request.provider, "test-acp");
    assert_eq!(request.provider_session_id, "fake-session-new");
    assert_eq!(request.tool_call_id, "permission-tool");
    assert_eq!(request.title, "Choose an action");
    assert_eq!(
        request.kind,
        jcode_provider_acp_runtime::AcpToolKind::Execute
    );
    assert_eq!(request.raw_input, Some(json!({"command":["echo","safe"]})));
    assert_eq!(
        request.content[0].value,
        json!({"type":"content","content":{"type":"text","text":"details"}})
    );
    assert_eq!(request.locations[0].path, PathBuf::from("/tmp/example"));
    assert_eq!(request.locations[0].line, Some(7));
    assert_eq!(request.locations[0].meta, Some(json!({"location":"kept"})));
    assert_eq!(request.options.len(), 4);
    assert_eq!(request.options[3].option_id, "plan.choice/opaque:β-17");
    assert_eq!(request.options[3].meta, Some(json!({"choice":3})));
    assert_eq!(request.meta, Some(json!({"permission":{"nested":[1,2,3]}})));

    let log = read_log(&log_path);
    assert_eq!(
        permission_reply(&log).pointer("/result/outcome/optionId"),
        Some(&json!("plan.choice/opaque:β-17"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn permission_failures_reject_or_cancel_and_never_persist_grants() {
    let cases: Vec<(&str, Option<Arc<dyn AcpPermissionBroker>>)> = vec![
        ("missing", None),
        (
            "unknown",
            Some(Arc::new(RecordingBroker::new(
                AcpPermissionDecision::Select {
                    option_id: "not-advertised".to_string(),
                },
            ))),
        ),
        (
            "allow-always",
            Some(Arc::new(RecordingBroker::new(
                AcpPermissionDecision::Select {
                    option_id: "allow_always".to_string(),
                },
            ))),
        ),
        (
            "timeout",
            Some(Arc::new(
                RecordingBroker::new(AcpPermissionDecision::Select {
                    option_id: "allow_once".to_string(),
                })
                .delayed(Duration::from_secs(3)),
            )),
        ),
        (
            "explicit-reject",
            Some(Arc::new(RecordingBroker::new(
                AcpPermissionDecision::Select {
                    option_id: "reject_once".to_string(),
                },
            ))),
        ),
    ];

    for (label, broker) in cases {
        let policy = TestPolicy::new("permission");
        let log_path = policy.log_path.clone();
        let provider = test_provider(
            policy,
            broker,
            Duration::from_secs(2),
            Duration::from_secs(4),
        );
        let events = collect_turn(&provider, None).await;
        assert!(
            error_text(&events).is_none(),
            "{label}: {:?}",
            error_text(&events)
        );
        let log = read_log(&log_path);
        assert_eq!(
            permission_reply(&log).pointer("/result/outcome/optionId"),
            Some(&json!("reject_once")),
            "{label}"
        );
    }

    let policy = TestPolicy::new("permission_no_reject");
    let log_path = policy.log_path.clone();
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));
    let events = collect_turn(&provider, None).await;
    assert!(error_text(&events).is_none());
    let log = read_log(&log_path);
    assert_eq!(
        permission_reply(&log).pointer("/result/outcome/outcome"),
        Some(&json!("cancelled"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn negotiation_auth_and_process_failures_are_clear_and_bounded() {
    for (scenario, expected) in [
        ("unsupported_version", "unsupported ACP protocol version"),
        ("malformed_json", "shut down unexpectedly"),
        ("child_exit", "shut down unexpectedly"),
        ("auth_missing", "cached authentication is required"),
    ] {
        let policy = TestPolicy::new(scenario);
        let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));
        let error = provider.prefetch_models().await.expect_err(scenario);
        let rendered = format!("{error:#}");
        assert!(
            rendered
                .to_ascii_lowercase()
                .contains(&expected.to_ascii_lowercase()),
            "{scenario}: {rendered}"
        );
        assert!(
            rendered.contains("test login hint"),
            "{scenario}: {rendered}"
        );
    }

    let policy = TestPolicy::new("happy").auth_mode(AuthMode::Terminal);
    let log_path = policy.log_path.clone();
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));
    let error = provider
        .prefetch_models()
        .await
        .expect_err("terminal auth must stop");
    assert!(format!("{error:#}").contains("terminal authentication required"));
    assert!(!methods(&read_log(&log_path)).contains(&"authenticate"));

    let policy = TestPolicy::new("stderr_failure");
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));
    let error = provider
        .prefetch_models()
        .await
        .expect_err("stderr failure");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("ACP stderr: bounded diagnostic before initialize"),
        "{rendered}"
    );
    assert!(
        !rendered.contains(&"x".repeat(100)),
        "stderr was not bounded: {rendered}"
    );

    let policy = TestPolicy::new("oversized_rpc_error");
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));
    let error = provider
        .prefetch_models()
        .await
        .expect_err("oversized protocol failure");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("oversized protocol diagnostic"),
        "{rendered}"
    );
    assert!(
        !rendered.contains(&"x".repeat(100)),
        "protocol diagnostic was not bounded: {rendered}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn initialize_and_prompt_timeouts_are_distinct_and_prompt_timeout_cancels() {
    let policy = TestPolicy::new("slow_initialize");
    let provider = test_provider(
        policy,
        None,
        Duration::from_millis(50),
        Duration::from_secs(1),
    );
    let error = provider
        .prefetch_models()
        .await
        .expect_err("initialize timeout");
    assert!(format!("{error:#}").contains("initialize timed out"));

    let policy = TestPolicy::new("prompt_hang");
    let log_path = policy.log_path.clone();
    let provider = test_provider(
        policy,
        None,
        Duration::from_secs(2),
        Duration::from_millis(50),
    );
    let events = collect_turn(&provider, None).await;
    assert!(
        error_text(&events).is_some_and(|error| error.contains("session/prompt timed out")),
        "{events:?}"
    );
    assert!(methods(&read_log(&log_path)).contains(&"session/cancel"));
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_resume_is_surfaced_and_stream_drop_cancels_without_blocking() {
    let policy = TestPolicy::new("unknown_resume");
    let log_path = policy.log_path.clone();
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(1));
    let events = collect_turn(&provider, Some("missing-session")).await;
    assert!(
        error_text(&events).is_some_and(|error| error.contains("unknown session")),
        "{events:?}"
    );
    let log = read_log(&log_path);
    let calls = methods(&log);
    assert!(calls.contains(&"session/resume"));
    assert!(!calls.contains(&"session/load"));

    let policy = TestPolicy::new("prompt_hang").auth_mode(AuthMode::None);
    let log_path = policy.log_path.clone();
    let provider = test_provider(
        policy,
        None,
        Duration::from_secs(2),
        Duration::from_secs(10),
    );
    let messages = [Message::user("cancel me")];
    let mut stream = provider
        .complete(&messages, &[], "system", None)
        .await
        .expect("start cancellable turn");
    let first = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("session id timeout")
        .expect("session id event")
        .expect("session id result");
    assert!(matches!(first, StreamEvent::SessionId(_)));
    let started = Instant::now();
    drop(stream);
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "stream Drop blocked"
    );

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if log_path.exists() && methods(&read_log(&log_path)).contains(&"session/cancel") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("cancel notification was logged");
}

#[tokio::test(flavor = "current_thread")]
async fn prompt_completion_cannot_overtake_updates_under_backpressure() {
    let policy = TestPolicy::new("happy").auth_mode(AuthMode::None);
    let provider = test_provider_with_buffer(
        policy,
        None,
        Duration::from_secs(2),
        Duration::from_secs(1),
        1,
    );
    let messages = [Message::user("backpressure")];
    let mut stream = provider
        .complete(&messages, &[], "system", None)
        .await
        .expect("start backpressured turn");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let events = tokio::time::timeout(Duration::from_secs(2), async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event.expect("backpressured event"));
        }
        events
    })
    .await
    .expect("backpressured stream completed");
    assert!(matches!(events.first(), Some(StreamEvent::SessionId(_))));
    let thinking = events
        .iter()
        .position(|event| matches!(event, StreamEvent::ThinkingDelta(text) if text == "thinking"))
        .expect("thinking update");
    let status = events
        .iter()
        .position(|event| matches!(event, StreamEvent::StatusDetail { detail } if detail == "provider tool running"))
        .expect("tool update");
    let text = events
        .iter()
        .position(|event| matches!(event, StreamEvent::TextDelta(value) if value == "ACP_TEST_OK"))
        .expect("text update");
    let end = events
        .iter()
        .position(|event| matches!(event, StreamEvent::MessageEnd { .. }))
        .expect("message end");
    assert!(
        thinking < status && status < text && text < end,
        "{events:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn stream_drop_cancels_during_initialize() {
    let policy = TestPolicy::new("cancel_initialize").auth_mode(AuthMode::None);
    let log_path = policy.log_path.clone();
    let provider = test_provider(policy, None, Duration::from_secs(5), Duration::from_secs(5));
    let messages = [Message::user("cancel setup")];
    let stream = provider
        .complete(&messages, &[], "system", None)
        .await
        .expect("start setup cancellation turn");
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if log_path.exists() && methods(&read_log(&log_path)).contains(&"initialize") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("initialize request was logged");
    drop(stream);
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert!(
        !read_log(&log_path)
            .iter()
            .any(|entry| entry.get("cancelInitializeCompleted") == Some(&json!(true))),
        "initialization continued after stream drop"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn stream_drop_cancels_a_pending_known_session_resume() {
    let policy = TestPolicy::new("resume_hang").auth_mode(AuthMode::None);
    let log_path = policy.log_path.clone();
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(5));
    let messages = [Message::user("cancel resume")];
    let stream = provider
        .complete(&messages, &[], "system", Some("known-session"))
        .await
        .expect("start resume cancellation turn");
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if log_path.exists() && methods(&read_log(&log_path)).contains(&"session/resume") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("resume request was logged");
    drop(stream);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if methods(&read_log(&log_path)).contains(&"session/cancel") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("resume cancellation was logged");
}

#[tokio::test(flavor = "current_thread")]
async fn stream_drop_cancels_during_session_setup() {
    let policy = TestPolicy::new("set_model_hang").auth_mode(AuthMode::None);
    let log_path = policy.log_path.clone();
    let provider = test_provider(policy, None, Duration::from_secs(2), Duration::from_secs(5));
    provider.set_model("model-b").expect("select setup model");
    let messages = [Message::user("cancel model setup")];
    let mut stream = provider
        .complete(&messages, &[], "system", None)
        .await
        .expect("start setup cancellation turn");
    let first = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("session id timeout")
        .expect("session id event")
        .expect("session id result");
    assert!(matches!(first, StreamEvent::SessionId(_)));
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if methods(&read_log(&log_path)).contains(&"session/set_model") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("model setup request was logged");
    drop(stream);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if methods(&read_log(&log_path)).contains(&"session/cancel") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("setup cancellation was logged");
}

#[tokio::test(flavor = "current_thread")]
async fn stream_drop_cancels_pending_permission_before_teardown() {
    let policy = TestPolicy::new("permission").auth_mode(AuthMode::None);
    let log_path = policy.log_path.clone();
    let broker = Arc::new(
        RecordingBroker::new(AcpPermissionDecision::Select {
            option_id: "allow_once".to_string(),
        })
        .delayed(Duration::from_secs(10)),
    );
    let requests = Arc::clone(&broker.requests);
    let provider = test_provider(
        policy,
        Some(broker),
        Duration::from_secs(30),
        Duration::from_secs(30),
    );
    let messages = [Message::user("cancel permission")];
    let mut stream = provider
        .complete(&messages, &[], "system", None)
        .await
        .expect("start permission cancellation turn");
    let first = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("session id timeout")
        .expect("session id event")
        .expect("session id result");
    assert!(matches!(first, StreamEvent::SessionId(_)));
    tokio::time::timeout(Duration::from_secs(1), async {
        while requests.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("permission broker was called");
    drop(stream);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if log_path.exists() {
                let log = read_log(&log_path);
                if methods(&log).contains(&"session/cancel")
                    && log.iter().any(|entry| {
                        entry.get("id") == Some(&json!(900))
                            && entry.pointer("/result/outcome/outcome") == Some(&json!("cancelled"))
                    })
                {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("pending permission was cancelled before teardown");
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn stream_drop_terminates_and_reaps_a_stalled_child() {
    let policy = TestPolicy::new("cancel_stall").auth_mode(AuthMode::None);
    let log_path = policy.log_path.clone();
    let provider = test_provider(
        policy,
        None,
        Duration::from_secs(2),
        Duration::from_secs(30),
    );
    let messages = [Message::user("reap stalled child")];
    let mut stream = provider
        .complete(&messages, &[], "system", None)
        .await
        .expect("start stalled turn");
    let first = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .expect("session id timeout")
        .expect("session id event")
        .expect("session id result");
    assert!(matches!(first, StreamEvent::SessionId(_)));
    let pid = read_log(&log_path)
        .iter()
        .find_map(|entry| entry.pointer("/fakeProcess/pid").and_then(Value::as_u64))
        .expect("fake ACP pid");
    assert!(process_is_alive(pid), "fake ACP child was not running");

    let started = Instant::now();
    drop(stream);
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "stream Drop blocked"
    );
    // Cancellation cleanup may consume the full 2-second request timeout
    // before the runtime force-kills and reaps the deliberately stalled agent.
    tokio::time::timeout(Duration::from_secs(4), async {
        while process_is_alive(pid) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("stalled ACP child was not reaped");
    assert!(
        !read_log(&log_path)
            .iter()
            .any(|entry| entry.get("cancelStallCompleted") == Some(&json!(true))),
        "stalled ACP child completed instead of being terminated"
    );
}
