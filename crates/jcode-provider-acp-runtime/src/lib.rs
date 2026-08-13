//! Shared ACP stdio subprocess runtime for jcode providers.
//!
//! Provider-specific crates supply product identity, authentication choice,
//! prompt conversion, session setup, and update mapping. This crate owns the
//! bidirectional ACP v1 subprocess lifecycle and enforces fail-closed approval.

pub use agent_client_protocol as acp;

use acp::Agent as _;
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use futures::Stream;
use jcode_message_types::{Message, StreamEvent, ToolDefinition};
use jcode_provider_core::{EventStream, Provider};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::task::{Context as TaskContext, Poll};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

mod permission;

use permission::normalize_permission;

/// Executable and environment used to start one ACP agent process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpProcessSpec {
    pub command: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

/// Bounds for ACP requests, prompts, diagnostics, and stream buffering.
#[derive(Clone, Debug)]
pub struct AcpRuntimeConfig {
    pub request_timeout: Duration,
    pub prompt_timeout: Duration,
    pub stderr_limit: usize,
    pub event_buffer: usize,
}

impl Default for AcpRuntimeConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            prompt_timeout: Duration::from_secs(60 * 60),
            stderr_limit: 64 * 1024,
            event_buffer: 128,
        }
    }
}

/// A terminal authentication action surfaced to the host, never run on ACP stdio.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalAuthSpec {
    pub command: Option<PathBuf>,
    pub args: Vec<String>,
    pub meta: Option<Value>,
}

/// Authentication selected by a provider policy from advertised ACP methods.
#[derive(Clone, Debug, PartialEq)]
pub enum AcpAuthAction {
    None,
    Authenticate {
        method_id: acp::AuthMethodId,
        meta: Map<String, Value>,
    },
    TerminalLoginRequired(TerminalAuthSpec),
}

/// Normalized model catalog returned by a provider policy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscoveredModels {
    pub current: Option<String>,
    pub available: Vec<String>,
}

/// Session state supplied to provider-specific discovery and setup hooks.
#[derive(Clone, Debug)]
pub struct AcpSessionState {
    pub initialized: acp::InitializeResponse,
    pub session_id: acp::SessionId,
    pub resumed: bool,
    pub selected_model: Option<String>,
    pub models: Option<acp::SessionModelState>,
    pub config_options: Option<Vec<acp::SessionConfigOption>>,
    pub meta: Option<Map<String, Value>>,
}

/// Inputs available while a provider policy builds ACP prompt blocks.
pub struct AcpPromptInput<'a> {
    pub messages: &'a [Message],
    pub tools: &'a [ToolDefinition],
    pub system: &'a str,
    pub resumed: bool,
}

/// A provider-selected mutation to apply after opening an ACP session.
#[derive(Clone, Debug, PartialEq)]
pub enum AcpSessionMutation {
    SetModel {
        model_id: String,
        meta: Option<Map<String, Value>>,
    },
    SetConfigOption {
        config_id: String,
        value: String,
        meta: Option<Map<String, Value>>,
    },
}

/// Provider-specific ACP dialect and product policy.
#[async_trait]
pub trait AcpProviderPolicy: Send + Sync + 'static {
    fn provider_id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn process(&self) -> AcpProcessSpec;
    fn initialize_request(&self) -> acp::InitializeRequest;
    fn choose_auth(&self, initialized: &acp::InitializeResponse) -> Result<AcpAuthAction>;
    fn discover_models(
        &self,
        initialized: &acp::InitializeResponse,
        session: Option<&AcpSessionState>,
    ) -> DiscoveredModels;
    fn prompt_blocks(&self, input: AcpPromptInput<'_>) -> Result<Vec<acp::ContentBlock>>;
    fn session_setup(&self, state: &AcpSessionState) -> Result<Vec<AcpSessionMutation>>;
    fn map_update(&self, update: acp::SessionUpdate) -> Vec<StreamEvent>;
    fn login_hint(&self, error: &anyhow::Error) -> String;
}

/// Normalized ACP tool category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcpToolKind {
    Read,
    Edit,
    Delete,
    Move,
    Search,
    Execute,
    Think,
    Fetch,
    Other,
}

