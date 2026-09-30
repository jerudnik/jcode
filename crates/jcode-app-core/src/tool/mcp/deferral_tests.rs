use super::*;
use crate::mcp::{McpConfig, McpSchemaCache, McpToolDef, create_mcp_tools_from_cached};
use crate::tool::{
    Registry, clear_session_tool_policy, set_session_name_exclusions, set_session_tool_policy,
};
use std::collections::HashSet;

fn context(session: &str) -> ToolContext {
    ToolContext {
        session_id: session.into(),
        message_id: "test".into(),
        tool_call_id: "test".into(),
        working_dir: None,
        stdin_request_tx: None,
        graceful_shutdown_signal: None,
        execution_mode: crate::tool::ToolExecutionMode::Direct,
    }
}

fn definition(name: &str) -> McpToolDef {
    McpToolDef {
        name: name.into(),
        description: Some("Find a record".into()),
        input_schema: json!({"type": "object", "properties": {"id": {"type": "integer"}}}),
    }
}

fn config(command: &str) -> McpServerConfig {
    serde_json::from_value(json!({"command": command, "shared": false})).unwrap()
}

#[test]
fn call_schema_preserves_free_form_arguments_and_refuses_strict_mode() {
    use jcode_provider_core::openai_schema::{openai_compatible_schema, schema_supports_strict};
    let manager = Arc::new(RwLock::new(McpManager::with_config(McpConfig::default())));
    let schema = McpCallTool::new(manager).parameters_schema();
    let normalized = openai_compatible_schema(&schema);
    assert_eq!(
        schema["properties"]["arguments"]["additionalProperties"],
        true
    );
    assert_eq!(
        normalized["properties"]["arguments"]["additionalProperties"],
        true
    );
    assert!(!schema_supports_strict(&normalized));
}

#[tokio::test]
async fn fixed_surface_visibility_is_inferred_only_from_exact_proxy_entries() {
    let registry = Registry::empty();
    let manager = Arc::new(RwLock::new(McpManager::with_config(McpConfig::default())));
    register_fixed_mcp_surface(&registry, &manager).await;
    let session = "fixed-policy";
    for (allow, expected) in [
        (None, 2),
        (Some(HashSet::from(["mcp__A__x".into()])), 2),
        (Some(HashSet::from(["bash".into()])), 0),
    ] {
        assert_eq!(registry.definitions(allow.as_ref()).await.len(), expected);
        set_session_tool_policy(session, allow, HashSet::new());
        let result = registry
            .execute("mcp_search", json!({}), context(session))
            .await;
        if expected == 2 {
            assert_eq!(
                serde_json::from_str::<Value>(&result.unwrap().output).unwrap()["count"],
                0
            );
        } else {
            result.expect_err("fixed tool must not bypass the allow list");
        }
    }
    clear_session_tool_policy(session);
}

#[test]
fn fixed_dispatch_policy_keeps_exact_allow_and_deny_precedence() {
    let session = "dispatch-policy";
    set_session_tool_policy(
        session,
        Some(HashSet::from(["mcp__A__x".into()])),
        HashSet::new(),
    );
    crate::tool::session_mcp_dispatch_is_allowed(session, "mcp__A__x").unwrap();
    crate::tool::session_mcp_dispatch_is_allowed(session, "mcp__B__x").unwrap_err();
    for denied in ["mcp__A__x", "mcp_call"] {
        set_session_tool_policy(
            session,
            Some(HashSet::from(["mcp_call".into()])),
            HashSet::from([denied.into()]),
        );
        crate::tool::session_mcp_dispatch_is_allowed(session, "mcp__A__x").unwrap_err();
    }
    clear_session_tool_policy(session);
}

#[test]
fn fixed_dispatch_refuses_name_exclusions_and_has_no_management_umbrella() {
    let session = "excluded-policy";
    let management = HashSet::from(["mcp".into()]);
    assert!(!crate::tool::tool_is_allowed(
        Some(&management),
        crate::tool::MCP_CALL_TOOL_NAME
    ));
    assert!(!crate::tool::tool_is_allowed(
        Some(&management),
        crate::tool::MCP_SEARCH_TOOL_NAME
    ));
    set_session_tool_policy(session, Some(management), HashSet::new());
    crate::tool::session_mcp_dispatch_is_allowed(session, "mcp__A__x").unwrap_err();
    set_session_tool_policy(session, None, HashSet::new());
    set_session_name_exclusions(session, HashSet::from(["mcp__A__x".into()]));
    crate::tool::session_mcp_dispatch_is_allowed(session, "mcp__A__x").unwrap_err();
    crate::tool::session_mcp_dispatch_is_allowed(session, "mcp__B__x").unwrap();
    clear_session_tool_policy(session);
}

