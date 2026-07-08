//! `jcode mcp-serve`: re-publish the running daemon's tool registry over the
//! Model Context Protocol (MCP) as a stdio JSON-RPC server.
//!
//! This is an **additive seam** (new file + one clap arm + one dispatch arm): it
//! adds no new protocol and edits no upstream logic. It reuses:
//!   - the daemon's debug socket (`tool:<name> <json>` -> `execute_tool`, and
//!     `tools` -> the registry's `definitions()`), the same transport
//!     `src/cli/debug.rs` uses, and
//!   - the MCP wire shapes upstream already defines for the *client* side in
//!     `jcode_base::mcp::protocol` (`McpToolDef`, tools/list, tools/call).
//!
//! An MCP client (e.g. a Claude-Agent-SDK-hosted session, or any editor speaking
//! MCP) can therefore call every jcode tool - including the native `swarm` tool -
//! by adding one entry to `~/.jcode/mcp.json`:
//!
//! ```json
//! { "servers": { "jcode": { "command": "jcode", "args": ["mcp-serve"] } } }
//! ```
//!
//! Session-scoped tools (the swarm coordinator, session_search, ...) need a target
//! session. `--session <id>` pins one; otherwise the server auto-creates a
//! coordinator session in `--cwd` on first use via the `create_session` debug
//! command, so the swarm tool "just works" out of the box.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::server;

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

// Standard JSON-RPC error codes (mirrors src/cli/acp.rs).
const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INTERNAL_ERROR: i64 = -32603;

/// Entry point for `jcode mcp-serve`.
pub async fn run_mcp_serve_command(session: Option<String>, cwd: Option<String>) -> Result<()> {
    let mut server = McpServe {
        session,
        cwd,
        stdout: tokio::io::stdout(),
    };
    server.run().await
}

struct McpServe {
    /// Pinned target session for session-scoped tool calls. Lazily created if None.
    session: Option<String>,
    /// Working dir used when auto-creating a coordinator session.
    cwd: Option<String>,
    stdout: tokio::io::Stdout,
}

impl McpServe {
    async fn run(&mut self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
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

            if msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                self.write_error(id, JSONRPC_INVALID_REQUEST, "jsonrpc must be \"2.0\"")
                    .await?;
                continue;
            }

            let id = msg.get("id").cloned();
            let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
            let params = msg.get("params").cloned().unwrap_or(Value::Null);

            // Notifications (no id) get no response.
            let Some(id) = id else {
                continue;
            };

            match method {
                "initialize" => self.handle_initialize(id).await?,
                "tools/list" => self.handle_tools_list(id).await?,
                "tools/call" => self.handle_tools_call(id, params).await?,
                "ping" => self.write_result(id, json!({})).await?,
                other => {
                    self.write_error(
                        id,
                        JSONRPC_METHOD_NOT_FOUND,
                        &format!("Method not found: {other}"),
                    )
                    .await?;
                }
            }
        }
    }

    async fn handle_initialize(&mut self, id: Value) -> Result<()> {
        let result = json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "jcode", "version": jcode_build_meta::VERSION },
        });
        self.write_result(id, result).await
    }

    async fn handle_tools_list(&mut self, id: Value) -> Result<()> {
        // The daemon's `tools:full` debug command returns the registry's
        // ToolDefinition list (name, description, input_schema) as JSON. It needs a
        // session context, so ensure a coordinator session exists first.
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
        let tools: Vec<Value> = defs
            .as_array()
            .map(|arr| arr.iter().map(tool_def_to_mcp).collect())
            .unwrap_or_default();

        self.write_result(id, json!({ "tools": tools })).await
    }

    async fn handle_tools_call(&mut self, id: Value, params: Value) -> Result<()> {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() {
            return self
                .write_error(
                    id,
                    JSONRPC_INVALID_REQUEST,
                    "tools/call requires a tool name",
                )
                .await;
        }
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        // Session-scoped tools need a coordinator session; create one lazily.
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

    /// Return the pinned session, creating a coordinator session on first use.
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
}