/// Provider content attached to a permission prompt, preserved as JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct AcpPermissionContent {
    pub value: Value,
}

/// A location attached to a permission prompt.
#[derive(Clone, Debug, PartialEq)]
pub struct AcpLocation {
    pub path: PathBuf,
    pub line: Option<u32>,
    pub meta: Option<Value>,
}

/// One opaque provider-advertised permission choice.
#[derive(Clone, Debug, PartialEq)]
pub struct AcpPermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: acp::PermissionOptionKind,
    pub meta: Option<Value>,
}

/// Complete permission request passed to the host without boolean reduction.
#[derive(Clone, Debug, PartialEq)]
pub struct AcpPermissionRequest {
    pub provider: &'static str,
    pub provider_session_id: String,
    pub tool_call_id: String,
    pub title: String,
    pub kind: AcpToolKind,
    pub raw_input: Option<Value>,
    pub content: Vec<AcpPermissionContent>,
    pub locations: Vec<AcpLocation>,
    pub options: Vec<AcpPermissionOption>,
    pub meta: Option<Value>,
}

/// Host decision. Selected IDs remain opaque provider protocol data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcpPermissionDecision {
    Select { option_id: String },
    Cancel,
}

/// Bridge from an active ACP turn into the host approval UI.
pub trait AcpPermissionBroker: Send + Sync {
    fn decide(
        &self,
        request: AcpPermissionRequest,
    ) -> Pin<Box<dyn Future<Output = AcpPermissionDecision> + Send + '_>>;
}

/// Shared protocol engine configuration and approval bridge.
pub struct AcpEngine {
    config: AcpRuntimeConfig,
    permission_broker: Option<Arc<dyn AcpPermissionBroker>>,
}

impl AcpEngine {
    pub fn new(
        config: AcpRuntimeConfig,
        permission_broker: Option<Arc<dyn AcpPermissionBroker>>,
    ) -> Self {
        Self {
            config,
            permission_broker,
        }
    }
}

/// Generic jcode provider wrapper backed by one ACP subprocess per turn.
pub struct AcpProvider<P> {
    policy: Arc<P>,
    engine: Arc<AcpEngine>,
    selected_model: Arc<RwLock<Option<String>>>,
    catalog: Arc<RwLock<DiscoveredModels>>,
}

impl<P: AcpProviderPolicy> AcpProvider<P> {
    pub fn new(policy: P) -> Self {
        Self::with_engine(policy, AcpRuntimeConfig::default(), None)
    }

    pub fn with_engine(
        policy: P,
        config: AcpRuntimeConfig,
        permission_broker: Option<Arc<dyn AcpPermissionBroker>>,
    ) -> Self {
        Self {
            policy: Arc::new(policy),
            engine: Arc::new(AcpEngine::new(config, permission_broker)),
            selected_model: Arc::new(RwLock::new(None)),
            catalog: Arc::new(RwLock::new(DiscoveredModels::default())),
        }
    }

    fn update_catalog(&self, discovered: DiscoveredModels) {
        let mut catalog = write_lock(&self.catalog);
        if !discovered.available.is_empty() {
            catalog.available = discovered.available;
        }
        if discovered.current.is_some() {
            catalog.current = discovered.current;
        }
    }
}

#[async_trait]
impl<P: AcpProviderPolicy> Provider for AcpProvider<P> {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: &str,
        resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        let prompt = self.policy.prompt_blocks(AcpPromptInput {
            messages,
            tools,
            system,
            resumed: resume_session_id.is_some(),
        })?;
        let (tx, rx) = mpsc::channel(self.engine.config.event_buffer.max(1));
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let error_limit = self.engine.config.stderr_limit;
        let turn = TurnSpec {
            policy: Arc::clone(&self.policy),
            engine: Arc::clone(&self.engine),
            selected_model: read_lock(&self.selected_model).clone(),
            catalog: Arc::clone(&self.catalog),
            resume_session_id: resume_session_id.map(ToOwned::to_owned),
            prompt,
            tx: tx.clone(),
            cancel_tx: cancel_tx.clone(),
            cancel_rx,
        };

