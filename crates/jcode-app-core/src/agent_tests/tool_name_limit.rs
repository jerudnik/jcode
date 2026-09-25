use super::*;
use crate::mcp::{McpManager, McpToolDef, create_mcp_tools_from_cached};
use jcode_provider_core::{ProviderCapabilities, ToolNameLimit};

/// Provider whose only interesting property is its tool-name limit. The
/// model name selects the limit so a test can switch transports through the
/// agent's normal model-switch path: `wide` is 128, anything else is 64.
struct LimitedNameProvider(Arc<Mutex<ToolNameLimit>>);

impl LimitedNameProvider {
    fn with(limit: ToolNameLimit) -> Self {
        Self(Arc::new(Mutex::new(limit)))
    }

    fn limit(&self) -> ToolNameLimit {
        *self.0.lock().expect("limit lock")
    }
}

#[async_trait]
impl Provider for LimitedNameProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        let (_tx, rx) = tokio_mpsc::channel::<Result<StreamEvent>>(1);
        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    fn name(&self) -> &str {
        "limited"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            reasoning_context_replay: false,
            tool_name_limit: self.limit(),
        }
    }

    fn set_model(&self, model: &str) -> Result<()> {
        *self.0.lock().expect("limit lock") = if model == "wide" {
            ToolNameLimit::PROBED_128
        } else {
            ToolNameLimit::CONSERVATIVE
        };
        Ok(())
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(Self(Arc::clone(&self.0)))
    }
}

fn padded_tool_name(composed_len: usize, server: &str) -> String {
    let prefix_len = format!("mcp__{server}__").len();
    let mut name = String::from("sequential_thinking-");
    while name.len() < composed_len - prefix_len {
        name.push_str("abc_xyz-");
    }
    name.truncate(composed_len - prefix_len);
    if name.ends_with(['-', '_']) {
        name.pop();
        name.push('z');
    }
    name
}

async fn agent_with_mcp_tools(limit: ToolNameLimit, tools: &[&str]) -> Agent {
    let provider: Arc<dyn Provider> = Arc::new(LimitedNameProvider::with(limit));
    let registry = Registry::new(provider.clone()).await;
    let manager = Arc::new(tokio::sync::RwLock::new(McpManager::new()));
    let defs: Vec<McpToolDef> = tools
        .iter()
        .map(|name| McpToolDef {
            name: name.to_string(),
            description: None,
            input_schema: serde_json::json!({"type": "object"}),
        })
        .collect();
    for (name, proxy) in create_mcp_tools_from_cached("probe", &defs, manager) {
        registry.register(name, proxy).await;
    }
    Agent::new(provider, registry)
}

#[tokio::test]
async fn transport_limit_withholds_over_long_names_and_refuses_their_execution() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());
    crate::config::Config::invalidate_cache();

    let ok = padded_tool_name(128, "probe");
    let over = padded_tool_name(129, "probe");
    let mut agent = agent_with_mcp_tools(ToolNameLimit::PROBED_128, &[&ok, &over]).await;

    let names: Vec<String> = agent
        .tool_definitions()
        .await
        .into_iter()
        .map(|def| def.name)
        .collect();
    let ok_key = format!("mcp__probe__{ok}");
    let over_key = format!("mcp__probe__{over}");
    assert_eq!(ok_key.len(), 128);
    assert_eq!(over_key.len(), 129);
    assert!(names.contains(&ok_key), "128-char name is advertised");
    assert!(!names.contains(&over_key), "129-char name is withheld");

    // Withheld at execution too, not only from the advertised list.
    agent
        .validate_tool_allowed(&ok_key)
        .expect("advertised name executes");
    let err = agent
        .validate_tool_allowed(&over_key)
        .expect_err("withheld name must not execute");
    assert!(err.to_string().contains("not advertised on this transport"));
    assert_eq!(agent.name_excluded_tool_names(), vec![over_key.clone()]);

    // The tool stays registered for transports that allow it.
    assert!(agent.tool_names().await.contains(&over_key));

    crate::env::remove_var("JCODE_HOME");
    crate::config::Config::invalidate_cache();
}

#[tokio::test]
async fn conservative_limit_withholds_names_between_65_and_128() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());
    crate::config::Config::invalidate_cache();

    let sixty_four = padded_tool_name(64, "probe");
    let sixty_five = padded_tool_name(65, "probe");
    let mut agent =
        agent_with_mcp_tools(ToolNameLimit::CONSERVATIVE, &[&sixty_four, &sixty_five]).await;
    let names: Vec<String> = agent
        .tool_definitions()
        .await
        .into_iter()
        .map(|def| def.name)
        .collect();
    assert!(names.contains(&format!("mcp__probe__{sixty_four}")));
    assert!(!names.contains(&format!("mcp__probe__{sixty_five}")));

    crate::env::remove_var("JCODE_HOME");
    crate::config::Config::invalidate_cache();
}

