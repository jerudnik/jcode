//! `jcode mcp-serve`: expose a running daemon's session-filtered tool registry to
//! an external Model Context Protocol (MCP) client over stdio JSON-RPC.
//!
//! The bridge currently proxies tool discovery and calls through the daemon debug
//! socket. Configure it in the external MCP client that launches this process,
//! such as an ACP session request. Do not add it to Jcode's own
//! `~/.jcode/mcp.json`; that would make the daemon register itself as an MCP
//! dependency.
//!
//! `--session <id>` pins tool discovery and calls to an existing daemon session.
//! Without it, the bridge creates an independent, unlinked headless session in
//! `--cwd` on first use. That lazy session is not a coordinator and has no parent
//! relationship to the client that launched the bridge.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::{mcp::MCP_OWNER_PID_ENV, server};

const MCP_PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const MCP_PREFERRED_PROTOCOL_VERSION: &str = MCP_PROTOCOL_VERSIONS[0];

// Standard JSON-RPC error codes (mirrors src/cli/acp.rs).
const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;
const JSONRPC_INTERNAL_ERROR: i64 = -32603;
const OWNER_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Deserialize)]
struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
}

#[derive(Default, Deserialize)]
struct ToolsListParams {
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct ToolsCallParams {
    name: String,
    #[serde(default)]
    arguments: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerPollResult {
    Live,
    Dead,
    Unknown,
}

fn should_exit_for_owner(owner_pid: Option<u32>, result: OwnerPollResult) -> bool {
    owner_pid.is_some() && result == OwnerPollResult::Dead
}

fn owner_pid_from_env() -> Result<Option<u32>> {
    let Some(raw) = std::env::var_os(MCP_OWNER_PID_ENV) else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    let pid = raw
        .parse::<u32>()
        .with_context(|| format!("{MCP_OWNER_PID_ENV} must be a positive process ID"))?;
    if pid == 0 {
        anyhow::bail!("{MCP_OWNER_PID_ENV} must be a positive process ID");
    }
    Ok(Some(pid))
}

#[cfg(unix)]
fn poll_owner(pid: u32) -> OwnerPollResult {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if result == 0 {
        return OwnerPollResult::Live;
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => OwnerPollResult::Dead,
        Some(libc::EPERM) => OwnerPollResult::Live,
        _ => OwnerPollResult::Unknown,
    }
}

#[cfg(windows)]
fn poll_owner(pid: u32) -> OwnerPollResult {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const ERROR_INVALID_PARAMETER: u32 = 87;
    const STILL_ACTIVE: u32 = 259;
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return if GetLastError() == ERROR_INVALID_PARAMETER {
                OwnerPollResult::Dead
            } else {
                OwnerPollResult::Unknown
            };
        }
        let mut exit_code = 0;
        let queried = GetExitCodeProcess(handle, &mut exit_code) != 0;
        CloseHandle(handle);
        if !queried {
            OwnerPollResult::Unknown
        } else if exit_code == STILL_ACTIVE {
            OwnerPollResult::Live
        } else {
            OwnerPollResult::Dead
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn poll_owner(_pid: u32) -> OwnerPollResult {
    OwnerPollResult::Unknown
}

async fn wait_for_owner_exit(owner_pid: Option<u32>) {
    let Some(owner_pid) = owner_pid else {
        std::future::pending::<()>().await;
        return;
    };

    loop {
        if should_exit_for_owner(Some(owner_pid), poll_owner(owner_pid)) {
            return;
        }
        tokio::time::sleep(OWNER_POLL_INTERVAL).await;
    }
}

/// Entry point for `jcode mcp-serve`.
pub async fn run_mcp_serve_command(session: Option<String>, cwd: Option<String>) -> Result<()> {
    let mut server = McpServe {
        session,
        cwd,
        owner_pid: owner_pid_from_env()?,
        stdout: tokio::io::stdout(),
    };
    server.run().await
}

struct McpServe<W> {
    /// Pinned target session for session-scoped tool calls. Lazily created if None.
    session: Option<String>,
    /// Working dir used when auto-creating an unlinked headless session.
    cwd: Option<String>,
    /// Daemon/editor process that owns this stdio MCP server, when supplied.
    owner_pid: Option<u32>,
    stdout: W,
}

impl<W> McpServe<W>
where
    W: AsyncWrite + Unpin,
{
    async fn run(&mut self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        let owner_exit = wait_for_owner_exit(self.owner_pid);
        tokio::pin!(owner_exit);

        loop {
            line.clear();
            let n = tokio::select! {
                _ = &mut owner_exit => {
                    crate::logging::info(&format!(
                        "mcp-serve: owner PID {:?} exited; shutting down",
                        self.owner_pid
                    ));
                    return Ok(());
                }
                result = reader.read_line(&mut line) => result?,
            };
            if n == 0 {
                return Ok(()); // client closed stdin
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let msg: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(err) => {
                    self.write_error(
                        Value::Null,
                        JSONRPC_PARSE_ERROR,
                        &format!("Parse error: {err}"),
                    )
                    .await?;
                    continue;
                }
            };

            self.handle_message(msg).await?;
        }
    }

    async fn handle_message(&mut self, msg: Value) -> Result<()> {
        if msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            return self
                .write_error(id, JSONRPC_INVALID_REQUEST, "jsonrpc must be \"2.0\"")
                .await;
        }

        // Notifications, including notifications/initialized, get no response.
        let Some(id) = msg.get("id").cloned() else {
            return Ok(());
        };
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => self.handle_initialize(id, params).await,
            "tools/list" => self.handle_tools_list(id, params).await,
            "tools/call" => self.handle_tools_call(id, params).await,
            "ping" => self.write_result(id, json!({})).await,
            other => {
                self.write_error(
                    id,
                    JSONRPC_METHOD_NOT_FOUND,
                    &format!("Method not found: {other}"),
                )
                .await
            }
        }
    }

    async fn handle_initialize(&mut self, id: Value, params: Value) -> Result<()> {
        let protocol_version = match negotiate_protocol_version(params) {
            Ok(version) => version,
            Err(message) => {
                return self.write_error(id, JSONRPC_INVALID_PARAMS, &message).await;
            }
        };
        let result = json!({
            "protocolVersion": protocol_version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "jcode", "version": jcode_build_meta::VERSION },
        });
        self.write_result(id, result).await
    }