        let thread = std::thread::Builder::new()
            .name(format!("jcode-{}-acp", self.policy.provider_id()))
            .spawn(move || {
                if let Err(error) = run_turn_thread(turn) {
                    tx.blocking_send(Ok(StreamEvent::Error {
                        message: bounded_text(format!("{error:#}"), error_limit),
                        retry_after_secs: None,
                    }))
                    .unwrap_or(());
                }
            })
            .context("Failed to start ACP runtime thread")?;

        Ok(Box::pin(AcpEventStream {
            inner: ReceiverStream::new(rx),
            cancel: Some(cancel_tx),
            thread: Some(thread),
        }))
    }

    fn name(&self) -> &str {
        self.policy.provider_id()
    }

    fn display_name(&self) -> String {
        self.policy.display_name().to_string()
    }

    fn model(&self) -> String {
        read_lock(&self.selected_model)
            .clone()
            .or_else(|| read_lock(&self.catalog).current.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn set_model(&self, model: &str) -> Result<()> {
        if model.trim().is_empty() {
            bail!("ACP model id cannot be empty");
        }
        let available = read_lock(&self.catalog).available.clone();
        if !available.is_empty() && !available.iter().any(|known| known == model) {
            bail!("Unknown {} model: {model}", self.policy.display_name());
        }
        *write_lock(&self.selected_model) = Some(model.to_string());
        Ok(())
    }

    fn available_models_display(&self) -> Vec<String> {
        read_lock(&self.catalog).available.clone()
    }

    async fn prefetch_models(&self) -> Result<()> {
        let policy = Arc::clone(&self.policy);
        let engine = Arc::clone(&self.engine);
        let operation_engine = Arc::clone(&engine);
        let discovered = run_on_acp_thread(policy.clone(), engine, move |connection| {
            Box::pin(async move {
                let initialized = initialize_and_authenticate(
                    &connection,
                    policy.as_ref(),
                    &operation_engine.config,
                )
                .await
                .map_err(|error| with_login_hint(policy.as_ref(), error))?;
                Ok(policy.discover_models(&initialized, None))
            })
        })
        .await?;
        self.update_catalog(discovered);
        Ok(())
    }

    fn handles_tools_internally(&self) -> bool {
        true
    }

    fn transport(&self) -> Option<String> {
        Some("ACP stdio".to_string())
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(Self {
            policy: Arc::clone(&self.policy),
            engine: Arc::clone(&self.engine),
            selected_model: Arc::new(RwLock::new(read_lock(&self.selected_model).clone())),
            catalog: Arc::new(RwLock::new(read_lock(&self.catalog).clone())),
        })
    }
}

struct AcpEventStream {
    inner: ReceiverStream<Result<StreamEvent>>,
    cancel: Option<watch::Sender<bool>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Stream for AcpEventStream {
    type Item = Result<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for AcpEventStream {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.send_replace(true);
        }
        self.thread.take();
    }
}

struct TurnSpec<P> {
    policy: Arc<P>,
    engine: Arc<AcpEngine>,
    selected_model: Option<String>,
    catalog: Arc<RwLock<DiscoveredModels>>,
    resume_session_id: Option<String>,
    prompt: Vec<acp::ContentBlock>,
    tx: mpsc::Sender<Result<StreamEvent>>,
    cancel_tx: watch::Sender<bool>,
    cancel_rx: watch::Receiver<bool>,
}

fn run_turn_thread<P: AcpProviderPolicy>(turn: TurnSpec<P>) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to build ACP Tokio runtime")?;
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let policy = Arc::clone(&turn.policy);
        let engine = Arc::clone(&turn.engine);
        let client_cancel_rx = turn.cancel_rx.clone();
        with_connection(
            policy.clone(),
            engine.clone(),
            turn.tx.clone(),
            client_cancel_rx,
            async move |connection, callbacks, graceful_shutdown| {
                let cancel_tx = turn.cancel_tx.clone();
                let mut cancel_rx = turn.cancel_rx;
                let Some(initialized) = until_cancelled(
                    &mut cancel_rx,
                    async {
                        initialize_and_authenticate(&connection, policy.as_ref(), &engine.config)
                            .await
                            .map_err(|error| with_login_hint(policy.as_ref(), error))
                    },
                )
                .await?
                else {
                    return Ok(());
                };
                let cwd = policy
                    .process()
                    .cwd
                    .map(Ok)
                    .unwrap_or_else(std::env::current_dir)
                    .context("Failed to determine ACP working directory")?;
                let resumed = turn.resume_session_id.is_some();
                let (session_id, models, config_options, meta) =
                    if let Some(session_id) = turn.resume_session_id {
                        let resumed_session_id = acp::SessionId::new(session_id.clone());
                        let Some(response) = until_cancelled(
                            &mut cancel_rx,
                            timeout_request(
                                &engine.config,
                                "session/resume",
                                connection.resume_session(acp::ResumeSessionRequest::new(
                                    session_id.clone(),
                                    cwd,
                                )),
                            ),
                        )
                        .await?
                        else {
                            return cancel_session(
                                &connection,
                                &engine.config,
                                &resumed_session_id,
                                graceful_shutdown.as_ref(),
                            )
                            .await;
                        };
                        (
                            resumed_session_id,
                            response.models,
                            response.config_options,
                            response.meta,
                        )
                    } else {
                        let Some(response) = until_cancelled(
                            &mut cancel_rx,
                            timeout_request(
                                &engine.config,
                                "session/new",
                                connection.new_session(
                                    acp::NewSessionRequest::new(cwd).mcp_servers(Vec::new()),
                                ),
                            ),
                        )
                        .await?
                        else {
                            return Ok(());
                        };
                        (
                            response.session_id,
                            response.models,
                            response.config_options,
                            response.meta,
                        )
                    };

                let state = AcpSessionState {
                    initialized,
                    session_id: session_id.clone(),
                    resumed,
                    selected_model: turn.selected_model,
                    models,
                    config_options,
                    meta,
                };
                turn.tx
                    .send(Ok(StreamEvent::SessionId(session_id.0.to_string())))
                    .await
                    .map_err(|_| anyhow!("ACP stream consumer closed"))?;

                let discovered = policy.discover_models(&state.initialized, Some(&state));
                {
                    let mut catalog = write_lock(&turn.catalog);
                    if !discovered.available.is_empty() {
                        catalog.available = discovered.available;
                    }
                    if discovered.current.is_some() {
                        catalog.current = discovered.current;
                    }
                }

                let mutations = policy.session_setup(&state)?;
                let Some(()) = until_cancelled(
                    &mut cancel_rx,
                    apply_mutations(&connection, &engine.config, &session_id, mutations),
                )
                .await?
                else {
                    return cancel_session(
                        &connection,
                        &engine.config,
                        &session_id,
                        graceful_shutdown.as_ref(),
                    )
                    .await;
                };

                let prompt_request = acp::PromptRequest::new(session_id.clone(), turn.prompt);
                let prompt = connection.prompt(prompt_request);
                tokio::pin!(prompt);
                let response = tokio::select! {
                    response = &mut prompt => {
                        response.map_err(|error| anyhow!("ACP session/prompt failed: {error}"))?
                    }
                    _ = tokio::time::sleep(engine.config.prompt_timeout) => {
                        cancel_tx.send_replace(true);
                        cancel_prompt(
                            &connection,
                            &engine.config,
                            &session_id,
                            callbacks.as_ref(),
                            graceful_shutdown.as_ref(),
                            prompt.as_mut(),
                        )
                        .await
                        .context("Failed to clean up timed-out ACP prompt")?;
                        bail!("ACP session/prompt timed out after {:?}", engine.config.prompt_timeout);
                    }
                    _ = wait_for_cancel(&mut cancel_rx) => {
                        return cancel_prompt(
                            &connection,
                            &engine.config,
                            &session_id,
                            callbacks.as_ref(),
                            graceful_shutdown.as_ref(),
                            prompt.as_mut(),
                        )
                        .await;
                    }
                };
                turn.tx
                    .send(Ok(StreamEvent::MessageEnd {
                        stop_reason: Some(
                            format!("{:?}", response.stop_reason).to_ascii_lowercase(),
                        ),
                    }))
                    .await
                    .map_err(|_| anyhow!("ACP stream consumer closed"))?;
                graceful_shutdown.store(true, Ordering::Release);
                Ok(())
            },
        )
        .await
    })
}

