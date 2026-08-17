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
//! Modern (`2026-07-28`) requests require that explicit pin and never create hidden
//! session state. For legacy clients only, omitting `--session` preserves the old
//! compatibility behavior: the bridge lazily creates an independent, unlinked
//! headless session in `--cwd`. That session is not a coordinator and has no parent
//! relationship to the client that launched the bridge.

use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{Mutex, Semaphore, mpsc},
    task::JoinHandle,
};

use crate::{mcp::MCP_OWNER_PID_ENV, server};

const MCP_MODERN_PROTOCOL_VERSION: &str = "2026-07-28";
const MCP_LEGACY_PROTOCOL_VERSIONS: &[&str] =
    &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const MCP_SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    MCP_MODERN_PROTOCOL_VERSION,
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
];
const MCP_PREFERRED_PROTOCOL_VERSION: &str = MCP_LEGACY_PROTOCOL_VERSIONS[0];
const MCP_PROTOCOL_VERSION_META_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const MCP_CLIENT_INFO_META_KEY: &str = "io.modelcontextprotocol/clientInfo";
const MCP_CLIENT_CAPABILITIES_META_KEY: &str = "io.modelcontextprotocol/clientCapabilities";
const MCP_SERVER_INFO_META_KEY: &str = "io.modelcontextprotocol/serverInfo";
const MCP_DISCOVER_TTL_MS: u64 = 60_000;
const MCP_TOOLS_LIST_TTL_MS: u64 = 5_000;
const MCP_MAX_IN_FLIGHT_REQUESTS: usize = 8;

// Standard JSON-RPC error codes (mirrors src/cli/acp.rs).
const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;
const JSONRPC_INTERNAL_ERROR: i64 = -32603;
const MCP_UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;
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

#[derive(Deserialize)]
struct ClientInfo {
    name: String,
    version: String,
}

struct ModernRequestMeta {
    // Kept request-local for logging and attribution without introducing
    // connection-scoped protocol state.
    client_info: Option<ClientInfo>,
    _client_capabilities: serde_json::Map<String, Value>,
}

enum ProtocolEra {
    Legacy,
    Modern(ModernRequestMeta),
}

impl ProtocolEra {
    fn is_modern(&self) -> bool {
        matches!(self, Self::Modern(_))
    }

    fn client_info(&self) -> Option<&ClientInfo> {
        match self {
            Self::Legacy => None,
            Self::Modern(metadata) => metadata.client_info.as_ref(),
        }
    }
}

struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    fn with_data(code: i64, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

#[async_trait]
trait McpBackend: Send + Sync + 'static {
    async fn command(&self, command: &str, session_id: Option<&str>) -> Result<String>;
}

struct DebugBackend;

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
    McpServe::new(session, cwd, owner_pid_from_env()?, DebugBackend)
        .run(tokio::io::stdin(), tokio::io::stdout())
        .await
}

struct SessionBinding {
    /// Explicit session pin. Modern requests require this immutable configuration.
    pinned: Option<String>,
    /// Legacy-only lazy session retained for compatibility with existing clients.
    legacy_lazy: Mutex<Option<String>>,
    /// Working dir used when auto-creating an unlinked legacy headless session.
    cwd: Option<String>,
}

struct McpServe<B> {
    sessions: Arc<SessionBinding>,
    /// Daemon/editor process that owns this stdio MCP server, when supplied.
    owner_pid: Option<u32>,
    backend: Arc<B>,
}

impl<B> Clone for McpServe<B>
where
    B: McpBackend,
{
    fn clone(&self) -> Self {
        Self {
            sessions: Arc::clone(&self.sessions),
            owner_pid: self.owner_pid,
            backend: Arc::clone(&self.backend),
        }
    }
}

struct RequestCompletion {
    key: String,
    response: Value,
}

