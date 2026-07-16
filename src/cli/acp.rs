use super::dispatch;
use super::provider_init::ProviderChoice;
use crate::protocol::{Request, ServerEvent};
use crate::transport::{ReadHalf, WriteHalf};
use anyhow::{Context, Result};
use jcode_provider_core::{ALL_CLAUDE_MODELS, ALL_OPENAI_MODELS};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

const ACP_PROTOCOL_VERSION: u64 = 1;

const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;
const JSONRPC_INTERNAL_ERROR: i64 = -32603;
const JSONRPC_SERVER_ERROR: i64 = -32000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AcpProfile {
    Standard,
    Extended,
    Full,
}

impl AcpProfile {
    fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "extended" => Self::Extended,
            "full" => Self::Full,
            _ => Self::Standard,
        }
    }

    fn is_extended(self) -> bool {
        matches!(self, Self::Extended | Self::Full)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Extended => "extended",
            Self::Full => "full",
        }
    }
}

#[derive(Debug)]
struct JsonRpcMessage {
    id: Option<Value>,
    method: Option<String>,
    params: Value,
}

impl JsonRpcMessage {
    fn parse(line: &str) -> std::result::Result<Self, (i64, String)> {
        let value: Value =
            serde_json::from_str(line).map_err(|err| (JSONRPC_PARSE_ERROR, err.to_string()))?;
        let object = value.as_object().ok_or_else(|| {
            (
                JSONRPC_INVALID_REQUEST,
                "JSON-RPC message must be an object".to_string(),
            )
        })?;
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err((
                JSONRPC_INVALID_REQUEST,
                "JSON-RPC message must include jsonrpc=\"2.0\"".to_string(),
            ));
        }
        Ok(Self {
            id: object.get("id").cloned(),
            method: object
                .get("method")
                .and_then(Value::as_str)
                .map(str::to_string),
            params: object.get("params").cloned().unwrap_or(Value::Null),
        })
    }
}

struct DaemonSession {
    session_id: String,
    reader: Mutex<BufReader<ReadHalf>>,
    writer: Mutex<WriteHalf>,
    next_request_id: AtomicU64,
    active_prompt_id: Mutex<Option<u64>>,
    prompt_running: AtomicBool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AcpSessionConfig {
    provider: ProviderChoice,
    model: Option<String>,
    tool_profile: String,
}

impl AcpSessionConfig {
    fn new(
        provider: ProviderChoice,
        model: Option<String>,
        tool_profile: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            model,
            tool_profile: normalize_tool_profile(&tool_profile.into()).to_string(),
        }
    }

    fn model_value(&self) -> &str {
        self.model.as_deref().unwrap_or("auto")
    }
}

struct AcpSession {
    daemon: Arc<DaemonSession>,
    config: Mutex<AcpSessionConfig>,
}

impl AcpSession {
    fn new(daemon: DaemonSession, config: AcpSessionConfig) -> Self {
        Self {
            daemon: Arc::new(daemon),
            config: Mutex::new(config),
        }
    }
}

impl DaemonSession {
    fn new(session_id: String, reader: ReadHalf, writer: WriteHalf, next_request_id: u64) -> Self {
        Self {
            session_id,
            reader: Mutex::new(BufReader::new(reader)),
            writer: Mutex::new(writer),
            next_request_id: AtomicU64::new(next_request_id),
            active_prompt_id: Mutex::new(None),
            prompt_running: AtomicBool::new(false),
        }
    }

    fn next_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn send(&self, request: &Request) -> Result<()> {
        let mut json = serde_json::to_string(request)?;
        json.push('\n');
        let mut writer = self.writer.lock().await;
        writer.write_all(json.as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn read_event(&self) -> Result<ServerEvent> {
        let mut line = String::new();
        let mut reader = self.reader.lock().await;
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            anyhow::bail!("Jcode daemon disconnected");
        }
        let event = serde_json::from_str(&line)
            .with_context(|| format!("failed to decode Jcode daemon event: {}", line.trim_end()))?;
        Ok(event)
    }
}

#[derive(Clone)]
struct AcpRuntime {
    stdout: Arc<Mutex<tokio::io::Stdout>>,
    sessions: Arc<Mutex<HashMap<String, Arc<AcpSession>>>>,
    profile: AcpProfile,
    provider_choice: ProviderChoice,
    model: Option<String>,
    provider_profile: Option<String>,
    default_tool_profile: String,
}

impl AcpRuntime {
    fn new(
        profile: AcpProfile,
        provider_choice: ProviderChoice,
        model: Option<String>,
        provider_profile: Option<String>,
        default_tool_profile: String,
    ) -> Self {
        Self {
            stdout: Arc::new(Mutex::new(tokio::io::stdout())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            profile,
            provider_choice,
            model,
            provider_profile,
            default_tool_profile: normalize_tool_profile(&default_tool_profile).to_string(),
        }
    }

    fn default_session_config(&self) -> AcpSessionConfig {
        AcpSessionConfig::new(
            self.provider_choice,
            self.model.clone(),
            self.default_tool_profile.clone(),
        )
    }

    async fn run(self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                return Ok(());
            }
            if line.trim().is_empty() {
                continue;
            }

            let message = match JsonRpcMessage::parse(&line) {
                Ok(message) => message,
                Err((code, message)) => {
                    self.write_error_value(
                        Value::Null,
                        code,
                        format!("Invalid JSON-RPC request: {message}"),
                    )
                    .await?;
                    continue;
                }
            };

            self.handle_message(message).await?;
        }
    }