async fn apply_mutations(
    connection: &acp::ClientSideConnection,
    config: &AcpRuntimeConfig,
    session_id: &acp::SessionId,
    mutations: Vec<AcpSessionMutation>,
) -> Result<()> {
    for mutation in mutations {
        match mutation {
            AcpSessionMutation::SetModel { model_id, meta } => {
                let mut request = acp::SetSessionModelRequest::new(session_id.clone(), model_id);
                if let Some(meta) = meta {
                    request = request.meta(meta);
                }
                timeout_request(
                    config,
                    "session/set_model",
                    connection.set_session_model(request),
                )
                .await?;
            }
            AcpSessionMutation::SetConfigOption {
                config_id,
                value,
                meta,
            } => {
                let mut request =
                    acp::SetSessionConfigOptionRequest::new(session_id.clone(), config_id, value);
                if let Some(meta) = meta {
                    request = request.meta(meta);
                }
                timeout_request(
                    config,
                    "session/set_config_option",
                    connection.set_session_config_option(request),
                )
                .await?;
            }
        }
    }
    Ok(())
}

type LocalConnectionFuture<T> = Pin<Box<dyn Future<Output = Result<T>> + 'static>>;

async fn run_on_acp_thread<P, T>(
    policy: Arc<P>,
    engine: Arc<AcpEngine>,
    operation: impl FnOnce(acp::ClientSideConnection) -> LocalConnectionFuture<T> + Send + 'static,
) -> Result<T>
where
    P: AcpProviderPolicy,
    T: Send + 'static,
{
    let (result_tx, result_rx) = oneshot::channel();
    std::thread::Builder::new()
        .name(format!("jcode-{}-acp-probe", policy.provider_id()))
        .spawn(move || {
            let result = (|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let local = tokio::task::LocalSet::new();
                local.block_on(&runtime, async move {
                    let (_cancel_tx, cancel_rx) = watch::channel(false);
                    with_connection(
                        policy,
                        engine,
                        mpsc::channel(1).0,
                        cancel_rx,
                        move |connection, _callbacks, graceful_shutdown| async move {
                            let result = operation(connection).await;
                            if result.is_ok() {
                                graceful_shutdown.store(true, Ordering::Release);
                            }
                            result
                        },
                    )
                    .await
                })
            })();
            result_tx.send(result).unwrap_or(());
        })
        .context("Failed to start ACP probe thread")?;
    result_rx
        .await
        .context("ACP probe thread exited without a result")?
}

