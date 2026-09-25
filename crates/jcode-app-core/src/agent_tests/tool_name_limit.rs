use super::*;
use crate::mcp::{McpManager, McpToolDef, create_mcp_tools_from_cached};
use jcode_provider_core::{ProviderCapabilities, ToolNameLimit};

/// Provider whose only interesting property is its tool-name limit.
struct LimitedNameProvider(ToolNameLimit);

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
            tool_name_limit: self.0,
        }
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(Self(self.0))
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
    let provider: Arc<dyn Provider> = Arc::new(LimitedNameProvider(limit));
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