    async fn handle_tools_list(&mut self, id: Value, params: Value) -> Result<()> {
        if let Err(message) = validate_tools_list_params(params) {
            return self.write_error(id, JSONRPC_INVALID_PARAMS, &message).await;
        }

        // The daemon's `tools:full` debug command returns the registry's
        // ToolDefinition list (name, description, input_schema) as JSON. It needs a
        // session context, so ensure a target session exists first.
        let session = match self.ensure_session().await {
            Ok(s) => s,
            Err(err) => {
                return self
                    .write_error(id, JSONRPC_INTERNAL_ERROR, &format!("no session: {err}"))
                    .await;
            }
        };
        let raw = match self.debug_command("tools:full", Some(&session)).await {
            Ok(out) => out,
            Err(err) => {
                return self
                    .write_error(
                        id,
                        JSONRPC_INTERNAL_ERROR,
                        &format!("tools/list failed: {err}"),
                    )
                    .await;
            }
        };

        let defs: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        self.write_result(id, tools_list_result(&defs)).await
    }

    async fn handle_tools_call(&mut self, id: Value, params: Value) -> Result<()> {
        let (name, arguments) = match parse_tools_call_params(params) {
            Ok(parsed) => parsed,
            Err(message) => {
                return self.write_error(id, JSONRPC_INVALID_PARAMS, &message).await;
            }
        };

        // Tool discovery and calls need a session context; create one lazily.
        let session = match self.ensure_session().await {
            Ok(s) => s,
            Err(err) => {
                return self
                    .write_error(id, JSONRPC_INTERNAL_ERROR, &format!("no session: {err}"))
                    .await;
            }
        };

        let cmd = format!("tool:{name} {}", serde_json::to_string(&arguments)?);
        match self.debug_command(&cmd, Some(&session)).await {
            Ok(out) => {
                // The debug `tool:` path returns {output,title,metadata}; surface
                // `output` as a single MCP text block.
                let text = serde_json::from_str::<Value>(&out)
                    .ok()
                    .and_then(|v| v.get("output").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or(out);
                let result = json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false,
                });
                self.write_result(id, result).await
            }
            Err(err) => {
                // MCP convention: tool failures are a successful response with
                // isError=true, not a protocol error.
                let result = json!({
                    "content": [{ "type": "text", "text": format!("{err}") }],
                    "isError": true,
                });
                self.write_result(id, result).await
            }
        }
    }

    /// Return the pinned session, creating an unlinked headless session on first use.
    async fn ensure_session(&mut self) -> Result<String> {
        if let Some(s) = &self.session {
            return Ok(s.clone());
        }
        let create_cmd = match &self.cwd {
            Some(dir) => format!("create_session:{dir}"),
            None => "create_session".to_string(),
        };
        let raw = self.debug_command(&create_cmd, None).await?;
        let parsed: Value = serde_json::from_str(&raw)
            .with_context(|| format!("create_session returned non-JSON: {raw}"))?;
        let sid = parsed
            .get("session_id")
            .and_then(Value::as_str)
            .context("create_session response missing session_id")?
            .to_string();
        self.session = Some(sid.clone());
        Ok(sid)
    }

    /// Send one debug command to the daemon over the debug socket and return the
    /// `output` string. Mirrors `src/cli/debug.rs::run_debug_command`.
    async fn debug_command(&self, command: &str, session_id: Option<&str>) -> Result<String> {
        let debug_socket = server::debug_socket_path();
        if !crate::transport::is_socket_path(&debug_socket) {
            anyhow::bail!(
                "Debug socket not found at {debug_socket:?}. Start a jcode server and set \
                 [display] debug_socket = true in ~/.jcode/config.toml."
            );
        }

        let stream = server::connect_socket(&debug_socket).await?;
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        let request = json!({
            "type": "debug_command",
            "id": 1,
            "command": command,
            "session_id": session_id,
        });
        let mut payload = serde_json::to_string(&request)?;
        payload.push('\n');
        writer.write_all(payload.as_bytes()).await?;

        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            anyhow::bail!("daemon disconnected before responding");
        }
        let response: Value = serde_json::from_str(&line)?;
        match response.get("type").and_then(Value::as_str) {
            Some("debug_response") => {
                let ok = response.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let output = response
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if ok {
                    Ok(output)
                } else {
                    anyhow::bail!("{output}")
                }
            }
            Some("error") => {
                let message = response
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error");
                anyhow::bail!("{message}")
            }
            _ => Ok(line.trim().to_string()),
        }
    }

    async fn write_result(&mut self, id: Value, result: Value) -> Result<()> {
        self.write_message(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
            .await
    }

    async fn write_error(&mut self, id: Value, code: i64, message: &str) -> Result<()> {
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }))
        .await
    }

    async fn write_message(&mut self, msg: Value) -> Result<()> {
        let mut line = serde_json::to_string(&msg)?;
        line.push('\n');
        self.stdout.write_all(line.as_bytes()).await?;
        self.stdout.flush().await?;
        Ok(())
    }
}