async fn with_connection<P, T, F, Fut>(
    policy: Arc<P>,
    engine: Arc<AcpEngine>,
    event_tx: mpsc::Sender<Result<StreamEvent>>,
    cancel_rx: watch::Receiver<bool>,
    operation: F,
) -> Result<T>
where
    P: AcpProviderPolicy,
    F: FnOnce(acp::ClientSideConnection, Arc<CallbackTracker>, Arc<AtomicBool>) -> Fut,
    Fut: Future<Output = Result<T>> + 'static,
{
    let process = policy.process();
    let mut command = Command::new(&process.command);
    command
        .args(&process.args)
        .envs(&process.env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = &process.cwd {
        command.current_dir(cwd);
    }
    let mut child = command.spawn().with_context(|| {
        format!(
            "Failed to launch {} ACP process at '{}'",
            policy.display_name(),
            process.command.display()
        )
    })?;
    let stdin = child.stdin.take().context("ACP child stdin unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("ACP child stdout unavailable")?;
    let stderr = child
        .stderr
        .take()
        .context("ACP child stderr unavailable")?;
    let stderr_capture = Arc::new(Mutex::new(String::new()));
    let mut stderr_task = tokio::task::spawn_local(capture_stderr(
        stderr,
        Arc::clone(&stderr_capture),
        engine.config.stderr_limit,
    ));
    let client = RuntimeClient {
        provider: policy.provider_id(),
        policy: Arc::clone(&policy),
        broker: engine.permission_broker.clone(),
        permission_timeout: engine.config.request_timeout,
        cancel_rx,
        tx: event_tx,
    };
    let callbacks = Arc::new(CallbackTracker::default());
    let graceful_shutdown = Arc::new(AtomicBool::new(false));
    let dispatcher_spawned = Arc::new(AtomicBool::new(false));
    let spawn_callbacks = Arc::clone(&callbacks);
    let spawn_dispatcher = Arc::clone(&dispatcher_spawned);
    let (connection, io) = acp::ClientSideConnection::new(
        client,
        stdin.compat_write(),
        stdout.compat(),
        move |future| {
            if spawn_dispatcher.swap(true, Ordering::AcqRel) {
                let active = spawn_callbacks.enter();
                tokio::task::spawn_local(async move {
                    let _active = active;
                    future.await;
                });
            } else {
                tokio::task::spawn_local(future);
            }
        },
    );
    let mut io_task = tokio::task::spawn_local(io);
    let operation = operation(connection, callbacks, Arc::clone(&graceful_shutdown));
    tokio::pin!(operation);
    let (result, io_finished) = tokio::select! {
        biased;
        result = &mut operation => (result, false),
        io_result = &mut io_task => (match io_result {
            Ok(Ok(())) => operation.as_mut().await,
            Ok(Err(error)) => Err(anyhow!("ACP transport failed: {error}")),
            Err(error) => Err(anyhow!("ACP transport task failed: {error}")),
        }, true),
    };
    if result.is_err() {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let io_shutdown = if io_finished {
        Ok(())
    } else {
        io_task.abort();
        match io_task.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(anyhow!("ACP transport failed during teardown: {error}")),
            Err(error) if error.is_cancelled() => Ok(()),
            Err(error) => Err(anyhow!(
                "ACP transport task failed during teardown: {error}"
            )),
        }
    };
    let teardown = terminate_child(&mut child, graceful_shutdown.load(Ordering::Acquire)).await;
    if !matches!(
        tokio::time::timeout(Duration::from_millis(25), &mut stderr_task).await,
        Ok(Ok(()))
    ) {
        stderr_task.abort();
    }
    let result = match (result, io_shutdown) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(shutdown)) => {
            Err(error.context(format!("ACP transport shutdown also failed: {shutdown:#}")))
        }
    };
    let result = match (result, teardown) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(teardown)) => {
            Err(error.context(format!("ACP process teardown also failed: {teardown:#}")))
        }
    };
    result.map_err(|error| {
        let stderr = mutex_lock(&stderr_capture);
        if stderr.trim().is_empty() {
            error
        } else {
            error.context(format!("ACP stderr: {}", stderr.trim()))
        }
    })
}