#[tokio::test]
async fn search_hides_denied_disabled_stale_and_ambiguous_cached_tools() {
    let _lock = crate::storage::lock_test_env();
    let home = tempfile::tempdir().unwrap();
    let _home = crate::storage::EnvVarGuard::set("JCODE_HOME", home.path());
    let mut configs = McpConfig::default();
    let mut cache = McpSchemaCache::default();
    let catalog = [
        (" A ", "x"),
        ("A", "x"),
        ("B", "x"),
        ("off", "x"),
        ("a", "b__x"),
        ("a__b", "x"),
        ("stale", "x"),
    ];
    for (server, tool) in catalog {
        let mut cfg = config("/nonexistent/mcp-test");
        if server == "off" {
            cfg.enabled = Some(false);
        }
        cache.update(server, &cfg, vec![definition(tool)]);
        if server == "stale" {
            cfg.command.push_str("-changed");
        }
        configs.servers.insert(server.into(), cfg);
    }
    cache.save();
    let manager = Arc::new(RwLock::new(McpManager::with_config(configs)));
    let registry = Registry::empty();
    register_fixed_mcp_surface(&registry, &manager).await;
    for (server, tool) in catalog {
        for (name, proxy) in
            create_mcp_tools_from_cached(server, &[definition(tool)], manager.clone())
        {
            registry.register(name, proxy).await;
        }
    }
    let session = "cached-search";
    let search = |input| registry.execute("mcp_search", input, context(session));
    let all: Value = serde_json::from_str(&search(json!({})).await.unwrap().output).unwrap();
    assert_eq!(
        all["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["mcp__ A __x", "mcp__A__x", "mcp__B__x"]
    );
    let exact: Value =
        serde_json::from_str(&search(json!({"server":" A "})).await.unwrap().output).unwrap();
    assert_eq!(exact["count"], 1);
    assert_eq!(exact["tools"][0]["server"], " A ");
    set_session_tool_policy(
        session,
        Some(HashSet::from(["mcp__A__x".into()])),
        HashSet::new(),
    );
    let found: Value = serde_json::from_str(
        &search(json!({"query":"RECORD", "server":"A"}))
            .await
            .unwrap()
            .output,
    )
    .unwrap();
    assert_eq!(found["count"], 1);
    assert_eq!(
        found["tools"][0]["input_schema"],
        definition("x").input_schema
    );
    assert_eq!(found["tools"][0]["server"], "A");
    let missing: Value =
        serde_json::from_str(&search(json!({"query":"absent"})).await.unwrap().output).unwrap();
    assert_eq!(missing["count"], 0);
    set_session_name_exclusions(session, HashSet::from(["mcp__A__x".into()]));
    let excluded: Value = serde_json::from_str(&search(json!({})).await.unwrap().output).unwrap();
    assert_eq!(excluded["count"], 0);
    clear_session_tool_policy(session);
    // Both sides of the collision are refused even with an unrestricted session.
    for (server, tool) in [("a", "b__x"), ("a__b", "x"), ("off", "x"), ("A", "x")] {
        registry
            .execute(
                "mcp_call",
                json!({"server":server, "tool":tool}),
                context(session),
            )
            .await
            .unwrap_err();
    }
    // A failed cached-only connect does not make the session unusable.
    assert_eq!(
        serde_json::from_str::<Value>(&search(json!({"server":"B"})).await.unwrap().output)
            .unwrap()["count"],
        1
    );
    manager.read().await.disconnect_all().await;
}

#[cfg(unix)]
#[tokio::test]
async fn call_and_eager_proxy_share_wire_arguments_rendering_and_reconnect() {
    let _lock = crate::storage::lock_test_env();
    let home = tempfile::tempdir().unwrap();
    let _home = crate::storage::EnvVarGuard::set("JCODE_HOME", home.path());
    let script = home.path().join("mcp.sh");
    let trace = home.path().join("calls.jsonl");
    let pidfile = home.path().join("pid");
    std::fs::write(&script, r##"#!/bin/sh
echo $$ > "$PIDFILE"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | grep -o '"id":[0-9]*' | grep -o '[0-9]*' | head -1)
  case "$line" in
    *'"initialize"'*) echo '{"jsonrpc":"2.0","id":'"$id"',"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"test","version":"1"}}}' ;;
    *'"tools/list"'*) echo '{"jsonrpc":"2.0","id":'"$id"',"result":{"tools":[{"name":"x","description":"Find a record","inputSchema":{"type":"object"}}]}}' ;;
    *'"tools/call"'*) printf '%s\n' "$line" >> "$TRACE"; echo '{"jsonrpc":"2.0","id":'"$id"',"result":{"content":[{"type":"text","text":"ok"},{"type":"image","data":"abc","mimeType":"image/png"}],"isError":false}}' ;;
    *'"shutdown"'*) exit 0 ;;
  esac