fn direct_ctx(agent: &Agent) -> crate::tool::ToolContext {
    crate::tool::ToolContext {
        session_id: agent.session_id().to_string(),
        message_id: "test".to_string(),
        tool_call_id: "test".to_string(),
        working_dir: None,
        stdin_request_tx: None,
        graceful_shutdown_signal: None,
        execution_mode: crate::tool::ToolExecutionMode::Direct,
    }
}

#[tokio::test]
async fn registry_execute_refuses_name_excluded_tools_for_nested_calls() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());
    crate::config::Config::invalidate_cache();

    let over = padded_tool_name(129, "probe");
    let mut agent = agent_with_mcp_tools(ToolNameLimit::PROBED_128, &[&over]).await;
    agent.tool_definitions().await;
    let over_key = format!("mcp__probe__{over}");

    // `batch` and other nested paths execute through the registry directly,
    // bypassing Agent::validate_tool_allowed; the session policy must carry
    // the exclusion there too.
    let err = agent
        .registry()
        .execute(&over_key, serde_json::json!({}), direct_ctx(&agent))
        .await
        .expect_err("registry refuses a withheld name");
    assert!(
        err.to_string().contains("not advertised on this transport"),
        "unexpected error: {err}"
    );

    crate::env::remove_var("JCODE_HOME");
    crate::config::Config::invalidate_cache();
}

#[tokio::test]
async fn model_switch_rebuilds_snapshot_only_when_the_limit_changes_the_tool_set() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());
    crate::config::Config::invalidate_cache();

    let long = padded_tool_name(100, "probe");
    let long_key = format!("mcp__probe__{long}");
    let mut agent = agent_with_mcp_tools(ToolNameLimit::PROBED_128, &[&long]).await;
    let names: Vec<String> = agent
        .tool_definitions()
        .await
        .into_iter()
        .map(|def| def.name)
        .collect();
    assert!(names.contains(&long_key), "wide transport advertises the 100-char name");

    // Switching to a 64-limit transport must drop the stale snapshot so the
    // next request does not carry a name the new provider rejects.
    agent
        .set_model_from_auth("narrow")
        .expect("switch to narrow transport");
    let names: Vec<String> = agent
        .tool_definitions()
        .await
        .into_iter()
        .map(|def| def.name)
        .collect();
    assert!(!names.contains(&long_key), "narrow transport withholds the 100-char name");
    assert!(agent.validate_tool_allowed(&long_key).is_err());

    // And back: the exclusion is lifted rather than left stale.
    agent
        .set_model_from_auth("wide")
        .expect("switch back to wide transport");
    let names: Vec<String> = agent
        .tool_definitions()
        .await
        .into_iter()
        .map(|def| def.name)
        .collect();
    assert!(names.contains(&long_key));
    agent
        .validate_tool_allowed(&long_key)
        .expect("advertised again after switching back");

    crate::env::remove_var("JCODE_HOME");
    crate::config::Config::invalidate_cache();
}

#[tokio::test]
async fn name_excluded_tools_do_not_consume_the_late_mcp_registration_latch() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    crate::env::set_var("JCODE_HOME", temp_home.path());
    crate::config::Config::invalidate_cache();

    let over = padded_tool_name(129, "probe");
    let mut agent = agent_with_mcp_tools(ToolNameLimit::PROBED_128, &[&over]).await;
    let first = agent.tool_definitions().await;
    // A second call sees the withheld name still absent from the snapshot; it
    // must not be mistaken for a late-registered MCP tool, which would burn
    // the one-shot rebuild before a real late registration arrives.
    let second = agent.tool_definitions().await;
    assert_eq!(first.len(), second.len());
    assert!(
        !agent.mcp_late_register_latched(),
        "withheld names must not trip the late-registration latch"
    );

    // A genuinely late MCP tool still triggers the one-shot rebuild.
    let manager = Arc::new(tokio::sync::RwLock::new(McpManager::new()));
    let late = McpToolDef {
        name: "late".to_string(),
        description: None,
        input_schema: serde_json::json!({"type": "object"}),
    };
    for (name, proxy) in create_mcp_tools_from_cached("probe", &[late], manager) {
        agent.registry().register(name, proxy).await;
    }
    let third = agent.tool_definitions().await;
    assert!(third.iter().any(|def| def.name == "mcp__probe__late"));
    assert!(agent.mcp_late_register_latched());

    crate::env::remove_var("JCODE_HOME");
    crate::config::Config::invalidate_cache();
}