async fn cancel_session(
    connection: &acp::ClientSideConnection,
    config: &AcpRuntimeConfig,
    session_id: &acp::SessionId,
    graceful_shutdown: &AtomicBool,
) -> Result<()> {
    tokio::time::timeout(
        config.request_timeout,
        connection.cancel(acp::CancelNotification::new(session_id.clone())),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "ACP session cancellation timed out after {:?}",
            config.request_timeout
        )
    })?
    .map_err(|error| anyhow!("Failed to cancel ACP session: {error}"))?;
    tokio::task::yield_now().await;
    graceful_shutdown.store(true, Ordering::Release);
    Ok(())
}

async fn cancel_prompt<F, T>(
    connection: &acp::ClientSideConnection,
    config: &AcpRuntimeConfig,
    session_id: &acp::SessionId,
    callbacks: &CallbackTracker,
    graceful_shutdown: &AtomicBool,
    mut prompt: Pin<&mut F>,
) -> Result<()>
where
    F: Future<Output = acp::Result<T>>,
{
    tokio::time::timeout(config.request_timeout, async {
        connection
            .cancel(acp::CancelNotification::new(session_id.clone()))
            .await
            .map_err(|error| anyhow!("Failed to cancel ACP session: {error}"))?;
        let prompt_result = prompt.as_mut().await;
        drop(prompt_result);
        // ACP queues callback requests before delivering later prompt responses.
        // Yield once so the dispatcher drains that queue and registers every
        // callback synchronously with CallbackTracker before we wait for idle.
        tokio::task::yield_now().await;
        callbacks.wait_idle().await;
        graceful_shutdown.store(true, Ordering::Release);
        Ok(())
    })
    .await
    .map_err(|_| {
        anyhow!(
            "ACP prompt cancellation cleanup timed out after {:?}",
            config.request_timeout
        )
    })?
}