impl<B> McpServe<B>
where
    B: McpBackend,
{
    fn new(
        session: Option<String>,
        cwd: Option<String>,
        owner_pid: Option<u32>,
        backend: B,
    ) -> Self {
        Self {
            sessions: Arc::new(SessionBinding {
                pinned: session,
                legacy_lazy: Mutex::new(None),
                cwd,
            }),
            owner_pid,
            backend: Arc::new(backend),
        }
    }

    async fn run<R, W>(&self, input: R, mut output: W) -> Result<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut lines = BufReader::new(input).lines();
        let owner_exit = wait_for_owner_exit(self.owner_pid);
        tokio::pin!(owner_exit);
        let parallelism = Arc::new(Semaphore::new(MCP_MAX_IN_FLIGHT_REQUESTS));
        let (completion_tx, mut completion_rx) = mpsc::unbounded_channel::<RequestCompletion>();
        let mut in_flight: HashMap<String, JoinHandle<()>> = HashMap::new();

        loop {
            let n = tokio::select! {
                _ = &mut owner_exit => {
                    crate::logging::info(&format!(
                        "mcp-serve: owner PID {:?} exited; shutting down",
                        self.owner_pid
                    ));
                    abort_all(&mut in_flight);
                    return Ok(());
                }
                line = lines.next_line() => {
                    let Some(line) = line? else {
                        abort_all(&mut in_flight);
                        return Ok(());
                    };
                    line
                }
                Some(completion) = completion_rx.recv() => {
                    if in_flight.remove(&completion.key).is_some() {
                        write_message(&mut output, &completion.response).await?;
                    }
                    continue;
                }
            };
            let trimmed = n.trim();
            if trimmed.is_empty() {
                continue;
            }

            let msg: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(err) => {
                    write_message(
                        &mut output,
                        &rpc_error_response(
                            Value::Null,
                            RpcError::new(JSONRPC_PARSE_ERROR, format!("Parse error: {err}")),
                        ),
                    )
                    .await?;
                    continue;
                }
            };

            if msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                write_message(
                    &mut output,
                    &rpc_error_response(
                        id,
                        RpcError::new(JSONRPC_INVALID_REQUEST, "jsonrpc must be \"2.0\""),
                    ),
                )
                .await?;
                continue;
            }

            let Some(id) = msg.get("id").cloned() else {
                self.handle_notification(&msg, &mut in_flight);
                continue;
            };
            let key = request_key(&id);
            if let Some(previous) = in_flight.remove(&key) {
                previous.abort();
            }

            let server = self.clone();
            let task_key = key.clone();
            let task_completion_tx = completion_tx.clone();
            let task_parallelism = Arc::clone(&parallelism);
            let task = tokio::spawn(async move {
                let Ok(_permit) = task_parallelism.acquire_owned().await else {
                    return;
                };
                let response = server.response_for_request(msg).await;
                let _ = task_completion_tx.send(RequestCompletion {
                    key: task_key,
                    response,
                });
            });
            in_flight.insert(key, task);
        }
    }

    fn handle_notification(&self, msg: &Value, in_flight: &mut HashMap<String, JoinHandle<()>>) {
        if msg.get("method").and_then(Value::as_str) != Some("notifications/cancelled") {
            return;
        }
        let Some(request_id) = msg.pointer("/params/requestId") else {
            return;
        };
        if let Some(task) = in_flight.remove(&request_key(request_id)) {
            task.abort();
        }
    }

    async fn response_for_request(&self, msg: Value) -> Value {
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        if msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return rpc_error_response(
                id,
                RpcError::new(JSONRPC_INVALID_REQUEST, "jsonrpc must be \"2.0\""),
            );
        }

        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let era = match protocol_era(method, &params) {
            Ok(era) => era,
            Err(error) => return rpc_error_response(id, error),
        };
        if let Some(client) = era.client_info() {
            crate::logging::info(&format!(
                "mcp-serve: modern {method} request from {} {}",
                client.name, client.version
            ));
        }
        let modern = era.is_modern();

        let result = match method {
            "initialize" if !modern => self.handle_initialize(params),
            "server/discover" if modern => Ok(discover_result()),
            "tools/list" => self.handle_tools_list(params, modern).await,
            "tools/call" => self.handle_tools_call(params, modern).await,
            "ping" if !modern => Ok(json!({})),
            other => Err(RpcError::new(
                JSONRPC_METHOD_NOT_FOUND,
                format!("Method not found: {other}"),
            )),
        };

        match result {
            Ok(result) if modern => rpc_result_response(id, modern_complete_result(result)),
            Ok(result) => rpc_result_response(id, result),
            Err(error) => rpc_error_response(id, error),
        }
    }

    fn handle_initialize(&self, params: Value) -> std::result::Result<Value, RpcError> {
        let protocol_version = match negotiate_protocol_version(params) {
            Ok(version) => version,
            Err(message) => return Err(RpcError::new(JSONRPC_INVALID_PARAMS, message)),
        };
        Ok(json!({
            "protocolVersion": protocol_version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "jcode", "version": jcode_build_meta::VERSION },
        }))
    }

    async fn handle_tools_list(
        &self,
        params: Value,
        modern: bool,
    ) -> std::result::Result<Value, RpcError> {
        if let Err(message) = validate_tools_list_params(params) {
            return Err(RpcError::new(JSONRPC_INVALID_PARAMS, message));
        }

        // The daemon's `tools:full` debug command returns the registry's
        // ToolDefinition list (name, description, input_schema) as JSON. It needs a
        // session context, so ensure a target session exists first.
        let session = self
            .session_for_request(modern)
            .await
            .map_err(|err| RpcError::new(JSONRPC_INTERNAL_ERROR, format!("no session: {err}")))?;
        let raw = self
            .backend
            .command("tools:full", Some(&session))
            .await
            .map_err(|err| {
                RpcError::new(JSONRPC_INTERNAL_ERROR, format!("tools/list failed: {err}"))
            })?;

        let defs: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        Ok(tools_list_result(&defs, modern))
    }

    async fn handle_tools_call(
        &self,
        params: Value,
        modern: bool,
    ) -> std::result::Result<Value, RpcError> {
        let (name, arguments) = match parse_tools_call_params(params) {
            Ok(parsed) => parsed,
            Err(message) => return Err(RpcError::new(JSONRPC_INVALID_PARAMS, message)),
        };

        let session = self
            .session_for_request(modern)
            .await
            .map_err(|err| RpcError::new(JSONRPC_INTERNAL_ERROR, format!("no session: {err}")))?;

        let cmd = format!(
            "tool:{name} {}",
            serde_json::to_string(&arguments)
                .map_err(|err| RpcError::new(JSONRPC_INTERNAL_ERROR, err.to_string()))?
        );
        match self.backend.command(&cmd, Some(&session)).await {
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
                Ok(result)
            }
            Err(err) => {
                // MCP convention: tool failures are a successful response with
                // isError=true, not a protocol error.
                let result = json!({
                    "content": [{ "type": "text", "text": format!("{err}") }],
                    "isError": true,
                });
                Ok(result)
            }
        }
    }

    async fn session_for_request(&self, modern: bool) -> Result<String> {
        if let Some(session) = &self.sessions.pinned {
            return Ok(session.clone());
        }
        if modern {
            anyhow::bail!(
                "modern tools require --session <id>; mcp-serve does not lazily create modern sessions"
            );
        }

        let mut legacy_lazy = self.sessions.legacy_lazy.lock().await;
        if let Some(session) = legacy_lazy.as_ref() {
            return Ok(session.clone());
        }
        let create_cmd = match &self.sessions.cwd {
            Some(dir) => format!("create_session:{dir}"),
            None => "create_session".to_string(),
        };
        let raw = self.backend.command(&create_cmd, None).await?;
        let parsed: Value = serde_json::from_str(&raw)
            .with_context(|| format!("create_session returned non-JSON: {raw}"))?;
        let sid = parsed
            .get("session_id")
            .and_then(Value::as_str)
            .context("create_session response missing session_id")?
            .to_string();
        *legacy_lazy = Some(sid.clone());
        Ok(sid)
    }
}