    async fn handle_message(&self, message: JsonRpcMessage) -> Result<()> {
        let Some(method) = message.method.as_deref() else {
            if let Some(id) = message.id {
                self.write_error_value(
                    id,
                    JSONRPC_INVALID_REQUEST,
                    "JSON-RPC request missing method".to_string(),
                )
                .await?;
            }
            return Ok(());
        };

        match method {
            "initialize" => {
                if let Some(id) = message.id {
                    self.write_result(id, initialize_result(&message.params, self.profile))
                        .await?;
                }
            }
            "session/new" => self.handle_session_new(message).await?,
            "session/load" => self.handle_session_load(message, true).await?,
            "session/resume" => self.handle_session_load(message, false).await?,
            "session/set_config_option" => self.handle_session_set_config_option(message).await?,
            "session/set_mode" => self.handle_session_set_mode(message).await?,
            "session/prompt" => self.handle_session_prompt(message).await?,
            "session/cancel" => self.handle_session_cancel(message).await?,
            "session/close" => self.handle_session_close(message).await?,
            _ if method.starts_with('_') => {
                if let Some(id) = message.id {
                    self.write_error_value(
                        id,
                        JSONRPC_METHOD_NOT_FOUND,
                        format!("Unsupported Jcode ACP extension method: {method}"),
                    )
                    .await?;
                }
            }
            _ => {
                if let Some(id) = message.id {
                    self.write_error_value(
                        id,
                        JSONRPC_METHOD_NOT_FOUND,
                        format!("Unsupported ACP method: {method}"),
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_session_new(&self, message: JsonRpcMessage) -> Result<()> {
        let Some(id) = message.id else {
            return Ok(());
        };
        let cwd = match cwd_from_params(&message.params) {
            Ok(cwd) => cwd,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        if let Err(err) = ensure_no_acp_mcp_servers(&message.params) {
            self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                .await?;
            return Ok(());
        }

        match self.create_new_session(cwd).await {
            Ok(session) => {
                let session_id = session.session_id.clone();
                let config = self.default_session_config();
                self.sessions.lock().await.insert(
                    session_id.clone(),
                    Arc::new(AcpSession::new(session, config.clone())),
                );
                self.write_result(id, session_setup_result(Some(&session_id), &config))
                    .await?;
            }
            Err(err) => {
                self.write_error_value(
                    id,
                    JSONRPC_INTERNAL_ERROR,
                    format!("Failed to create Jcode session: {err:#}"),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn handle_session_load(
        &self,
        message: JsonRpcMessage,
        replay_history: bool,
    ) -> Result<()> {
        let Some(id) = message.id else {
            return Ok(());
        };
        let session_id = match required_session_id(&message.params) {
            Ok(session_id) => session_id,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        let cwd = match cwd_from_params(&message.params) {
            Ok(cwd) => cwd,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        if let Err(err) = ensure_no_acp_mcp_servers(&message.params) {
            self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                .await?;
            return Ok(());
        }

        match self
            .attach_existing_session(session_id.clone(), cwd, replay_history)
            .await
        {
            Ok(session) => {
                let config = self.default_session_config();
                self.sessions.lock().await.insert(
                    session.session_id.clone(),
                    Arc::new(AcpSession::new(session, config.clone())),
                );
                self.write_result(id, session_setup_result(None, &config))
                    .await?;
            }
            Err(err) => {
                self.write_error_value(
                    id,
                    JSONRPC_INTERNAL_ERROR,
                    format!("Failed to attach Jcode session '{session_id}': {err:#}"),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn handle_session_set_config_option(&self, message: JsonRpcMessage) -> Result<()> {
        let Some(id) = message.id else {
            return Ok(());
        };
        let session_id = match required_session_id(&message.params) {
            Ok(session_id) => session_id,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        let config_id = match required_string_param(&message.params, "configId") {
            Ok(value) => value,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        let value = match required_string_param(&message.params, "value") {
            Ok(value) => value,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };

        let session = {
            let sessions = self.sessions.lock().await;
            sessions.get(&session_id).cloned()
        };
        let Some(session) = session else {
            self.write_error_value(
                id,
                JSONRPC_INVALID_PARAMS,
                format!("Unknown ACP session id: {session_id}"),
            )
            .await?;
            return Ok(());
        };

        let config = {
            let mut config = session.config.lock().await;
            if let Err(err) = apply_config_option(&mut config, &config_id, &value) {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
            config.clone()
        };

        self.write_result(id, json!({ "configOptions": config_options(&config) }))
            .await?;
        Ok(())
    }

    async fn handle_session_set_mode(&self, message: JsonRpcMessage) -> Result<()> {
        let Some(id) = message.id else {
            return Ok(());
        };
        let session_id = match required_session_id(&message.params) {
            Ok(session_id) => session_id,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        let mode_id = match required_string_param(&message.params, "modeId") {
            Ok(value) => value,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        let session = {
            let sessions = self.sessions.lock().await;
            sessions.get(&session_id).cloned()
        };
        let Some(session) = session else {
            self.write_error_value(
                id,
                JSONRPC_INVALID_PARAMS,
                format!("Unknown ACP session id: {session_id}"),
            )
            .await?;
            return Ok(());
        };

        let config = {
            let mut config = session.config.lock().await;
            if let Err(err) = apply_config_option(&mut config, "tool_profile", &mode_id) {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
            config.clone()
        };

        self.write_result(
            id,
            json!({ "modes": modes(&config), "configOptions": config_options(&config) }),
        )
        .await?;
        Ok(())
    }

    async fn handle_session_prompt(&self, message: JsonRpcMessage) -> Result<()> {
        let Some(id) = message.id else {
            return Ok(());
        };
        let session_id = match required_session_id(&message.params) {
            Ok(session_id) => session_id,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        let (text, images) = match prompt_from_params(&message.params) {
            Ok(prompt) => prompt,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        let session = {
            let sessions = self.sessions.lock().await;
            sessions.get(&session_id).cloned()
        };
        let Some(session) = session else {
            self.write_error_value(
                id,
                JSONRPC_INVALID_PARAMS,
                format!("Unknown ACP session id: {session_id}"),
            )
            .await?;
            return Ok(());
        };

        if session
            .daemon
            .prompt_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            self.write_error_value(
                id,
                JSONRPC_SERVER_ERROR,
                format!("Session {session_id} is already processing a prompt"),
            )
            .await?;
            return Ok(());
        }

        let runtime = self.clone();
        tokio::spawn(async move {
            let result = runtime.run_prompt(id.clone(), session, text, images).await;
            if let Err(err) = result {
                let _ = runtime
                    .write_error_value(
                        id,
                        JSONRPC_INTERNAL_ERROR,
                        format!("Prompt failed: {err:#}"),
                    )
                    .await;
            }
        });
        Ok(())
    }

    async fn handle_session_cancel(&self, message: JsonRpcMessage) -> Result<()> {
        let session_id = match required_session_id(&message.params) {
            Ok(session_id) => session_id,
            Err(err) => {
                if let Some(id) = message.id {
                    self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                        .await?;
                }
                return Ok(());
            }
        };
        let session = {
            let sessions = self.sessions.lock().await;
            sessions.get(&session_id).cloned()
        };
        if let Some(session) = session {
            let cancel_id = session.daemon.next_id();
            let _ = session
                .daemon
                .send(&Request::Cancel { id: cancel_id })
                .await;
        }
        if let Some(id) = message.id {
            self.write_result(id, json!({})).await?;
        }
        Ok(())
    }

    async fn handle_session_close(&self, message: JsonRpcMessage) -> Result<()> {
        let Some(id) = message.id else {
            return Ok(());
        };
        let session_id = match required_session_id(&message.params) {
            Ok(session_id) => session_id,
            Err(err) => {
                self.write_error_value(id, JSONRPC_INVALID_PARAMS, err)
                    .await?;
                return Ok(());
            }
        };
        if let Some(session) = self.sessions.lock().await.remove(&session_id) {
            let cancel_id = session.daemon.next_id();
            let _ = session
                .daemon
                .send(&Request::Cancel { id: cancel_id })
                .await;
        }
        self.write_result(id, json!({})).await?;
        Ok(())
    }

    async fn ensure_daemon(&self) -> Result<()> {
        if dispatch::server_is_running().await {
            return Ok(());
        }
        dispatch::spawn_server(
            &self.provider_choice,
            self.model.as_deref(),
            self.provider_profile.as_deref(),
        )
        .await
    }

    async fn connect_daemon(&self) -> Result<(ReadHalf, WriteHalf)> {
        self.ensure_daemon().await?;
        let stream = crate::server::connect_socket(&crate::server::socket_path()).await?;
        Ok(stream.into_split())
    }

    async fn create_new_session(&self, cwd: PathBuf) -> Result<DaemonSession> {
        let (reader, writer) = self.connect_daemon().await?;
        let session = DaemonSession::new(String::new(), reader, writer, 2);
        let subscribe_id = 1;
        session
            .send(&Request::Subscribe {
                id: subscribe_id,
                working_dir: Some(cwd.display().to_string()),
                selfdev: None,
                target_session_id: None,
                client_instance_id: Some("acp".to_string()),
                client_has_local_history: false,
                allow_session_takeover: false,
                terminal_env: crate::terminal_launch::snapshot_client_terminal_env(),
                // ACP is a thin client that does not re-exec; stay legacy so the
                // server never sends it a handshake verdict event (NS1).
                protocol_version: None,
                build_hash: None,
                runtime_identity: None,
                spawn_swarm_id: None,
                spawn_session_id: None,
                client_pid: Some(std::process::id()),
            })
            .await?;
        wait_for_done(&session, subscribe_id).await?;
        let history = request_history(&session).await?;
        let session_id = match history {
            ServerEvent::History { session_id, .. } => session_id,
            other => anyhow::bail!("expected history after session creation, got {other:?}"),
        };
        Ok(DaemonSession::new(
            session_id,
            session.reader.into_inner().into_inner(),
            session.writer.into_inner(),
            session.next_request_id.load(Ordering::Relaxed),
        ))
    }

    async fn attach_existing_session(
        &self,
        target_session_id: String,
        _cwd: PathBuf,
        replay_history: bool,
    ) -> Result<DaemonSession> {
        let (reader, writer) = self.connect_daemon().await?;
        let session = DaemonSession::new(String::new(), reader, writer, 2);
        let resume_id = 1;
        session
            .send(&Request::ResumeSession {
                id: resume_id,
                session_id: target_session_id.clone(),
                client_instance_id: Some("acp".to_string()),
                client_has_local_history: false,
                allow_session_takeover: false,
            })
            .await?;

        let mut attached_id = target_session_id;
        loop {
            let event = session.read_event().await?;
            match event {
                ServerEvent::Ack { .. } => {}
                ServerEvent::History {
                    session_id,
                    messages,
                    ..
                } => {
                    attached_id = session_id.clone();
                    if replay_history {
                        self.replay_history(&session_id, messages).await?;
                    }
                }
                ServerEvent::Done { id } if id == resume_id => break,
                ServerEvent::Error { id, message, .. } if id == resume_id => {
                    anyhow::bail!(message);
                }
                other => {
                    if self.profile.is_extended() {
                        self.write_jcode_extension_event(&attached_id, &other)
                            .await?;
                    }
                }
            }
        }

        Ok(DaemonSession::new(
            attached_id,
            session.reader.into_inner().into_inner(),
            session.writer.into_inner(),
            session.next_request_id.load(Ordering::Relaxed),
        ))
    }

    async fn replay_history(
        &self,
        session_id: &str,
        messages: Vec<crate::protocol::HistoryMessage>,
    ) -> Result<()> {
        for message in messages {
            let update_name = match message.role.as_str() {
                "user" => "user_message_chunk",
                "assistant" => "agent_message_chunk",
                _ => "agent_message_chunk",
            };
            self.write_notification(
                "session/update",
                json!({
                    "sessionId": session_id,
                    "update": {
                        "sessionUpdate": update_name,
                        "content": {
                            "type": "text",
                            "text": message.content,
                        }
                    }
                }),
            )
            .await?;
        }
        Ok(())
    }

    async fn run_prompt(
        &self,
        rpc_id: Value,
        acp_session: Arc<AcpSession>,
        text: String,
        images: Vec<(String, String)>,
    ) -> Result<()> {
        let config = acp_session.config.lock().await.clone();
        let session = acp_session.daemon.clone();
        if let Some(model) = effective_model_spec(&config) {
            let set_model_id = session.next_id();
            if let Err(err) = session
                .send(&Request::SetModel {
                    id: set_model_id,
                    model,
                })
                .await
            {
                cleanup_prompt_state(&session).await;
                return Err(err);
            }
            if let Err(err) = wait_for_done(&session, set_model_id).await {
                cleanup_prompt_state(&session).await;
                return Err(err);
            }
        }

        let prompt_id = session.next_id();
        {
            let mut active = session.active_prompt_id.lock().await;
            *active = Some(prompt_id);
        }

        let send_result = session
            .send(&Request::Message {
                id: prompt_id,
                content: text,
                images,
                system_reminder: None,
            })
            .await;
        if let Err(err) = send_result {
            cleanup_prompt_state(&session).await;
            return Err(err);
        }

        let mut mapper = EventMapper::new(session.session_id.clone(), self.profile);
        let mut stop_reason = "end_turn".to_string();
        loop {
            let event = match session.read_event().await {
                Ok(event) => event,
                Err(err) => {
                    cleanup_prompt_state(&session).await;
                    return Err(err);
                }
            };
            if self.profile.is_extended() {
                self.write_jcode_extension_event(&session.session_id, &event)
                    .await?;
            }
            match event {
                ServerEvent::Ack { .. } => {}
                ServerEvent::Done { id } if id == prompt_id => break,
                ServerEvent::Interrupted => {
                    stop_reason = "cancelled".to_string();
                }
                ServerEvent::Error { id, message, .. } if id == prompt_id => {
                    cleanup_prompt_state(&session).await;
                    self.write_error_value(rpc_id, JSONRPC_SERVER_ERROR, message)
                        .await?;
                    return Ok(());
                }
                other => {
                    for update in mapper.map_event(other) {
                        self.write_notification(
                            "session/update",
                            json!({
                                "sessionId": session.session_id,
                                "update": update,
                            }),
                        )
                        .await?;
                    }
                }
            }
        }

        cleanup_prompt_state(&session).await;
        self.write_result(rpc_id, json!({ "stopReason": stop_reason }))
            .await?;
        Ok(())
    }

    async fn write_result(&self, id: Value, result: Value) -> Result<()> {
        self.write_value(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        }))
        .await
    }

    async fn write_error_value(&self, id: Value, code: i64, message: String) -> Result<()> {
        self.write_value(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message,
            }
        }))
        .await
    }

    async fn write_notification(&self, method: &str, params: Value) -> Result<()> {
        self.write_value(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
        .await
    }

    async fn write_jcode_extension_event(
        &self,
        session_id: &str,
        event: &ServerEvent,
    ) -> Result<()> {
        self.write_notification(
            "_jcode/server_event",
            json!({
                "sessionId": session_id,
                "event": serde_json::to_value(event).unwrap_or(Value::Null),
            }),
        )
        .await
    }

    async fn write_value(&self, value: Value) -> Result<()> {
        let mut stdout = self.stdout.lock().await;
        let mut line = serde_json::to_string(&value)?;
        line.push('\n');
        stdout.write_all(line.as_bytes()).await?;
        stdout.flush().await?;
        Ok(())
    }
}

async fn cleanup_prompt_state(session: &DaemonSession) {
    {
        let mut active = session.active_prompt_id.lock().await;
        *active = None;
    }
    session.prompt_running.store(false, Ordering::SeqCst);
}

async fn wait_for_done(session: &DaemonSession, request_id: u64) -> Result<()> {
    loop {
        match session.read_event().await? {
            ServerEvent::Ack { .. } => {}
            ServerEvent::Done { id } if id == request_id => return Ok(()),
            ServerEvent::Error { id, message, .. } if id == request_id => anyhow::bail!(message),
            _ => {}
        }
    }
}

async fn request_history(session: &DaemonSession) -> Result<ServerEvent> {
    let id = session.next_id();
    session.send(&Request::GetHistory { id }).await?;
    loop {
        match session.read_event().await? {
            ServerEvent::Ack { .. } => {}
            event @ ServerEvent::History { id: event_id, .. } if event_id == id => {
                return Ok(event);
            }
            ServerEvent::Error {
                id: event_id,
                message,
                ..
            } if event_id == id => anyhow::bail!(message),
            _ => {}
        }
    }
}

struct EventMapper {
    session_id: String,
    profile: AcpProfile,
    current_tool_id: Option<String>,
    tool_inputs: HashMap<String, String>,
}

impl EventMapper {
    fn new(session_id: String, profile: AcpProfile) -> Self {
        Self {
            session_id,
            profile,
            current_tool_id: None,
            tool_inputs: HashMap::new(),
        }
    }

    fn map_event(&mut self, event: ServerEvent) -> Vec<Value> {
        match event {
            ServerEvent::TextDelta { text } => vec![agent_message_chunk(text)],
            ServerEvent::TextReplace { text } => vec![agent_message_chunk(text)],
            ServerEvent::ToolStart { id, name } => {
                self.current_tool_id = Some(id.clone());
                self.tool_inputs.entry(id.clone()).or_default();
                vec![json!({
                    "sessionUpdate": "tool_call",
                    "toolCallId": id,
                    "title": tool_title(&name),
                    "kind": tool_kind(&name),
                    "status": "pending",
                })]
            }
            ServerEvent::ToolInput { delta } => {
                let Some(tool_id) = self.current_tool_id.clone() else {
                    return Vec::new();
                };
                let buffer = self.tool_inputs.entry(tool_id.clone()).or_default();
                buffer.push_str(&delta);
                let mut update = json!({
                    "sessionUpdate": "tool_call_update",
                    "toolCallId": tool_id,
                });
                if let Some(raw_input) = parse_json_object(buffer)
                    && let Some(object) = update.as_object_mut()
                {
                    object.insert("rawInput".to_string(), raw_input);
                }
                vec![update]
            }
            ServerEvent::ToolExec { id, name } => {
                self.current_tool_id = Some(id.clone());
                let mut update = json!({
                    "sessionUpdate": "tool_call_update",
                    "toolCallId": id,
                    "title": tool_title(&name),
                    "kind": tool_kind(&name),
                    "status": "in_progress",
                });
                if let Some(input) = self
                    .tool_inputs
                    .get(update["toolCallId"].as_str().unwrap_or_default())
                    && let Some(raw_input) = parse_json_object(input)
                    && let Some(object) = update.as_object_mut()
                {
                    object.insert("rawInput".to_string(), raw_input);
                }
                vec![update]
            }
            ServerEvent::ToolDone {
                id,
                name,
                output,
                error,
            } => vec![json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": id,
                "title": tool_title(&name),
                "kind": tool_kind(&name),
                "status": if error.is_some() { "failed" } else { "completed" },
                "content": [{
                    "type": "content",
                    "content": {
                        "type": "text",
                        "text": output,
                    }
                }],
                "rawOutput": {
                    "output": output,
                    "error": error,
                }
            })],
            ServerEvent::GeneratedImage {
                id,
                path,
                output_format,
                revised_prompt,
                ..
            } => vec![json!({
                "sessionUpdate": "tool_call_update",
                "toolCallId": id,
                "status": "completed",
                "content": [{
                    "type": "content",
                    "content": {
                        "type": "text",
                        "text": format!("Generated image: {path} ({output_format}){}", revised_prompt.map(|prompt| format!("\nRevised prompt: {prompt}")).unwrap_or_default()),
                    }
                }]
            })],
            ServerEvent::Compaction { trigger, .. } if self.profile.is_extended() => vec![json!({
                "sessionUpdate": "agent_message_chunk",
                "content": {
                    "type": "text",
                    "text": format!("\n[Jcode compacted context: {trigger}]\n"),
                }
            })],
            ServerEvent::SessionRenamed { display_title, .. } => vec![json!({
                "sessionUpdate": "session_info_update",
                "title": display_title,
            })],
            ServerEvent::McpStatus { servers } if self.profile.is_extended() => vec![json!({
                "sessionUpdate": "agent_message_chunk",
                "content": {
                    "type": "text",
                    "text": format!("\n[Jcode MCP status: {}]\n", servers.join(", ")),
                }
            })],
            _ => {
                let _ = &self.session_id;
                Vec::new()
            }
        }
    }
}

fn parse_json_object(input: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(input).ok()?;
    value.as_object()?;
    Some(value)
}

fn initialize_result(params: &Value, profile: AcpProfile) -> Value {
    // We only speak exactly ACP_PROTOCOL_VERSION; the response pins to our
    // version regardless of the `protocolVersion` the client requested.
    let _ = params;
    let protocol_version = ACP_PROTOCOL_VERSION;
    let mut agent_capabilities = json!({
        "loadSession": true,
        "promptCapabilities": {
            "image": true,
            "audio": false,
            "embeddedContext": true,
        },
        "mcpCapabilities": {
            "http": false,
            "sse": false,
        },
        "sessionCapabilities": {
            "close": {},
            "resume": {},
        }
    });

    if profile.is_extended()
        && let Some(object) = agent_capabilities.as_object_mut()
    {
        object.insert(
            "_meta".to_string(),
            json!({
                "jcode": {
                    "profile": profile.as_str(),
                    "extensions": ["raw_server_event"]
                }
            }),
        );
    }

    json!({
        "protocolVersion": protocol_version,
        "agentCapabilities": agent_capabilities,
        "agentInfo": {
            "name": "jcode",
            "title": "Jcode",
            "version": jcode_build_meta::PKG_VERSION,
        },
        "authMethods": [],
    })
}

fn session_setup_result(session_id: Option<&str>, config: &AcpSessionConfig) -> Value {
    let mut result = json!({
        "configOptions": config_options(config),
        "modes": modes(config),
    });
    if let Some(session_id) = session_id
        && let Some(object) = result.as_object_mut()
    {
        object.insert("sessionId".to_string(), json!(session_id));
    }
    result
}

fn config_options(config: &AcpSessionConfig) -> Value {
    json!([
        {
            "id": "provider",
            "name": "Provider",
            "description": "Jcode provider preference for subsequent prompts. Unsupported provider/model pairings are rejected by Jcode when the next prompt starts.",
            "category": "_provider",
            "type": "select",
            "currentValue": config.provider.as_arg_value(),
            "options": provider_option_values(),
        },
        {
            "id": "model",
            "name": "Model",
            "description": "Model to use for subsequent prompts. 'Auto' lets Jcode choose the provider default.",
            "category": "model",
            "type": "select",
            "currentValue": config.model_value(),
            "options": model_option_values(config.model.as_deref()),
        },
        {
            "id": "tool_profile",
            "name": "Tool Profile",
            "description": "Controls the tool capability profile exposed by Jcode.",
            "category": "mode",
            "type": "select",
            "currentValue": config.tool_profile,
            "options": tool_profile_option_values(),
        }
    ])
}

fn modes(config: &AcpSessionConfig) -> Value {
    json!({
        "currentModeId": config.tool_profile,
        "availableModes": tool_profile_modes(),
    })
}

fn provider_choices() -> &'static [ProviderChoice] {
    &[
        ProviderChoice::Auto,
        ProviderChoice::Jcode,
        ProviderChoice::Claude,
        ProviderChoice::AnthropicApi,
        ProviderChoice::Openai,
        ProviderChoice::OpenaiApi,
        ProviderChoice::Openrouter,
        ProviderChoice::Bedrock,
        ProviderChoice::Copilot,
        ProviderChoice::Gemini,
        ProviderChoice::GeminiApi,
        ProviderChoice::Antigravity,
        ProviderChoice::Google,
        ProviderChoice::Cursor,
    ]
}

fn provider_option_values() -> Value {
    Value::Array(
        provider_choices()
            .iter()
            .map(|choice| {
                json!({
                    "value": choice.as_arg_value(),
                    "name": provider_display_name(*choice),
                })
            })
            .collect(),
    )
}

fn provider_display_name(choice: ProviderChoice) -> &'static str {
    match choice {
        ProviderChoice::Auto => "Auto",
        ProviderChoice::Jcode => "Jcode",
        ProviderChoice::Claude => "Claude",
        ProviderChoice::AnthropicApi => "Anthropic API",
        ProviderChoice::Openai => "OpenAI",
        ProviderChoice::OpenaiApi => "OpenAI API",
        ProviderChoice::Openrouter => "OpenRouter",
        ProviderChoice::Bedrock => "Bedrock",
        ProviderChoice::Azure => "Azure OpenAI",
        ProviderChoice::Copilot => "GitHub Copilot",
        ProviderChoice::Gemini => "Gemini",
        ProviderChoice::GeminiApi => "Gemini API",
        ProviderChoice::Antigravity => "Antigravity",
        ProviderChoice::Google => "Google",
        ProviderChoice::Cursor => "Cursor",
        ProviderChoice::OpenaiCompatible => "OpenAI-compatible",
        ProviderChoice::Ollama => "Ollama",
        _ => choice.as_arg_value(),
    }
}

fn parse_provider_choice(value: &str) -> Option<ProviderChoice> {
    let trimmed = value.trim();
    provider_choices()
        .iter()
        .copied()
        .find(|choice| choice.as_arg_value().eq_ignore_ascii_case(trimmed))
}

fn model_option_values(current_model: Option<&str>) -> Value {
    let mut models = vec!["auto".to_string()];
    for model in ALL_CLAUDE_MODELS.iter().chain(ALL_OPENAI_MODELS.iter()) {
        push_unique(&mut models, model);
    }
    for model in cheap_extra_model_options() {
        push_unique(&mut models, model);
    }
    if let Some(model) = current_model {
        push_unique(&mut models, model);
    }
    Value::Array(
        models
            .into_iter()
            .map(|model| {
                let name = if model == "auto" {
                    "Auto".to_string()
                } else {
                    model.clone()
                };
                json!({ "value": model, "name": name })
            })
            .collect(),
    )
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn cheap_extra_model_options() -> &'static [&'static str] {
    &[
        "gemini-2.5-pro",
        "gemini-2.5-flash",
        "gemini-3-flash",
        "gpt-5.2-codex",
        "composer-2.5",
        "anthropic/claude-sonnet-4",
    ]
}

fn tool_profile_option_values() -> Value {
    Value::Array(
        tool_profile_modes()
            .into_iter()
            .map(|mut mode| {
                if let Some(object) = mode.as_object_mut()
                    && let Some(id) = object.remove("id")
                {
                    object.insert("value".to_string(), id);
                }
                mode
            })
            .collect(),
    )
}

fn tool_profile_modes() -> Vec<Value> {
    vec![
        json!({
            "id": "acp",
            "name": "ACP",
            "description": "Balanced tool set for ACP editor clients",
        }),
        json!({
            "id": "full",
            "name": "Full",
            "description": "Expose the full default Jcode tool set",
        }),
        json!({
            "id": "minimal",
            "name": "Minimal",
            "description": "Expose a smaller core coding tool set",
        }),
        json!({
            "id": "none",
            "name": "None",
            "description": "Disable built-in tools unless explicitly configured elsewhere",
        }),
    ]
}

fn normalize_tool_profile(value: &str) -> &str {
    parse_tool_profile(value).unwrap_or("acp")
}

fn parse_tool_profile(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "lite" | "small" => Some("minimal"),
        "off" | "disabled" => Some("none"),
        "full" => Some("full"),
        "none" => Some("none"),
        "minimal" => Some("minimal"),
        "acp" => Some("acp"),
        _ => None,
    }
}

fn apply_config_option(
    config: &mut AcpSessionConfig,
    config_id: &str,
    value: &str,
) -> std::result::Result<(), String> {
    match config_id.trim() {
        "provider" => {
            let provider = parse_provider_choice(value)
                .ok_or_else(|| format!("Unsupported ACP provider option: {value}"))?;
            config.provider = provider;
            Ok(())
        }
        "model" => {
            let value = value.trim();
            let valid = model_option_values(config.model.as_deref())
                .as_array()
                .map(|options| {
                    options
                        .iter()
                        .any(|option| option.get("value").and_then(Value::as_str) == Some(value))
                })
                .unwrap_or(false);
            if !valid {
                return Err(format!("Unsupported ACP model option: {value}"));
            }
            config.model = if value == "auto" {
                None
            } else {
                Some(value.to_string())
            };
            Ok(())
        }
        "tool_profile" | "permission_mode" | "mode" => {
            let normalized = parse_tool_profile(value)
                .ok_or_else(|| format!("Unsupported ACP tool profile option: {value}"))?;
            config.tool_profile = normalized.to_string();
            Ok(())
        }
        other => Err(format!("Unsupported ACP config option id: {other}")),
    }
}

fn effective_model_spec(config: &AcpSessionConfig) -> Option<String> {
    let model = config
        .model
        .as_deref()
        .or_else(|| provider_default_model(config.provider))?;
    Some(match provider_model_prefix(config.provider) {
        Some(prefix) => format!("{prefix}{model}"),
        None => model.to_string(),
    })
}

fn provider_model_prefix(provider: ProviderChoice) -> Option<&'static str> {
    match provider {
        ProviderChoice::Claude => Some("claude:"),
        ProviderChoice::AnthropicApi => Some("claude-api:"),
        ProviderChoice::Openai => Some("openai:"),
        ProviderChoice::OpenaiApi => Some("openai-api:"),
        ProviderChoice::Openrouter => Some("openrouter:"),
        ProviderChoice::Copilot => Some("copilot:"),
        ProviderChoice::Gemini | ProviderChoice::GeminiApi | ProviderChoice::Google => {
            Some("gemini:")
        }
        ProviderChoice::Antigravity => Some("antigravity:"),
        ProviderChoice::Cursor => Some("cursor:"),
        ProviderChoice::Bedrock => Some("bedrock:"),
        ProviderChoice::Auto | ProviderChoice::Jcode => None,
        _ => None,
    }
}

fn provider_default_model(provider: ProviderChoice) -> Option<&'static str> {
    match provider {
        ProviderChoice::Claude => Some("claude-opus-4-6"),
        ProviderChoice::AnthropicApi => Some("claude-opus-4-8"),
        ProviderChoice::Openai | ProviderChoice::OpenaiApi => Some("gpt-5.5"),
        ProviderChoice::Openrouter => Some("anthropic/claude-sonnet-4"),
        ProviderChoice::Copilot => Some("gpt-5.2-codex"),
        ProviderChoice::Gemini | ProviderChoice::GeminiApi | ProviderChoice::Google => {
            Some("gemini-2.5-pro")
        }
        ProviderChoice::Antigravity => Some("gemini-3-flash"),
        ProviderChoice::Cursor => Some("composer-2.5"),
        _ => None,
    }
}

fn cwd_from_params(params: &Value) -> std::result::Result<PathBuf, String> {
    let cwd = match params.get("cwd").and_then(Value::as_str) {
        Some(cwd) if !cwd.trim().is_empty() => PathBuf::from(cwd),
        _ => std::env::current_dir().map_err(|err| err.to_string())?,
    };
    if !cwd.is_absolute() {
        return Err(format!("ACP cwd must be absolute: {}", cwd.display()));
    }
    Ok(cwd)
}

fn required_string_param(params: &Value, name: &str) -> std::result::Result<String, String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("Missing required {name}"))
}

fn required_session_id(params: &Value) -> std::result::Result<String, String> {
    params
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "Missing required sessionId".to_string())
}

fn ensure_no_acp_mcp_servers(params: &Value) -> std::result::Result<(), String> {
    match params.get("mcpServers") {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Array(items)) if items.is_empty() => Ok(()),
        Some(_) => Err(
            "ACP mcpServers are not supported yet; configure MCP servers in Jcode config.toml"
                .to_string(),
        ),
    }
}

fn prompt_from_params(
    params: &Value,
) -> std::result::Result<(String, Vec<(String, String)>), String> {
    let prompt = params
        .get("prompt")
        .and_then(Value::as_array)
        .ok_or_else(|| "Missing required prompt array".to_string())?;
    let mut text_parts = Vec::new();
    let mut images = Vec::new();

    for block in prompt {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    text_parts.push(text.to_string());
                }
            }
            Some("image") => {
                let mime_type = block
                    .get("mimeType")
                    .or_else(|| block.get("mime_type"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Image content block missing mimeType".to_string())?;
                let data = block
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Image content block missing data".to_string())?;
                images.push((mime_type.to_string(), data.to_string()));
            }
            Some("resource") => {
                if let Some(resource) = block.get("resource") {
                    text_parts.push(format_resource_block(resource));
                }
            }
            Some("resource_link") => {
                let uri = block.get("uri").and_then(Value::as_str).unwrap_or("");
                let name = block.get("name").and_then(Value::as_str).unwrap_or(uri);
                text_parts.push(format!("[Resource link: {name} <{uri}>]"));
            }
            Some(other) => {
                return Err(format!(
                    "Unsupported ACP prompt content block type: {other}"
                ));
            }
            None => return Err("Prompt content block missing type".to_string()),
        }
    }

    Ok((text_parts.join("\n\n"), images))
}

fn format_resource_block(resource: &Value) -> String {
    let uri = resource
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or("resource");
    if let Some(text) = resource.get("text").and_then(Value::as_str) {
        format!("[Embedded resource: {uri}]\n{text}")
    } else if let Some(blob) = resource.get("blob").and_then(Value::as_str) {
        let mime = resource
            .get("mimeType")
            .or_else(|| resource.get("mime_type"))
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream");
        format!(
            "[Embedded binary resource: {uri} ({mime}, {} base64 bytes)]",
            blob.len()
        )
    } else {
        format!("[Embedded resource: {uri}]")
    }
}

fn agent_message_chunk(text: String) -> Value {
    json!({
        "sessionUpdate": "agent_message_chunk",
        "content": {
            "type": "text",
            "text": text,
        }
    })
}

fn tool_title(name: &str) -> String {
    match name {
        "bash" => "Running shell command".to_string(),
        "read" => "Reading file".to_string(),
        "write" => "Writing file".to_string(),
        "edit" | "multiedit" | "patch" | "apply_patch" => "Editing files".to_string(),
        "agentgrep" | "grep" | "glob" | "ls" => "Searching workspace".to_string(),
        "webfetch" | "websearch" => "Fetching web content".to_string(),
        other => other.replace('_', " "),
    }
}

pub(crate) fn tool_kind(name: &str) -> &'static str {
    match name {
        "read" => "read",
        "write" | "edit" | "multiedit" | "patch" | "apply_patch" => "edit",
        "bash" | "bg" | "selfdev" => "execute",
        "agentgrep" | "grep" | "glob" | "ls" | "session_search" | "conversation_search" => "search",
        "webfetch" | "websearch" | "codesearch" => "fetch",
        _ => "other",
    }
}

pub(crate) async fn run_acp_command(
    provider_choice: ProviderChoice,
    model: Option<String>,
    provider_profile: Option<String>,
    explicit_tool_profile: bool,
) -> Result<()> {
    crate::env::set_var("JCODE_NON_INTERACTIVE", "1");
    let acp_config = crate::config::config().acp.clone();
    if !explicit_tool_profile {
        crate::env::set_var("JCODE_TOOL_PROFILE", acp_config.tool_profile.trim());
        crate::config::invalidate_config_cache();
    }
    let profile = AcpProfile::parse(&acp_config.profile);
    AcpRuntime::new(
        profile,
        provider_choice,
        model,
        provider_profile,
        acp_config.tool_profile,
    )
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn acp_tool_kind_maps_core_tools() {
        assert_eq!(tool_kind("read"), "read");
        assert_eq!(tool_kind("apply_patch"), "edit");
        assert_eq!(tool_kind("bash"), "execute");
        assert_eq!(tool_kind("agentgrep"), "search");
        assert_eq!(tool_kind("webfetch"), "fetch");
        assert_eq!(tool_kind("swarm"), "other");
    }

    #[test]
    fn json_rpc_parse_errors_use_standard_codes() {
        let (code, _) = JsonRpcMessage::parse("not json").unwrap_err();
        assert_eq!(code, JSONRPC_PARSE_ERROR);

        let (code, message) = JsonRpcMessage::parse(r#"{"method":"initialize"}"#).unwrap_err();
        assert_eq!(code, JSONRPC_INVALID_REQUEST);
        assert!(message.contains("jsonrpc"));
    }

    #[test]
    fn prompt_from_params_accepts_text_images_and_resources() {
        let params = json!({
            "sessionId": "s1",
            "prompt": [
                {"type": "text", "text": "hello"},
                {"type": "image", "mimeType": "image/png", "data": "abc"},
                {"type": "resource", "resource": {"uri": "file:///tmp/a.rs", "text": "fn main(){}"}},
                {"type": "resource_link", "uri": "file:///tmp/b.rs", "name": "b.rs"}
            ]
        });
        let (text, images) = prompt_from_params(&params).unwrap();
        assert!(text.contains("hello"));
        assert!(text.contains("Embedded resource: file:///tmp/a.rs"));
        assert!(text.contains("Resource link: b.rs"));
        assert_eq!(images, vec![("image/png".to_string(), "abc".to_string())]);
    }

    #[test]
    fn initialize_standard_omits_jcode_meta() {
        let result = initialize_result(&json!({"protocolVersion": 1}), AcpProfile::Standard);
        assert_eq!(result["protocolVersion"], 1);
        assert!(result["agentCapabilities"].get("_meta").is_none());
        assert_eq!(result["agentCapabilities"]["loadSession"], true);
    }

    #[test]
    fn initialize_full_advertises_jcode_extension_meta() {
        let result = initialize_result(&json!({"protocolVersion": 1}), AcpProfile::Full);
        assert_eq!(
            result["agentCapabilities"]["_meta"]["jcode"]["profile"],
            "full"
        );
    }

    #[test]
    fn session_new_result_includes_config_options_and_modes() {
        let config = AcpSessionConfig::new(
            ProviderChoice::Auto,
            Some("claude-sonnet-4-5".to_string()),
            "lite",
        );
        let result = session_setup_result(Some("s1"), &config);
        assert_eq!(result["sessionId"], "s1");
        let options = result["configOptions"].as_array().expect("config options");
        assert_eq!(options[0]["id"], "provider");
        assert_eq!(options[0]["category"], "_provider");
        assert!(
            options[0]["options"]
                .as_array()
                .unwrap()
                .iter()
                .any(|option| { option["value"] == "gemini" })
        );
        assert!(
            options[0]["options"]
                .as_array()
                .unwrap()
                .iter()
                .any(|option| { option["value"] == "antigravity" })
        );
        assert_eq!(options[1]["id"], "model");
        assert_eq!(options[1]["category"], "model");
        assert_eq!(options[1]["currentValue"], "claude-sonnet-4-5");
        assert_eq!(options[2]["id"], "tool_profile");
        assert_eq!(options[2]["category"], "mode");
        assert_eq!(options[2]["currentValue"], "minimal");
        assert_eq!(result["modes"]["currentModeId"], "minimal");
    }

    #[test]
    fn session_load_result_includes_config_options_without_session_id() {
        let config = AcpSessionConfig::new(ProviderChoice::Gemini, None, "acp");
        let result = session_setup_result(None, &config);
        assert!(result.get("sessionId").is_none());
        assert_eq!(result["configOptions"][0]["currentValue"], "gemini");
        assert_eq!(result["configOptions"][1]["currentValue"], "auto");
    }

    #[test]
    fn apply_config_option_updates_provider_model_and_tool_profile() {
        let mut config = AcpSessionConfig::new(ProviderChoice::Auto, None, "acp");
        apply_config_option(&mut config, "provider", "antigravity").unwrap();
        assert_eq!(config.provider, ProviderChoice::Antigravity);

        apply_config_option(&mut config, "model", "gemini-2.5-pro").unwrap();
        assert_eq!(config.model.as_deref(), Some("gemini-2.5-pro"));

        apply_config_option(&mut config, "tool_profile", "full").unwrap();
        assert_eq!(config.tool_profile, "full");

        apply_config_option(&mut config, "model", "auto").unwrap();
        assert_eq!(config.model, None);
    }

    #[test]
    fn apply_config_option_rejects_unknown_ids_and_values() {
        let mut config = AcpSessionConfig::new(ProviderChoice::Auto, None, "acp");
        assert!(apply_config_option(&mut config, "provider", "bogus").is_err());
        assert!(apply_config_option(&mut config, "model", "not-a-listed-model").is_err());
        assert!(apply_config_option(&mut config, "tool_profile", "dangerous").is_err());
        assert!(apply_config_option(&mut config, "missing", "auto").is_err());
    }

    #[test]
    fn effective_model_spec_applies_provider_prefixes_for_subsequent_prompts() {
        let mut config = AcpSessionConfig::new(ProviderChoice::Auto, None, "acp");
        assert_eq!(effective_model_spec(&config), None);

        apply_config_option(&mut config, "provider", "gemini").unwrap();
        assert_eq!(
            effective_model_spec(&config).as_deref(),
            Some("gemini:gemini-2.5-pro")
        );

        apply_config_option(&mut config, "model", "gemini-2.5-flash").unwrap();
        assert_eq!(
            effective_model_spec(&config).as_deref(),
            Some("gemini:gemini-2.5-flash")
        );

        apply_config_option(&mut config, "provider", "antigravity").unwrap();
        apply_config_option(&mut config, "model", "auto").unwrap();
        assert_eq!(
            effective_model_spec(&config).as_deref(),
            Some("antigravity:gemini-3-flash")
        );
    }

    #[test]
    fn event_mapper_maps_tool_lifecycle() {
        let mut mapper = EventMapper::new("session1".to_string(), AcpProfile::Standard);
        let start = mapper.map_event(ServerEvent::ToolStart {
            id: "tool1".to_string(),
            name: "bash".to_string(),
        });
        assert_eq!(start[0]["sessionUpdate"], "tool_call");
        assert_eq!(start[0]["kind"], "execute");

        let input = mapper.map_event(ServerEvent::ToolInput {
            delta: "{\"command\":\"true\"}".to_string(),
        });
        assert_eq!(input[0]["rawInput"]["command"], "true");

        let done = mapper.map_event(ServerEvent::ToolDone {
            id: "tool1".to_string(),
            name: "bash".to_string(),
            output: "ok".to_string(),
            error: None,
        });
        assert_eq!(done[0]["status"], "completed");
        assert_eq!(done[0]["content"][0]["content"]["text"], "ok");
    }

    #[test]
    fn non_empty_mcp_servers_rejected_until_session_scoped_mcp_is_supported() {
        let params = json!({"mcpServers": [{"name": "fs"}]});
        assert!(ensure_no_acp_mcp_servers(&params).is_err());
        let params = json!({"mcpServers": []});
        assert!(ensure_no_acp_mcp_servers(&params).is_ok());
    }

    #[test]
    fn cwd_must_be_absolute() {
        let params = json!({"cwd": "relative"});
        assert!(cwd_from_params(&params).is_err());
        let params = json!({"cwd": "/tmp"});
        assert_eq!(cwd_from_params(&params).unwrap(), Path::new("/tmp"));
    }
}