fn negotiate_protocol_version(params: Value) -> std::result::Result<String, String> {
    let params: InitializeParams = serde_json::from_value(params)
        .map_err(|err| format!("Invalid initialize params: {err}"))?;
    if params.protocol_version.is_empty() {
        return Err("Invalid initialize params: protocolVersion must not be empty".to_string());
    }
    if MCP_PROTOCOL_VERSIONS.contains(&params.protocol_version.as_str()) {
        Ok(params.protocol_version)
    } else {
        Ok(MCP_PREFERRED_PROTOCOL_VERSION.to_string())
    }
}

fn validate_tools_list_params(params: Value) -> std::result::Result<(), String> {
    let params = if params.is_null() {
        ToolsListParams::default()
    } else {
        serde_json::from_value(params).map_err(|err| format!("Invalid tools/list params: {err}"))?
    };
    if params
        .cursor
        .as_deref()
        .is_some_and(|cursor| !cursor.is_empty())
    {
        return Err("Invalid tools/list params: unknown cursor".to_string());
    }
    Ok(())
}

fn parse_tools_call_params(params: Value) -> std::result::Result<(String, Value), String> {
    let params: ToolsCallParams = serde_json::from_value(params)
        .map_err(|err| format!("Invalid tools/call params: {err}"))?;
    if params.name.is_empty() {
        return Err("Invalid tools/call params: name must not be empty".to_string());
    }
    Ok((
        params.name,
        Value::Object(params.arguments.unwrap_or_default()),
    ))
}

fn sorted_mcp_tools(defs: &Value) -> Vec<Value> {
    let mut tools: Vec<Value> = defs
        .as_array()
        .map(|arr| arr.iter().map(tool_def_to_mcp).collect())
        .unwrap_or_default();
    tools.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .cmp(
                right
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
    });
    tools
}

fn tools_list_result(defs: &Value) -> Value {
    json!({ "tools": sorted_mcp_tools(defs) })
}