async fn terminate_child(child: &mut Child, graceful: bool) -> Result<()> {
    if child
        .try_wait()
        .context("Failed to inspect ACP child process")?
        .is_some()
    {
        return Ok(());
    }
    if graceful {
        match tokio::time::timeout(Duration::from_millis(250), child.wait()).await {
            Ok(status) => {
                status.context("Failed to reap ACP child process")?;
                return Ok(());
            }
            Err(_elapsed) => {}
        }
    }
    if let Err(error) = child.start_kill()
        && child
            .try_wait()
            .context("Failed to inspect ACP child process after kill failure")?
            .is_none()
    {
        return Err(error).context("Failed to terminate ACP child process");
    }
    tokio::time::timeout(Duration::from_millis(100), child.wait())
        .await
        .map_err(|_| anyhow!("ACP child process did not exit after forced termination"))?
        .context("Failed to reap ACP child process")?;
    Ok(())
}

fn with_login_hint<P: AcpProviderPolicy>(policy: &P, error: anyhow::Error) -> anyhow::Error {
    let hint = policy.login_hint(&error);
    error.context(hint)
}

async fn capture_stderr(
    mut stderr: tokio::process::ChildStderr,
    capture: Arc<Mutex<String>>,
    limit: usize,
) {
    let mut buffer = [0_u8; 4096];
    loop {
        let Ok(read) = stderr.read(&mut buffer).await else {
            return;
        };
        if read == 0 {
            return;
        }
        let chunk = String::from_utf8_lossy(&buffer[..read]);
        let mut output = mutex_lock(&capture);
        if output.len() < limit {
            output.push_str(&chunk);
            if output.len() > limit {
                *output = bounded_text(std::mem::take(&mut *output), limit);
            }
        }
    }
}

async fn initialize_and_authenticate<P: AcpProviderPolicy>(
    connection: &acp::ClientSideConnection,
    policy: &P,
    config: &AcpRuntimeConfig,
) -> Result<acp::InitializeResponse> {
    let mut request = policy.initialize_request();
    request.protocol_version = acp::ProtocolVersion::V1;
    // Phase 1 installs permission and session notification callbacks. The ACP
    // capability object has no flags for those baseline methods, so its empty
    // value truthfully advertises no filesystem or terminal callbacks.
    request.client_capabilities = acp::ClientCapabilities::new();
    let response = timeout_request(config, "initialize", connection.initialize(request)).await?;
    if response.protocol_version != acp::ProtocolVersion::V1 {
        bail!(
            "{} negotiated unsupported ACP protocol version {:?}",
            policy.display_name(),
            response.protocol_version
        );
    }
    match policy.choose_auth(&response)? {
        AcpAuthAction::None => {}
        AcpAuthAction::Authenticate { method_id, meta } => {
            timeout_request(
                config,
                "authenticate",
                connection.authenticate(acp::AuthenticateRequest::new(method_id).meta(meta)),
            )
            .await?;
        }
        AcpAuthAction::TerminalLoginRequired(spec) => {
            bail!("terminal authentication required: {spec:?}");
        }
    }
    Ok(response)
}

async fn timeout_request<T>(
    config: &AcpRuntimeConfig,
    name: &'static str,
    future: impl Future<Output = acp::Result<T>>,
) -> Result<T> {
    tokio::time::timeout(config.request_timeout, future)
        .await
        .map_err(|_| anyhow!("ACP {name} timed out after {:?}", config.request_timeout))?
        .map_err(|error| {
            anyhow!(
                "ACP {name} failed: {}",
                bounded_text(error.to_string(), config.stderr_limit)
            )
        })
}

