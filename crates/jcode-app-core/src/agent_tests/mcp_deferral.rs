use super::*;
use crate::config::McpToolsMode;
use crate::mcp::{McpConfig, McpManager, McpToolDef, create_mcp_tools_from_cached};
use crate::tool::{MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME};
use jcode_provider_core::ToolNameLimit;

async fn add_proxy(registry: &Registry, name: &str, description_len: usize) {
    let manager = Arc::new(tokio::sync::RwLock::new(McpManager::with_config(
        McpConfig::default(),
    )));
    let def = McpToolDef {
        name: name.into(),
        description: Some("x".repeat(description_len)),
        input_schema: serde_json::json!({"type":"object"}),
    };
    for (name, proxy) in create_mcp_tools_from_cached("probe", &[def], manager) {
        registry.register(name, proxy).await;
    }
}

async fn agent(mode: McpToolsMode, threshold: usize) -> Agent {
    let provider: Arc<dyn Provider> = Arc::new(tool_name_limit::LimitedNameProvider::with(
        ToolNameLimit::PROBED_128,
    ));
    let registry = Registry::empty();
    let manager = Arc::new(tokio::sync::RwLock::new(McpManager::with_config(
        McpConfig::default(),
    )));
    crate::tool::mcp::register_fixed_mcp_surface(&registry, &manager).await;
    let mut agent = Agent::new(provider, registry);
    agent.mcp_tools_mode = mode;
    agent.mcp_tools_token_threshold = threshold;
    agent
}

fn names(tools: &[ToolDefinition]) -> Vec<&str> {
    tools.iter().map(|tool| tool.name.as_str()).collect()
}

fn policy(agent: &mut Agent, allowed: Option<&[&str]>, disabled: &[&str]) {
    agent.allowed_tools = allowed.map(|names| names.iter().map(|name| (*name).into()).collect());
    agent.disabled_tools = disabled.iter().map(|name| (*name).into()).collect();
    crate::tool::set_session_tool_policy(
        agent.session_id(),
        agent.allowed_tools.clone(),
        agent.disabled_tools.clone(),
    );
    agent.unlock_tools();
}

#[tokio::test]
async fn modes_and_empty_catalog_keep_exact_surfaces() {
    let _lock = crate::storage::lock_test_env();
    for mode in [
        McpToolsMode::Eager,
        McpToolsMode::Deferred,
        McpToolsMode::Auto,
    ] {
        let mut agent = agent(mode, 0).await;
        assert_eq!(agent.tool_definitions().await.len(), 0);
        add_proxy(&agent.registry, "x", 200).await;
        agent.unlock_tools();
        let tools = agent.tool_definitions().await;
        assert_eq!(
            names(&tools),
            if mode == McpToolsMode::Eager {
                vec!["mcp__probe__x"]
            } else {
                vec![MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME]
            }
        );
        assert_eq!(
            names(&agent.tool_definitions_for_debug().await),
            names(&tools)
        );
    }
}

#[tokio::test]
async fn status_identities_follow_current_catalog_and_policy_not_exposure() {
    let _lock = crate::storage::lock_test_env();
    for mode in [
        McpToolsMode::Deferred,
        McpToolsMode::Auto,
        McpToolsMode::Eager,
    ] {
        let mut agent = agent(mode, 0).await;
        let long = "x".repeat(140);
        let long_key = format!("mcp__probe__{long}");
        for name in ["x", "y", "denied", &long] {
            add_proxy(&agent.registry, name, 200).await;
        }
        policy(&mut agent, None, &["mcp__probe__denied"]);
        agent.tool_definitions().await;
        add_proxy(&agent.registry, "late", 200).await;

        for (allowed, expected) in [
            (None, vec!["late", "x", "y"]),
            (
                Some(vec!["mcp__probe__x", "mcp__probe__denied", &long_key]),
                vec!["x"],
            ),
            (Some(vec![MCP_CALL_TOOL_NAME]), vec!["late", "x", "y"]),
        ] {
            if allowed.is_some() {
                policy(&mut agent, allowed.as_deref(), &["mcp__probe__denied"]);
                agent.tool_definitions().await;
            }
            assert_eq!(
                agent.mcp_tool_identities().await,
                expected
                    .into_iter()
                    .map(|tool| (format!("mcp__probe__{tool}"), "probe".into(), tool.into()))
                    .collect::<Vec<_>>(),
                "status identities must not depend on {mode:?} exposure or the locked snapshot"
            );
        }
    }
}