/// Map a jcode `ToolDefinition` JSON object to an MCP `McpToolDef` JSON object.
fn tool_def_to_mcp(def: &Value) -> Value {
    let name = def.get("name").and_then(Value::as_str).unwrap_or_default();
    let description = def
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let input_schema = def
        .get("input_schema")
        .or_else(|| def.get("parameters"))
        .cloned()
        .unwrap_or_else(|| json!({ "type": "object" }));
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn response_for_message(message: Value) -> Value {
        let (stdout, reader) = tokio::io::duplex(4096);
        let mut server = McpServe {
            session: None,
            cwd: None,
            owner_pid: None,
            stdout,
        };
        server.handle_message(message).await.unwrap();

        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).unwrap()
    }

    async fn initialize_response(protocol_version: &str) -> Value {
        response_for_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": protocol_version,
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" }
            }
        }))
        .await
    }

    #[tokio::test]
    async fn initialize_negotiates_2025_11_25() {
        let response = initialize_response("2025-11-25").await;
        assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(
            response["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
    }

    #[tokio::test]
    async fn initialize_negotiates_2025_06_18() {
        let response = initialize_response("2025-06-18").await;
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
    }

    #[tokio::test]
    async fn initialize_negotiates_2025_03_26() {
        let response = initialize_response("2025-03-26").await;
        assert_eq!(response["result"]["protocolVersion"], "2025-03-26");
    }

    #[tokio::test]
    async fn initialize_negotiates_2024_11_05() {
        let response = initialize_response("2024-11-05").await;
        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
    }

    #[tokio::test]
    async fn initialize_unknown_revision_selects_preferred_legacy_revision() {
        let response = initialize_response("2099-01-01").await;
        assert_eq!(
            response["result"]["protocolVersion"],
            MCP_PREFERRED_PROTOCOL_VERSION
        );
    }

    #[tokio::test]
    async fn initialized_notification_is_accepted_without_a_response() {
        let mut server = McpServe {
            session: None,
            cwd: None,
            owner_pid: None,
            stdout: tokio::io::sink(),
        };
        server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {}
            }))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn malformed_tools_call_params_return_invalid_params() {
        for params in [json!({}), json!({ "name": "bash", "arguments": [] })] {
            let response = response_for_message(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": params
            }))
            .await;
            assert_eq!(response["error"]["code"], JSONRPC_INVALID_PARAMS);
        }
    }

    #[tokio::test]
    async fn unknown_tools_list_cursor_returns_invalid_params() {
        let response = response_for_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": { "cursor": "next-page" }
        }))
        .await;
        assert_eq!(response["error"]["code"], JSONRPC_INVALID_PARAMS);
    }

    #[test]
    fn tools_list_accepts_no_or_empty_cursor() {
        assert!(validate_tools_list_params(Value::Null).is_ok());
        assert!(validate_tools_list_params(json!({})).is_ok());
        assert!(validate_tools_list_params(json!({ "cursor": "" })).is_ok());
    }

    #[test]
    fn tools_list_is_sorted_and_has_no_next_cursor() {
        let result = tools_list_result(&json!([
            { "name": "zeta", "input_schema": { "type": "object" } },
            { "name": "alpha", "input_schema": { "type": "object" } },
            { "name": "middle", "input_schema": { "type": "object" } }
        ]));
        let names: Vec<_> = result["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["alpha", "middle", "zeta"]);
        assert!(result.get("nextCursor").is_none());
    }

    #[test]
    fn tool_def_maps_to_mcp_shape() {
        let def = json!({
            "name": "swarm",
            "description": "coordinate agents",
            "input_schema": { "type": "object", "required": ["action"] }
        });
        let mcp = tool_def_to_mcp(&def);
        assert_eq!(mcp["name"], "swarm");
        assert_eq!(mcp["description"], "coordinate agents");
        assert_eq!(mcp["inputSchema"]["required"][0], "action");
    }

    #[test]
    fn tool_def_defaults_missing_schema_to_object() {
        let def = json!({ "name": "x", "description": "y" });
        let mcp = tool_def_to_mcp(&def);
        assert_eq!(mcp["inputSchema"]["type"], "object");
    }

    #[test]
    fn tool_def_accepts_parameters_alias() {
        let def = json!({ "name": "x", "parameters": { "type": "object", "k": 1 } });
        let mcp = tool_def_to_mcp(&def);
        assert_eq!(mcp["inputSchema"]["k"], 1);
    }

    #[test]
    fn owner_liveness_decision_is_fail_safe_and_only_exits_on_dead() {
        assert!(!should_exit_for_owner(None, OwnerPollResult::Dead));
        assert!(!should_exit_for_owner(Some(42), OwnerPollResult::Live));
        assert!(!should_exit_for_owner(Some(42), OwnerPollResult::Unknown));
        assert!(should_exit_for_owner(Some(42), OwnerPollResult::Dead));
    }
}