done
"##).unwrap();
    let mut cfg = config("/bin/sh");
    cfg.args = vec![script.to_string_lossy().into_owned()];
    cfg.env
        .insert("TRACE".into(), trace.to_string_lossy().into_owned());
    cfg.env
        .insert("PIDFILE".into(), pidfile.to_string_lossy().into_owned());
    let mut configs = McpConfig::default();
    configs.servers.insert("A".into(), cfg.clone());
    for server in ["B", "a", "a__b"] {
        configs.servers.insert(server.into(), cfg.clone());
    }
    cfg.enabled = Some(false);
    configs.servers.insert("off".into(), cfg);
    let manager = Arc::new(RwLock::new(McpManager::with_config(configs)));
    let registry = Registry::empty();
    register_fixed_mcp_surface(&registry, &manager).await;
    for (name, proxy) in create_mcp_tools_from_cached("A", &[definition("x")], manager.clone()) {
        registry.register(name, proxy).await;
    }
    for (server, tool) in [("a", "b__x"), ("a__b", "x")] {
        for (name, proxy) in
            create_mcp_tools_from_cached(server, &[definition(tool)], manager.clone())
        {
            registry.register(name, proxy).await;
        }
    }
    let session = "live-dispatch";
    set_session_tool_policy(
        session,
        Some(HashSet::from(["mcp__A__x".into()])),
        HashSet::new(),
    );
    let deferred = registry
        .execute(
            "mcp_call",
            json!({"server":"A", "tool":"x", "arguments":null}),
            context(session),
        )
        .await
        .unwrap();
    let eager = registry
        .execute("mcp__A__x", json!({"id":42}), context(session))
        .await
        .unwrap();
    assert_eq!(deferred.output, eager.output);
    assert_eq!(deferred.title, eager.title);
    let calls: Vec<Value> = std::fs::read_to_string(&trace)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["params"]["arguments"], json!({}));
    assert_eq!(calls[1]["params"]["arguments"], json!({"id":42}));
    registry
        .execute(
            "mcp_call",
            json!({"server":"B", "tool":"x"}),
            context(session),
        )
        .await
        .unwrap_err();
    for denied in ["mcp__A__x", "mcp_call"] {
        set_session_tool_policy(
            session,
            Some(HashSet::from(["mcp_call".into()])),
            HashSet::from([denied.into()]),
        );
        registry
            .execute(
                "mcp_call",
                json!({"server":"A", "tool":"x"}),
                context(session),
            )
            .await
            .unwrap_err();
    }
    set_session_tool_policy(session, None, HashSet::new());
    for (server, tool) in [("a", "b__x"), ("a__b", "x")] {
        registry
            .execute(
                "mcp_call",
                json!({"server":server, "tool":tool}),
                context(session),
            )
            .await
            .unwrap_err();
    }
    registry
        .execute(
            "mcp_call",
            json!({"server":"off", "tool":"x"}),
            context(session),
        )
        .await
        .unwrap_err();
    registry
        .execute(
            "mcp_call",
            json!({"server":"A", "tool":"x", "arguments":[]}),
            context(session),
        )
        .await
        .unwrap_err();
    assert_eq!(std::fs::read_to_string(&trace).unwrap().lines().count(), 2);
    let pid: u32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status()
            .unwrap()
            .success()
    );
    // Let the OS publish child exit, just as the existing manager reconnect test does.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let reconnected = registry
        .execute(
            "mcp_call",
            json!({"server":"A", "tool":"x"}),
            context(session),
        )
        .await
        .unwrap();
    assert_eq!(reconnected.output, eager.output);
    assert_eq!(std::fs::read_to_string(&trace).unwrap().lines().count(), 3);
    clear_session_tool_policy(session);
    manager.read().await.disconnect_all().await;
}