fn bounded_text(text: String, limit: usize) -> String {
    if text.len() <= limit {
        return text;
    }
    const SUFFIX: &str = "...";
    if limit <= SUFFIX.len() {
        return SUFFIX[..limit].to_string();
    }
    let mut end = limit - SUFFIX.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut bounded = text[..end].to_string();
    bounded.push_str(SUFFIX);
    bounded
}

struct RuntimeClient<P> {
    provider: &'static str,
    policy: Arc<P>,
    broker: Option<Arc<dyn AcpPermissionBroker>>,
    permission_timeout: Duration,
    cancel_rx: watch::Receiver<bool>,
    tx: mpsc::Sender<Result<StreamEvent>>,
}

#[async_trait(?Send)]
impl<P: AcpProviderPolicy> acp::Client for RuntimeClient<P> {
    async fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let normalized = normalize_permission(self.provider, &request)?;
        let reject_once = request
            .options
            .iter()
            .find(|option| matches!(option.kind, acp::PermissionOptionKind::RejectOnce));
        let mut cancel_rx = self.cancel_rx.clone();
        let (decision, turn_cancelled) = match &self.broker {
            Some(broker) => tokio::select! {
                biased;
                _ = wait_for_cancel(&mut cancel_rx) => (AcpPermissionDecision::Cancel, true),
                decision = tokio::time::timeout(
                    self.permission_timeout,
                    broker.decide(normalized),
                ) => (decision.unwrap_or(AcpPermissionDecision::Cancel), false),
            },
            None if *cancel_rx.borrow() => (AcpPermissionDecision::Cancel, true),
            None => (AcpPermissionDecision::Cancel, false),
        };
        let selected = match decision {
            AcpPermissionDecision::Select { option_id } => request.options.iter().find(|option| {
                option.option_id.0.as_ref() == option_id
                    && !matches!(option.kind, acp::PermissionOptionKind::AllowAlways)
            }),
            AcpPermissionDecision::Cancel => None,
        };
        let outcome = if turn_cancelled {
            acp::RequestPermissionOutcome::Cancelled
        } else if let Some(option) = selected {
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                option.option_id.clone(),
            ))
        } else if let Some(option) = reject_once {
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                option.option_id.clone(),
            ))
        } else {
            acp::RequestPermissionOutcome::Cancelled
        };
        Ok(acp::RequestPermissionResponse::new(outcome))
    }

    async fn session_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> acp::Result<()> {
        for event in self.policy.map_update(notification.update) {
            if self.tx.send(Ok(event)).await.is_err() {
                return Ok(());
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct CallbackTracker {
    active: AtomicUsize,
    idle: Notify,
}

impl CallbackTracker {
    fn enter(self: &Arc<Self>) -> ActiveCallback {
        self.active.fetch_add(1, Ordering::AcqRel);
        ActiveCallback(Arc::clone(self))
    }

    async fn wait_idle(&self) {
        loop {
            let notified = self.idle.notified();
            if self.active.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
        }
    }
}

struct ActiveCallback(Arc<CallbackTracker>);

impl Drop for ActiveCallback {
    fn drop(&mut self) {
        if self.0.active.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.idle.notify_waiters();
        }
    }
}

async fn until_cancelled<T>(
    cancel_rx: &mut watch::Receiver<bool>,
    future: impl Future<Output = Result<T>>,
) -> Result<Option<T>> {
    tokio::select! {
        biased;
        _ = wait_for_cancel(cancel_rx) => Ok(None),
        result = future => result.map(Some),
    }
}

async fn wait_for_cancel(cancel_rx: &mut watch::Receiver<bool>) {
    if *cancel_rx.borrow() {
        return;
    }
    while cancel_rx.changed().await.is_ok() {
        if *cancel_rx.borrow() {
            return;
        }
    }
}

fn read_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn mutex_lock<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    lock.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod dependency_spike_tests;