#[tokio::test]
async fn auto_defers_only_above_the_threshold() {
    let _lock = crate::storage::lock_test_env();
    let mut agent = agent(McpToolsMode::Auto, usize::MAX).await;
    add_proxy(&agent.registry, "x", 200).await;
    let eager = agent.tool_definitions().await;
    let threshold = ToolDefinition::aggregate_prompt_token_estimate(&eager);
    for (threshold, deferred) in [(threshold, false), (threshold - 1, true)] {
        agent.unlock_tools();
        agent.mcp_tools_token_threshold = threshold;
        assert_eq!(
            Agent::locked_uses_fixed_mcp_surface(&agent.tool_definitions().await),
            deferred
        );
    }
}

#[tokio::test]
async fn fixed_surface_keeps_latch_and_cache_across_registration_and_route_switches() {
    let _lock = crate::storage::lock_test_env();
    let mut agent = agent(McpToolsMode::Deferred, 0).await;
    add_proxy(&agent.registry, "x", 200).await;
    let long = "long".repeat(24);
    add_proxy(&agent.registry, &long, 200).await;
    let first = agent.tool_definitions().await;
    agent.cache_tracker.record_prefix_hashes(&[1]);
    add_proxy(&agent.registry, "late", 30000).await;
    assert_eq!(names(&agent.tool_definitions().await), names(&first));
    assert!(!agent.mcp_late_register_latched());
    assert_eq!(agent.cache_tracker.turn_count(), 1);
    for (route, excluded) in [("narrow", true), ("wide", false)] {
        agent.set_model_from_auth(route).unwrap();
        assert_eq!(names(&agent.tool_definitions().await), names(&first));
        assert_eq!(agent.cache_tracker.turn_count(), 1);
        assert!(!agent.mcp_late_register_latched());
        assert_eq!(
            agent
                .name_excluded_tool_names()
                .contains(&format!("mcp__probe__{long}")),
            excluded
        );
    }
    let late_long = format!("mcp__probe__{}", "x".repeat(140));
    crate::tool::session_mcp_dispatch_is_allowed(agent.session_id(), &late_long).unwrap_err();
}

#[tokio::test]
async fn explicit_fixed_tools_pick_up_a_late_catalog_in_every_mode() {
    let _lock = crate::storage::lock_test_env();
    for mode in [
        McpToolsMode::Eager,
        McpToolsMode::Deferred,
        McpToolsMode::Auto,
    ] {
        let mut agent = agent(mode, usize::MAX).await;
        policy(
            &mut agent,
            Some(&[MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME]),
            &[],
        );
        assert_eq!(agent.tool_definitions().await.len(), 0);
        add_proxy(&agent.registry, "late", 100).await;
        assert_eq!(
            names(&agent.tool_definitions().await),
            vec![MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME]
        );
        assert!(agent.mcp_late_register_latched());
        agent.cache_tracker.record_prefix_hashes(&[1]);
        add_proxy(&agent.registry, "later", 100).await;
        assert_eq!(
            names(&agent.tool_definitions().await),
            vec![MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME]
        );
        assert_eq!(agent.cache_tracker.turn_count(), 1);
    }
}

#[tokio::test]
async fn auto_reestimates_only_at_the_one_shot_latch_or_explicit_unlock() {
    let _lock = crate::storage::lock_test_env();
    let mut agent = agent(McpToolsMode::Auto, 1000).await;
    add_proxy(&agent.registry, "x", 100).await;
    assert_eq!(
        names(&agent.tool_definitions().await),
        vec!["mcp__probe__x"]
    );
    agent.cache_tracker.record_prefix_hashes(&[1]);
    add_proxy(&agent.registry, "verbose", 8000).await;
    let fixed = agent.tool_definitions().await;
    assert_eq!(
        names(&fixed),
        vec![MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME]
    );
    assert!(agent.mcp_late_register_latched());
    assert_eq!(agent.cache_tracker.turn_count(), 0);
    agent.cache_tracker.record_prefix_hashes(&[1]);
    add_proxy(&agent.registry, "second_wave", 40000).await;
    assert_eq!(names(&agent.tool_definitions().await), names(&fixed));
    assert_eq!(agent.cache_tracker.turn_count(), 1);

    // If the one accepted rebuild stayed eager, a later threshold crossing
    // must wait for explicit reload instead of creating a second cache miss.
    let mut eager = self::agent(McpToolsMode::Auto, 1000).await;
    add_proxy(&eager.registry, "x", 100).await;
    eager.tool_definitions().await;
    add_proxy(&eager.registry, "small", 100).await;
    let once = eager.tool_definitions().await;
    add_proxy(&eager.registry, "verbose", 8000).await;
    assert_eq!(names(&eager.tool_definitions().await), names(&once));
    eager.unlock_tools();
    assert_eq!(
        names(&eager.tool_definitions().await),
        vec![MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME]
    );
}