#[async_trait]
impl McpBackend for DebugBackend {
    /// Send one debug command to the daemon over the debug socket and return the
    /// `output` string. Mirrors `src/cli/debug.rs::run_debug_command`.
    async fn command(&self, command: &str, session_id: Option<&str>) -> Result<String> {
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
}

fn abort_all(in_flight: &mut HashMap<String, JoinHandle<()>>) {
    for (_, task) in in_flight.drain() {
        task.abort();
    }
}

fn request_key(id: &Value) -> String {
    serde_json::to_string(id).unwrap_or_else(|_| "null".to_string())
}

async fn write_message<W>(output: &mut W, msg: &Value) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    output.write_all(line.as_bytes()).await?;
    output.flush().await?;
    Ok(())
}

fn rpc_result_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error_response(id: Value, error: RpcError) -> Value {
    let mut body = json!({
        "code": error.code,
        "message": error.message,
    });
    if let Some(data) = error.data {
        body["data"] = data;
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": body })
}

fn protocol_era(method: &str, params: &Value) -> std::result::Result<ProtocolEra, RpcError> {
    let has_modern_metadata = params
        .get("_meta")
        .and_then(Value::as_object)
        .is_some_and(|meta| {
            [
                MCP_PROTOCOL_VERSION_META_KEY,
                MCP_CLIENT_INFO_META_KEY,
                MCP_CLIENT_CAPABILITIES_META_KEY,
            ]
            .iter()
            .any(|key| meta.contains_key(*key))
        });
    if method == "server/discover" || has_modern_metadata {
        return validate_modern_request_meta(params).map(ProtocolEra::Modern);
    }
    Ok(ProtocolEra::Legacy)
}

fn validate_modern_request_meta(
    params: &Value,
) -> std::result::Result<ModernRequestMeta, RpcError> {
    let meta = params.get("_meta").and_then(Value::as_object);
    let Some(meta) = meta else {
        return Err(RpcError::with_data(
            MCP_UNSUPPORTED_PROTOCOL_VERSION,
            "Unsupported protocol version",
            json!({
                "supported": MCP_SUPPORTED_PROTOCOL_VERSIONS,
                "requested": Value::Null,
            }),
        ));
    };
    let requested = meta
        .get(MCP_PROTOCOL_VERSION_META_KEY)
        .cloned()
        .unwrap_or(Value::Null);
    if requested.as_str() != Some(MCP_MODERN_PROTOCOL_VERSION) {
        return Err(RpcError::with_data(
            MCP_UNSUPPORTED_PROTOCOL_VERSION,
            "Unsupported protocol version",
            json!({
                "supported": MCP_SUPPORTED_PROTOCOL_VERSIONS,
                "requested": requested,
            }),
        ));
    }

    let client_capabilities = meta
        .get(MCP_CLIENT_CAPABILITIES_META_KEY)
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            RpcError::new(
                JSONRPC_INVALID_PARAMS,
                format!(
                    "Invalid request params: _meta.{MCP_CLIENT_CAPABILITIES_META_KEY} must be an object"
                ),
            )
        })?;
    let client_info = match meta.get(MCP_CLIENT_INFO_META_KEY) {
        Some(value) => {
            let info: ClientInfo = serde_json::from_value(value.clone()).map_err(|err| {
                RpcError::new(
                    JSONRPC_INVALID_PARAMS,
                    format!("Invalid request params: clientInfo: {err}"),
                )
            })?;
            if info.name.is_empty() || info.version.is_empty() {
                return Err(RpcError::new(
                    JSONRPC_INVALID_PARAMS,
                    "Invalid request params: clientInfo name and version must not be empty",
                ));
            }
            Some(info)
        }
        None => None,
    };

    Ok(ModernRequestMeta {
        client_info,
        _client_capabilities: client_capabilities,
    })
}