#[tokio::test]
async fn exposure_obeys_allow_and_deny_policy() {
    let _lock = crate::storage::lock_test_env();
    let mut agent = agent(McpToolsMode::Deferred, 0).await;
    add_proxy(&agent.registry, "x", 100).await;
    add_proxy(&agent.registry, "y", 100).await;
    policy(&mut agent, Some(&["mcp__probe__x"]), &[]);
    let tools = agent.tool_definitions().await;
    assert_eq!(
        names(&tools),
        vec![MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME]
    );
    agent.validate_tool_allowed(MCP_CALL_TOOL_NAME).unwrap();
    agent.validate_tool_allowed(MCP_SEARCH_TOOL_NAME).unwrap();
    agent.validate_tool_allowed("mcp__probe__y").unwrap_err();
    policy(&mut agent, Some(&["mcp__probe__x"]), &[MCP_CALL_TOOL_NAME]);
    assert_eq!(
        names(&agent.tool_definitions().await),
        vec![MCP_SEARCH_TOOL_NAME]
    );
    agent.validate_tool_allowed(MCP_CALL_TOOL_NAME).unwrap_err();
    policy(
        &mut agent,
        Some(&["mcp__probe__x"]),
        &[MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME],
    );
    assert_eq!(agent.tool_definitions().await.len(), 0);
}

#[tokio::test]
async fn exposure_applies_allow_and_transport_filters_before_estimation() {
    let _lock = crate::storage::lock_test_env();
    let mut agent = agent(McpToolsMode::Deferred, 0).await;
    add_proxy(&agent.registry, "x", 100).await;
    add_proxy(&agent.registry, "y", 100).await;
    policy(&mut agent, Some(&["bash"]), &[]);
    assert_eq!(agent.tool_definitions().await.len(), 0);

    policy(&mut agent, None, &[]);
    agent.mcp_tools_mode = McpToolsMode::Auto;
    agent.mcp_tools_token_threshold = 1000;
    add_proxy(&agent.registry, &"long".repeat(40), 30000).await;
    let tools = agent.tool_definitions().await;
    assert_eq!(names(&tools), vec!["mcp__probe__x", "mcp__probe__y"]);
    assert_eq!(
        names(&agent.tool_definitions_for_debug().await),
        names(&tools)
    );
}

#[tokio::test]
async fn agent_snapshots_mode_and_threshold_at_construction() {
    let _lock = crate::storage::lock_test_env();
    let _mode = crate::storage::EnvVarGuard::set("JCODE_MCP_TOOLS", "deferred");
    let _threshold = crate::storage::EnvVarGuard::set("JCODE_MCP_TOOLS_TOKEN_THRESHOLD", "0");
    crate::config::Config::invalidate_cache();
    let provider: Arc<dyn Provider> = Arc::new(NativeAutoCompactionProvider);
    let registry = Registry::empty();
    let manager = Arc::new(tokio::sync::RwLock::new(McpManager::with_config(
        McpConfig::default(),
    )));
    crate::tool::mcp::register_fixed_mcp_surface(&registry, &manager).await;
    let mut agent = Agent::new(provider, registry);
    crate::env::set_var("JCODE_MCP_TOOLS", "eager");
    crate::config::Config::invalidate_cache();
    add_proxy(&agent.registry, "x", 100).await;
    assert_eq!(
        names(&agent.tool_definitions().await),
        vec![MCP_CALL_TOOL_NAME, MCP_SEARCH_TOOL_NAME]
    );
}