fn server_info_meta() -> Value {
    json!({
        (MCP_SERVER_INFO_META_KEY): {
            "name": "jcode",
            "version": jcode_build_meta::VERSION,
        }
    })
}

fn modern_complete_result(mut result: Value) -> Value {
    if !result.is_object() {
        return json!({
            "resultType": "complete",
            "value": result,
            "_meta": server_info_meta(),
        });
    }
    if let Some(result_object) = result.as_object_mut() {
        result_object.insert(
            "resultType".to_string(),
            Value::String("complete".to_string()),
        );
        result_object.insert("_meta".to_string(), server_info_meta());
    }
    result
}

fn discover_result() -> Value {
    json!({
        "supportedVersions": MCP_SUPPORTED_PROTOCOL_VERSIONS,
        "capabilities": { "tools": { "listChanged": false } },
        "ttlMs": MCP_DISCOVER_TTL_MS,
        "cacheScope": "private",
    })
}

fn negotiate_protocol_version(params: Value) -> std::result::Result<String, String> {
    let params: InitializeParams = serde_json::from_value(params)
        .map_err(|err| format!("Invalid initialize params: {err}"))?;
    if params.protocol_version.is_empty() {
        return Err("Invalid initialize params: protocolVersion must not be empty".to_string());
    }
    if MCP_LEGACY_PROTOCOL_VERSIONS.contains(&params.protocol_version.as_str()) {
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

fn tools_list_result(defs: &Value, modern: bool) -> Value {
    if modern {
        json!({
            "tools": sorted_mcp_tools(defs),
            "ttlMs": MCP_TOOLS_LIST_TTL_MS,
            "cacheScope": "private",
        })
    } else {
        json!({ "tools": sorted_mcp_tools(defs) })
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
    use std::sync::{
        Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;
    use tokio::{
        io::AsyncWriteExt,
        sync::Notify,
        time::{Duration, timeout},
    };

    #[derive(Default)]
    struct TestBackend {
        commands: StdMutex<Vec<(String, Option<String>)>>,
        slow_started: Arc<Notify>,
        slow_cancelled: Arc<AtomicBool>,
    }

    struct CancellationFlag(Arc<AtomicBool>);

    impl Drop for CancellationFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl McpBackend for TestBackend {
        async fn command(&self, command: &str, session_id: Option<&str>) -> Result<String> {
            self.commands
                .lock()
                .unwrap()
                .push((command.to_string(), session_id.map(ToString::to_string)));
            match command {
                "create_session" => Ok(json!({ "session_id": "lazy-session" }).to_string()),
                command if command.starts_with("create_session:") => {
                    Ok(json!({ "session_id": "lazy-session" }).to_string())
                }
                "tools:full" => Ok(json!([
                    { "name": "zeta", "input_schema": { "type": "object" } },
                    { "name": "alpha", "input_schema": { "type": "object" } }
                ])
                .to_string()),
                command if command.starts_with("tool:slow ") => {
                    let _cancelled = CancellationFlag(Arc::clone(&self.slow_cancelled));
                    self.slow_started.notify_one();
                    std::future::pending::<Result<String>>().await
                }
                command if command.starts_with("tool:") => Ok(json!({
                    "output": format!("called {command}")
                })
                .to_string()),
                other => anyhow::bail!("unexpected test command: {other}"),
            }
        }
    }

    fn server(session: Option<&str>) -> McpServe<TestBackend> {
        McpServe::new(
            session.map(ToString::to_string),
            None,
            None,
            TestBackend::default(),
        )
    }

    fn modern_meta(version: &str) -> Value {
        json!({
            (MCP_PROTOCOL_VERSION_META_KEY): version,
            (MCP_CLIENT_INFO_META_KEY): { "name": "test", "version": "1" },
            (MCP_CLIENT_CAPABILITIES_META_KEY): {},
        })
    }

    fn modern_request(id: Value, method: &str, mut params: Value) -> Value {
        params
            .as_object_mut()
            .expect("modern test params must be an object")
            .insert(
                "_meta".to_string(),
                modern_meta(MCP_MODERN_PROTOCOL_VERSION),
            );
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })
    }

    async fn response_for_message(message: Value) -> Value {
        server(None).response_for_request(message).await
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
        let server = server(None);
        let mut in_flight = HashMap::new();
        server.handle_notification(
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {}
            }),
            &mut in_flight,
        );
        assert!(in_flight.is_empty());
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
        let result = tools_list_result(
            &json!([
                { "name": "zeta", "input_schema": { "type": "object" } },
                { "name": "alpha", "input_schema": { "type": "object" } },
                { "name": "middle", "input_schema": { "type": "object" } }
            ]),
            false,
        );
        let names: Vec<_> = result["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["alpha", "middle", "zeta"]);
        assert!(result.get("nextCursor").is_none());
    }

    #[tokio::test]
    async fn modern_server_discover_reports_supported_versions_and_tools_only() {
        let response = server(None)
            .response_for_request(modern_request(
                json!("discover-1"),
                "server/discover",
                json!({}),
            ))
            .await;

        assert_eq!(response["result"]["resultType"], "complete");
        assert_eq!(
            response["result"]["supportedVersions"][0],
            MCP_MODERN_PROTOCOL_VERSION
        );
        assert_eq!(
            response["result"]["capabilities"],
            json!({ "tools": { "listChanged": false } })
        );
        assert_eq!(response["result"]["ttlMs"], MCP_DISCOVER_TTL_MS);
        assert_eq!(response["result"]["cacheScope"], "private");
        assert_eq!(
            response["result"]["_meta"][MCP_SERVER_INFO_META_KEY]["name"],
            "jcode"
        );
    }

    #[tokio::test]
    async fn modern_missing_or_unsupported_version_returns_minus_32022() {
        for (params, requested) in [
            (json!({}), Value::Null),
            (
                json!({
                    "_meta": modern_meta("1900-01-01")
                }),
                json!("1900-01-01"),
            ),
        ] {
            let response = server(None)
                .response_for_request(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "server/discover",
                    "params": params,
                }))
                .await;
            assert_eq!(response["error"]["code"], MCP_UNSUPPORTED_PROTOCOL_VERSION);
            assert_eq!(response["error"]["message"], "Unsupported protocol version");
            assert_eq!(response["error"]["data"]["requested"], requested);
            assert_eq!(
                response["error"]["data"]["supported"][0],
                MCP_MODERN_PROTOCOL_VERSION
            );
        }
    }

    #[tokio::test]
    async fn modern_request_requires_client_capabilities() {
        let response = server(None)
            .response_for_request(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "server/discover",
                "params": {
                    "_meta": {
                        (MCP_PROTOCOL_VERSION_META_KEY): MCP_MODERN_PROTOCOL_VERSION,
                    }
                },
            }))
            .await;
        assert_eq!(response["error"]["code"], JSONRPC_INVALID_PARAMS);
    }

    #[tokio::test]
    async fn modern_shaped_tools_request_missing_version_does_not_create_legacy_session() {
        let server = server(None);
        let response = server
            .response_for_request(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list",
                "params": {
                    "_meta": {
                        (MCP_CLIENT_CAPABILITIES_META_KEY): {},
                    }
                },
            }))
            .await;
        assert_eq!(response["error"]["code"], MCP_UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(response["error"]["data"]["requested"], Value::Null);
        assert!(server.backend.commands.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn legacy_progress_metadata_does_not_select_modern_era() {
        let response = response_for_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "_meta": { "progressToken": "legacy-progress" },
                "name": "bash",
                "arguments": []
            }
        }))
        .await;
        assert_eq!(response["error"]["code"], JSONRPC_INVALID_PARAMS);
    }

    #[tokio::test]
    async fn modern_tools_list_has_complete_private_cache_result() {
        let response = server(Some("pinned"))
            .response_for_request(modern_request(json!(1), "tools/list", json!({})))
            .await;
        assert_eq!(response["result"]["resultType"], "complete");
        assert_eq!(response["result"]["cacheScope"], "private");
        assert_eq!(response["result"]["ttlMs"], MCP_TOOLS_LIST_TTL_MS);
        assert_eq!(response["result"]["tools"][0]["name"], "alpha");
        assert_eq!(response["result"]["tools"][1]["name"], "zeta");
        assert!(response["result"].get("nextCursor").is_none());
        assert_eq!(
            response["result"]["_meta"][MCP_SERVER_INFO_META_KEY]["name"],
            "jcode"
        );
    }

    #[tokio::test]
    async fn modern_tools_call_has_complete_result_and_server_info() {
        let response = server(Some("pinned"))
            .response_for_request(modern_request(
                json!(2),
                "tools/call",
                json!({ "name": "echo", "arguments": { "text": "hi" } }),
            ))
            .await;
        assert_eq!(response["result"]["resultType"], "complete");
        assert_eq!(response["result"]["isError"], false);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("tool:echo")
        );
        assert_eq!(
            response["result"]["_meta"][MCP_SERVER_INFO_META_KEY]["name"],
            "jcode"
        );
    }

    #[tokio::test]
    async fn modern_tools_require_an_explicit_session_pin() {
        let server = server(None);
        let response = server
            .response_for_request(modern_request(json!(1), "tools/list", json!({})))
            .await;
        assert_eq!(response["error"]["code"], JSONRPC_INTERNAL_ERROR);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("modern tools require --session")
        );
        assert!(server.backend.commands.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn ping_is_legacy_only() {
        let modern = server(None)
            .response_for_request(modern_request(json!(1), "ping", json!({})))
            .await;
        assert_eq!(modern["error"]["code"], JSONRPC_METHOD_NOT_FOUND);

        let legacy = response_for_message(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "ping",
            "params": {}
        }))
        .await;
        assert_eq!(legacy["result"], json!({}));
    }

    #[tokio::test]
    async fn cancelled_long_tool_call_emits_no_response() {
        let backend = TestBackend::default();
        let slow_started = Arc::clone(&backend.slow_started);
        let slow_cancelled = Arc::clone(&backend.slow_cancelled);
        let server = McpServe::new(Some("pinned".to_string()), None, None, backend);
        let (mut client_input, server_input) = tokio::io::duplex(4096);
        let (server_output, client_output) = tokio::io::duplex(4096);
        let run = tokio::spawn(async move { server.run(server_input, server_output).await });

        let request = modern_request(
            json!(7),
            "tools/call",
            json!({ "name": "slow", "arguments": {} }),
        );
        client_input
            .write_all(format!("{}\n", serde_json::to_string(&request).unwrap()).as_bytes())
            .await
            .unwrap();
        timeout(Duration::from_secs(1), slow_started.notified())
            .await
            .expect("slow tool should start");

        client_input
            .write_all(
                b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{\"requestId\":7,\"reason\":\"test\"}}\n",
            )
            .await
            .unwrap();
        timeout(Duration::from_secs(1), async {
            while !slow_cancelled.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancellation should abort the request task");

        let discover = modern_request(json!(8), "server/discover", json!({}));
        client_input
            .write_all(format!("{}\n", serde_json::to_string(&discover).unwrap()).as_bytes())
            .await
            .unwrap();
        let mut output_lines = BufReader::new(client_output).lines();
        let line = timeout(Duration::from_secs(1), output_lines.next_line())
            .await
            .expect("discover response should arrive")
            .unwrap()
            .unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], 8);

        drop(client_input);
        run.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn modern_probe_does_not_disable_legacy_initialize_fallback_shape() {
        let server = server(None);
        let discover = server
            .response_for_request(modern_request(json!("probe"), "server/discover", json!({})))
            .await;
        assert_eq!(discover["result"]["resultType"], "complete");

        let initialize = server
            .response_for_request(json!({
                "jsonrpc": "2.0",
                "id": "legacy",
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "fallback", "version": "1" }
                }
            }))
            .await;
        assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");
        assert!(initialize["result"].get("resultType").is_none());
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
