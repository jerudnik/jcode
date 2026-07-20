//! MCP Client - handles communication with a single MCP server

use super::protocol::*;
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

/// Max concurrent OWNED (non-shared) MCP child processes across the whole
/// process. Shared servers are pool-deduped and not counted here.
pub(crate) const MAX_OWNED_MCP_CHILDREN: usize = 64;
static OWNED_MCP_CHILDREN: AtomicUsize = AtomicUsize::new(0);

/// Environment contract used by owned MCP children to monitor their daemon.
pub const MCP_OWNER_PID_ENV: &str = "JCODE_MCP_OWNER_PID";

/// Default bound for a single-client shutdown outside the daemon coordinator.
pub(crate) const DEFAULT_MCP_REAP_GRACE: Duration = Duration::from_millis(225);

/// Per-request health deadline: a child that accepts a request but never
/// replies within this bound is declared dead (hung-child detection). This is
/// separate from (and defaults below) the 30s total request timeout.
pub(crate) const DEFAULT_MCP_HEALTH_DEADLINE: Duration = Duration::from_millis(15_000);

/// Env override (milliseconds) for the per-request health deadline.
pub const MCP_HEALTH_DEADLINE_ENV: &str = "JCODE_MCP_HEALTH_DEADLINE_MS";

/// Hard cap on any single request, regardless of health-deadline override.
const MCP_REQUEST_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Bound for the liveness ping probe sent when a request exceeds the health
/// deadline. A server that answers the ping is alive-but-slow: the original
/// request keeps waiting until the total timeout instead of being declared
/// hung (F07 review BLOCKING-2: slow tools must not be killed mid-execution).
const MCP_PING_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Marker attached to failures where the request provably never reached the
/// server (dead-flag pre-send, writer channel closed before send). Only these
/// failures are safe to auto-retry after a reconnect; anything that failed
/// after the request was written may have executed server-side, and re-sending
/// would double side effects (F07 review BLOCKING-2).
#[derive(Debug, Clone, Copy)]
pub(crate) struct RequestNotDelivered;

impl std::fmt::Display for RequestNotDelivered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("request was never delivered to the MCP server")
    }
}

impl std::error::Error for RequestNotDelivered {}

/// True iff `err` carries the [`RequestNotDelivered`] marker, i.e. an
/// automatic retry cannot double-execute the call.
pub(crate) fn error_permits_auto_retry(err: &anyhow::Error) -> bool {
    err.downcast_ref::<RequestNotDelivered>().is_some()
}

fn health_deadline() -> Duration {
    std::env::var(MCP_HEALTH_DEADLINE_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|ms| *ms > 0)
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_MCP_HEALTH_DEADLINE)
        .min(MCP_REQUEST_TOTAL_TIMEOUT)
}

/// Shared dead-flag for one MCP child. All handle clones (pool caches,
/// per-session `pool_handles` clones) observe death through the same Arc, so
/// stale caches cannot resurrect a dead child. First recorded reason wins.
#[derive(Debug, Default)]
pub(crate) struct DeathState {
    dead: std::sync::atomic::AtomicBool,
    reason: OnceLock<String>,
}

impl DeathState {
    /// Mark dead. Returns true only for the first caller.
    fn mark(&self, reason: String) -> bool {
        let first = !self.dead.swap(true, Ordering::SeqCst);
        let _ = self.reason.set(reason);
        first
    }

    fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }

    fn reason(&self) -> String {
        self.reason
            .get()
            .cloned()
            .unwrap_or_else(|| "unknown cause".to_string())
    }
}

/// Mark the handle dead and fail every pending request immediately by
/// dropping its oneshot sender (receivers observe closure at once instead of
/// burning the full request timeout).
async fn mark_dead_and_fail_pending(
    death: &DeathState,
    pending: &Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>,
    server_name: &str,
    reason: String,
) {
    if death.mark(reason.clone()) {
        crate::logging::warn(&format!("MCP [{server_name}]: marked dead: {reason}"));
    }
    let mut pending = pending.lock().await;
    // Dropping the senders wakes all waiting receivers immediately.
    pending.clear();
}

/// Debug/introspection record for one child process owned by this daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrackedMcpChild {
    pub server_name: String,
    pub pid: u32,
    pub owner_pid: u32,
}

/// Result of one bounded graceful -> TERM -> KILL reap pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpChildReapReport {
    pub initial: usize,
    pub term_signaled: Vec<u32>,
    pub kill_signaled: Vec<u32>,
    /// PIDs that still appeared live after SIGKILL and the grace deadline.
    /// Their tracking records are removed so shutdown cannot retain stale state.
    pub unreaped: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReapSignal {
    Term,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReapAction {
    pid: u32,
    signal: ReapSignal,
}

fn escalation_actions(pids: &[u32], signal: ReapSignal) -> Vec<ReapAction> {
    pids.iter()
        .copied()
        .map(|pid| ReapAction { pid, signal })
        .collect()
}

/// Process-wide child registry. The shared pool owns an Arc to this registry,
/// and per-session managers use the same Arc so shutdown can reap both pooled
/// and non-shared children in one bounded pass.
#[derive(Debug)]
pub struct McpChildTracker {
    owner_pid: u32,
    children: StdMutex<HashMap<u32, TrackedMcpChild>>,
}

static PROCESS_MCP_CHILD_TRACKER: OnceLock<Arc<McpChildTracker>> = OnceLock::new();

impl McpChildTracker {
    pub fn process() -> Arc<Self> {
        Arc::clone(PROCESS_MCP_CHILD_TRACKER.get_or_init(|| {
            Arc::new(Self {
                owner_pid: std::process::id(),
                children: StdMutex::new(HashMap::new()),
            })
        }))
    }

    #[cfg(test)]
    fn with_owner_pid(owner_pid: u32) -> Arc<Self> {
        Arc::new(Self {
            owner_pid,
            children: StdMutex::new(HashMap::new()),
        })
    }

    pub fn owner_pid(&self) -> u32 {
        self.owner_pid
    }

    fn register(&self, server_name: String, pid: u32) {
        self.children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                pid,
                TrackedMcpChild {
                    server_name,
                    pid,
                    owner_pid: self.owner_pid,
                },
            );
    }

    fn unregister(&self, pid: u32) {
        self.children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&pid);
    }

    pub fn tracked_children(&self) -> Vec<TrackedMcpChild> {
        let mut children: Vec<_> = self
            .children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect();
        children.sort_by_key(|child| child.pid);
        children
    }

    pub async fn reap_all(&self, grace: Duration) -> McpChildReapReport {
        let pids: Vec<u32> = self
            .children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .copied()
            .collect();
        self.reap_pids(&pids, grace).await
    }

    pub(crate) async fn reap_pids(&self, pids: &[u32], grace: Duration) -> McpChildReapReport {
        let tracked: Vec<u32> = {
            let children = self
                .children
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            pids.iter()
                .copied()
                .filter(|pid| children.contains_key(pid))
                .collect()
        };
        let initial = tracked.len();
        let started = Instant::now();
        let graceful_deadline = started + grace / 4;
        let term_deadline = started + grace * 3 / 4;
        let final_deadline = started + grace;

        self.wait_for_exit_until(&tracked, graceful_deadline).await;

        let live = self.live_tracked(&tracked);
        let term_actions = escalation_actions(&live, ReapSignal::Term);
        let term_signaled = apply_reap_actions(&term_actions);
        self.wait_for_exit_until(&tracked, term_deadline).await;

        let live = self.live_tracked(&tracked);
        let kill_actions = escalation_actions(&live, ReapSignal::Kill);
        let kill_signaled = apply_reap_actions(&kill_actions);
        self.wait_for_exit_until(&tracked, final_deadline).await;

        let unreaped = self.live_tracked(&tracked);
        // A completed cleanup step must never retain stale ownership records.
        // `unreaped` preserves visibility if the OS rejected or delayed SIGKILL.
        for pid in &tracked {
            self.unregister(*pid);
        }

        McpChildReapReport {
            initial,
            term_signaled,
            kill_signaled,
            unreaped,
        }
    }

    async fn wait_for_exit_until(&self, pids: &[u32], deadline: Instant) {
        loop {
            if self.live_tracked(pids).is_empty() || Instant::now() >= deadline {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn live_tracked(&self, pids: &[u32]) -> Vec<u32> {
        let registered: Vec<u32> = {
            let children = self
                .children
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            pids.iter()
                .copied()
                .filter(|pid| children.contains_key(pid))
                .collect()
        };

        let mut live = Vec::new();
        for pid in registered {
            if child_is_live(pid) {
                live.push(pid);
            } else {
                self.unregister(pid);
            }
        }
        live
    }
}

fn apply_reap_actions(actions: &[ReapAction]) -> Vec<u32> {
    actions
        .iter()
        .filter_map(|action| {
            signal_child(action.pid, action.signal)
                .ok()
                .map(|()| action.pid)
        })
        .collect()
}

#[cfg(unix)]
fn child_is_live(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    let mut status = 0;
    let result = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
    if result == pid {
        return false;
    }
    if result == 0 {
        return true;
    }

    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ECHILD) => {
            let result = unsafe { libc::kill(pid, 0) };
            result == 0
                || matches!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::EPERM)
                )
        }
        Some(libc::ESRCH) => false,
        _ => true,
    }
}

#[cfg(windows)]
fn child_is_live(pid: u32) -> bool {
    jcode_core::process::is_running(pid)
}

#[cfg(not(any(unix, windows)))]
fn child_is_live(_pid: u32) -> bool {
    true
}

#[cfg(unix)]
fn signal_child(pid: u32, signal: ReapSignal) -> std::io::Result<()> {
    let signal = match signal {
        ReapSignal::Term => libc::SIGTERM,
        ReapSignal::Kill => libc::SIGKILL,
    };
    let result = unsafe { libc::kill(pid as libc::pid_t, signal) };
    if result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn signal_child(pid: u32, signal: ReapSignal) -> std::io::Result<()> {
    let mut command = std::process::Command::new("taskkill.exe");
    command.args(["/PID", &pid.to_string(), "/T"]);
    if signal == ReapSignal::Kill {
        command.arg("/F");
    }
    let status = command.status()?;
    if status.success() || !child_is_live(pid) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "taskkill failed for MCP child {pid}: {status}"
        )))
    }
}

#[cfg(not(any(unix, windows)))]
fn signal_child(_pid: u32, _signal: ReapSignal) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "MCP child signaling is unsupported on this platform",
    ))
}

/// Pure bounded-CAS reservation against `counter`, capped at `cap`. Returns
/// true if a slot was reserved (the caller must release exactly once), false if
/// already at `cap`. Factored out of `try_acquire` so the cap logic can be
/// unit-tested on an isolated counter, free of the shared global static.
fn try_reserve(counter: &AtomicUsize, cap: usize) -> bool {
    let mut cur = counter.load(Ordering::Relaxed);
    loop {
        if cur >= cap {
            return false;
        }
        match counter.compare_exchange_weak(cur, cur + 1, Ordering::AcqRel, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(actual) => cur = actual,
        }
    }
}

/// RAII permit for one owned MCP child. Acquire before spawning; hold it for
/// the child's lifetime (store inside McpClient). Drop decrements the count.
#[derive(Debug)]
pub struct OwnedChildPermit;

impl OwnedChildPermit {
    /// Try to reserve a slot. Returns None if the cap is already reached.
    pub fn try_acquire() -> Option<Self> {
        if try_reserve(&OWNED_MCP_CHILDREN, MAX_OWNED_MCP_CHILDREN) {
            Some(OwnedChildPermit)
        } else {
            None
        }
    }

    /// Current owned-child count (for tests/telemetry).
    pub fn current() -> usize {
        OWNED_MCP_CHILDREN.load(Ordering::Relaxed)
    }
}

impl Drop for OwnedChildPermit {
    fn drop(&mut self) {
        OWNED_MCP_CHILDREN.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Shared communication handle for an MCP server.
/// Multiple sessions can hold clones of this and send concurrent requests.
/// Request/response correlation by ID ensures no interference.
#[derive(Clone)]
pub struct McpHandle {
    pub(crate) name: String,
    request_id: Arc<AtomicU64>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    writer_tx: mpsc::Sender<String>,
    server_info: Arc<std::sync::RwLock<Option<ServerInfo>>>,
    capabilities: Arc<std::sync::RwLock<ServerCapabilities>>,
    tools: Arc<std::sync::RwLock<Vec<McpToolDef>>>,
    death: Arc<DeathState>,
}

impl McpHandle {
    /// Whether the child behind this handle has been declared dead.
    pub fn is_dead(&self) -> bool {
        self.death.is_dead()
    }

    /// Human-readable death reason (meaningful only when `is_dead`).
    pub fn death_reason(&self) -> String {
        self.death.reason()
    }

    /// Identity check: two handles are the same generation iff they share
    /// one `DeathState`. Eviction must verify this so a session holding a
    /// stale dead clone cannot evict (and kill) a healthy replacement child
    /// that another session already reconnected (F07 review BLOCKING-1).
    pub(crate) fn same_generation(&self, other: &McpHandle) -> bool {
        Arc::ptr_eq(&self.death, &other.death)
    }

    fn death_error(&self) -> anyhow::Error {
        anyhow::anyhow!(
            "MCP server '{}' is dead: {}",
            self.name,
            self.death.reason()
        )
    }

    /// Send a request and wait for response
    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<JsonRpcResponse> {
        if self.death.is_dead() {
            // Never delivered: safe for callers to auto-retry on a fresh
            // handle without risking double execution.
            return Err(anyhow::Error::new(RequestNotDelivered).context(self.death_error()));
        }

        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        let msg = serde_json::to_string(&request)? + "\n";
        if self.writer_tx.send(msg).await.is_err() {
            self.pending.lock().await.remove(&id);
            mark_dead_and_fail_pending(
                &self.death,
                &self.pending,
                &self.name,
                format!("MCP server '{}' writer channel closed", self.name),
            )
            .await;
            // The write never happened: still safe to auto-retry.
            return Err(anyhow::Error::new(RequestNotDelivered).context(self.death_error()));
        }

        // From here on the request may have reached the server; failures must
        // NOT carry the retry marker (re-sending could double side effects).
        //
        // Health deadline: if no response arrives in time, probe liveness
        // with a protocol ping before declaring the child hung. An
        // alive-and-responsive server merely running a slow tool keeps its
        // request waiting until the total timeout (F07 review BLOCKING-2).
        let deadline = health_deadline();
        let mut rx = rx;
        let response = match tokio::time::timeout(deadline, &mut rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_recv_closed)) => {
                // Sender dropped: the reader/writer task failed all pending.
                return Err(self.death_error());
            }
            Err(_elapsed) => {
                if self.probe_liveness().await {
                    // Alive but slow: wait out the remaining total budget.
                    let remaining = MCP_REQUEST_TOTAL_TIMEOUT.saturating_sub(deadline);
                    match tokio::time::timeout(remaining, &mut rx).await {
                        Ok(Ok(response)) => response,
                        Ok(Err(_recv_closed)) => return Err(self.death_error()),
                        Err(_elapsed) => {
                            self.pending.lock().await.remove(&id);
                            anyhow::bail!(
                                "MCP request '{}' to '{}' timed out after {}s (server alive; \
                                 not retried to avoid double execution)",
                                method,
                                self.name,
                                MCP_REQUEST_TOTAL_TIMEOUT.as_secs()
                            );
                        }
                    }
                } else {
                    self.pending.lock().await.remove(&id);
                    mark_dead_and_fail_pending(
                        &self.death,
                        &self.pending,
                        &self.name,
                        format!(
                            "health deadline exceeded ({}ms waiting for '{}' response; \
                             liveness probe failed)",
                            deadline.as_millis(),
                            method
                        ),
                    )
                    .await;
                    return Err(self.death_error());
                }
            }
        };

        if let Some(err) = &response.error {
            anyhow::bail!("MCP error {}: {}", err.code, err.message);
        }

        Ok(response)
    }

    /// Protocol-level liveness probe: send a `ping` request and wait briefly
    /// for ANY response (a JSON-RPC "method not found" error still proves the
    /// event loop is alive). Only silence or transport failure counts as
    /// hung. Single-threaded servers blocked inside a long tool call will
    /// fail the probe; that is the accepted limit of hung-detection, and the
    /// retry gate still prevents double execution for them.
    async fn probe_liveness(&self) -> bool {
        if self.death.is_dead() {
            return false;
        }
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, "ping", None);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        let Ok(msg) = serde_json::to_string(&request) else {
            self.pending.lock().await.remove(&id);
            return false;
        };
        if self.writer_tx.send(msg + "\n").await.is_err() {
            self.pending.lock().await.remove(&id);
            return false;
        }
        match tokio::time::timeout(MCP_PING_PROBE_TIMEOUT, rx).await {
            Ok(Ok(_response)) => true,
            Ok(Err(_recv_closed)) => false,
            Err(_elapsed) => {
                self.pending.lock().await.remove(&id);
                false
            }
        }
    }

    /// Call a tool
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
        let arguments = if arguments.is_null() {
            Value::Object(serde_json::Map::new())
        } else {
            arguments
        };
        let params = ToolCallParams {
            name: name.to_string(),
            arguments,
        };

        let response = self
            .request("tools/call", Some(serde_json::to_value(params)?))
            .await?;

        let result = response.result.context("No result from tool call")?;
        let tool_result: ToolCallResult = serde_json::from_value(result)?;

        Ok(tool_result)
    }

    /// Get the server name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get server info
    pub fn server_info(&self) -> Option<ServerInfo> {
        self.server_info
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Get available tools
    pub fn tools(&self) -> Vec<McpToolDef> {
        self.tools
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Refresh the list of available tools
    pub async fn refresh_tools(&self) -> Result<()> {
        let response = self.request("tools/list", None).await?;

        if let Some(result) = response.result {
            let tools_result: ToolsListResult = serde_json::from_value(result)?;
            *self
                .tools
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = tools_result.tools;
        }

        Ok(())
    }
}

/// MCP Client - owns the child process and provides shared handles.
/// Only one McpClient exists per MCP server process, but many McpHandle
/// clones can be distributed to different sessions.
pub struct McpClient {
    handle: McpHandle,
    child: Child,
    child_pid: u32,
    child_tracker: Arc<McpChildTracker>,
    shutdown_started: bool,
    /// Set for owned (non-shared) clients; keeps the process-cap slot reserved
    /// until this client is dropped. None for pool/shared clients.
    _child_permit: Option<OwnedChildPermit>,
}

impl McpClient {
    /// Connect to an MCP server
    pub async fn connect(name: String, config: &McpServerConfig) -> Result<Self> {
        Self::connect_with_tracker(name, config, McpChildTracker::process()).await
    }

    pub(crate) async fn connect_with_tracker(
        name: String,
        config: &McpServerConfig,
        child_tracker: Arc<McpChildTracker>,
    ) -> Result<Self> {
        crate::logging::info(&format!(
            "MCP: Connecting to '{}' ({} {:?})",
            name, config.command, config.args
        ));

        let mut env: HashMap<String, String> = std::env::vars().collect();
        env.extend(config.env.clone());
        // The daemon owns this contract. A server config cannot spoof or erase
        // the parent identity used by `jcode mcp-serve` self-liveness.
        env.insert(
            MCP_OWNER_PID_ENV.to_string(),
            child_tracker.owner_pid().to_string(),
        );

        let mut child = Command::new(&config.command)
            .args(&config.args)
            .envs(&env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn MCP server: {}", config.command))?;

        let child_pid = child.id().context("Spawned MCP server has no process ID")?;
        child_tracker.register(name.clone(), child_pid);

        let stdin = child.stdin.take().context("No stdin")?;
        let stdout = child.stdout.take().context("No stdout")?;
        let stderr = child.stderr.take().context("No stderr")?;

        // Spawn stderr reader
        let server_name = name.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            crate::logging::warn(&format!(
                                "MCP [{}] stderr: {}",
                                server_name, trimmed
                            ));
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Setup channels
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(32);
        let death: Arc<DeathState> = Arc::new(DeathState::default());

        // Spawn writer task
        let mut stdin = stdin;
        let writer_death = Arc::clone(&death);
        let writer_pending = Arc::clone(&pending);
        let writer_name = name.clone();
        tokio::spawn(async move {
            while let Some(msg) = writer_rx.recv().await {
                let write_failed =
                    stdin.write_all(msg.as_bytes()).await.is_err() || stdin.flush().await.is_err();
                if write_failed {
                    mark_dead_and_fail_pending(
                        &writer_death,
                        &writer_pending,
                        &writer_name,
                        format!("MCP server '{writer_name}' stdin write failed (child exited?)"),
                    )
                    .await;
                    break;
                }
            }
        });

        // Spawn reader task
        let pending_clone = Arc::clone(&pending);
        let reader_death = Arc::clone(&death);
        let reader_name = name.clone();
        let mut reader = BufReader::new(stdout);
        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        crate::logging::debug(&format!("MCP [{}]: stdout EOF", reader_name));
                        mark_dead_and_fail_pending(
                            &reader_death,
                            &pending_clone,
                            &reader_name,
                            format!("MCP server '{reader_name}' exited (stdout EOF)"),
                        )
                        .await;
                        break;
                    }
                    Ok(_) => {
                        if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&line) {
                            if let Some(id) = response.id {
                                let mut pending = pending_clone.lock().await;
                                if let Some(tx) = pending.remove(&id) {
                                    let _ = tx.send(response);
                                }
                            }
                        } else {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                crate::logging::debug(&format!(
                                    "MCP [{}] non-JSON output: {}",
                                    reader_name, trimmed
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        crate::logging::warn(&format!("MCP [{}] read error: {}", reader_name, e));
                        mark_dead_and_fail_pending(
                            &reader_death,
                            &pending_clone,
                            &reader_name,
                            format!("MCP server '{reader_name}' stdout read error: {e}"),
                        )
                        .await;
                        break;
                    }
                }
            }
        });

        let handle = McpHandle {
            name: name.clone(),
            request_id: Arc::new(AtomicU64::new(1)),
            pending,
            writer_tx,
            server_info: Arc::new(std::sync::RwLock::new(None)),
            capabilities: Arc::new(std::sync::RwLock::new(ServerCapabilities::default())),
            tools: Arc::new(std::sync::RwLock::new(Vec::new())),
            death,
        };

        let mut client = Self {
            handle,
            child,
            child_pid,
            child_tracker,
            shutdown_started: false,
            _child_permit: None,
        };

        client
            .initialize()
            .await
            .with_context(|| format!("MCP server '{}' failed to initialize", name))?;

        client
            .handle
            .refresh_tools()
            .await
            .with_context(|| format!("MCP server '{}' failed to list tools", name))?;

        crate::logging::info(&format!(
            "MCP: Connected to '{}' with {} tools",
            name,
            client.handle.tools().len()
        ));

        Ok(client)
    }

    /// Get a shareable handle to this client
    pub fn handle(&self) -> McpHandle {
        self.handle.clone()
    }

    pub fn attach_child_permit(&mut self, permit: OwnedChildPermit) {
        self._child_permit = Some(permit);
    }

    /// Initialize the MCP connection
    async fn initialize(&mut self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo {
                name: "jcode".to_string(),
                version: jcode_build_meta::PKG_VERSION.to_string(),
            },
        };

        let response = self
            .handle
            .request("initialize", Some(serde_json::to_value(params)?))
            .await?;

        if let Some(result) = response.result {
            let init_result: InitializeResult = serde_json::from_value(result)?;
            *self
                .handle
                .server_info
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = init_result.server_info;
            *self
                .handle
                .capabilities
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = init_result.capabilities;
        }

        // Send initialized notification
        let notif = JsonRpcRequest::new(0, "notifications/initialized", None);
        let msg = serde_json::to_string(&notif)? + "\n";
        self.handle.writer_tx.send(msg).await?;

        Ok(())
    }

    /// Check if server is still running
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => false,
        }
    }

    /// Shutdown the server
    pub async fn shutdown(&mut self) {
        let pid = self.request_shutdown();
        let report = self
            .child_tracker
            .reap_pids(&[pid], DEFAULT_MCP_REAP_GRACE)
            .await;
        if !report.unreaped.is_empty() {
            crate::logging::warn(&format!(
                "MCP: child PID(s) still live after bounded reap: {:?}",
                report.unreaped
            ));
        }
    }

    pub(crate) fn request_shutdown(&mut self) -> u32 {
        self.shutdown_started = true;
        let _ = self
            .handle
            .writer_tx
            .try_send("{\"jsonrpc\":\"2.0\",\"method\":\"shutdown\"}\n".to_string());
        self.child_pid
    }

    // === Legacy compatibility methods that delegate to handle ===

    pub fn name(&self) -> &str {
        &self.handle.name
    }

    pub fn server_info(&self) -> Option<ServerInfo> {
        self.handle.server_info()
    }

    pub fn tools(&self) -> Vec<McpToolDef> {
        self.handle.tools()
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
        self.handle.call_tool(name, arguments).await
    }

    pub async fn refresh_tools(&self) -> Result<()> {
        self.handle.refresh_tools().await
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if self.shutdown_started {
            return;
        }

        let _ = self
            .handle
            .writer_tx
            .try_send("{\"jsonrpc\":\"2.0\",\"method\":\"shutdown\"}\n".to_string());
        let tracker = Arc::clone(&self.child_tracker);
        let pid = self.child_pid;
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = tracker.reap_pids(&[pid], DEFAULT_MCP_REAP_GRACE).await;
            });
        } else {
            let _ = signal_child(pid, ReapSignal::Term);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_OWNED_MCP_CHILDREN, McpChildTracker, OwnedChildPermit, ReapAction, ReapSignal,
        escalation_actions, try_reserve,
    };
    use std::process::Stdio;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn try_reserve_enforces_cap_on_isolated_counter() {
        // Deterministic: a private counter, immune to the shared global static
        // that other tests in this binary mutate concurrently.
        let counter = AtomicUsize::new(0);
        for _ in 0..3 {
            assert!(try_reserve(&counter, 3), "reserve under cap succeeds");
        }
        assert!(!try_reserve(&counter, 3), "at cap: refused");
        counter.fetch_sub(1, Ordering::AcqRel); // release one slot
        assert!(try_reserve(&counter, 3), "reserve succeeds after release");
    }

    #[test]
    fn acquire_returns_some_under_cap_and_drop_is_safe() {
        // Only non-flaky facts: cap is positive, a permit acquires under cap,
        // and dropping it does not panic. No assertion on the shared global's
        // absolute value (other tests mutate it in parallel).
        assert!(MAX_OWNED_MCP_CHILDREN > 0);
        let permit = OwnedChildPermit::try_acquire();
        assert!(permit.is_some(), "should acquire while far under cap");
        drop(permit);
    }

    #[test]
    fn fake_pid_escalation_orders_term_before_kill() {
        let pids = [101, 202];
        assert_eq!(
            escalation_actions(&pids, ReapSignal::Term),
            vec![
                ReapAction {
                    pid: 101,
                    signal: ReapSignal::Term,
                },
                ReapAction {
                    pid: 202,
                    signal: ReapSignal::Term,
                },
            ]
        );
        assert_eq!(
            escalation_actions(&pids, ReapSignal::Kill),
            vec![
                ReapAction {
                    pid: 101,
                    signal: ReapSignal::Kill,
                },
                ReapAction {
                    pid: 202,
                    signal: ReapSignal::Kill,
                },
            ]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn real_child_ignoring_term_is_killed_and_reaped_within_grace() {
        let tracker = McpChildTracker::with_owner_pid(std::process::id());
        let temp = tempfile::tempdir().expect("tempdir");
        let ready = temp.path().join("term-trap-ready");
        let mut child = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "trap '' TERM; : > \"$1\"; while :; do sleep 1; done",
                "f06-term-resistant-child",
                &ready.to_string_lossy(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn TERM-resistant child");
        let ready_deadline = Instant::now() + Duration::from_secs(1);
        while !ready.exists() && Instant::now() < ready_deadline {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(
            ready.exists(),
            "child must install its TERM trap before reap"
        );
        let pid = child.id();
        tracker.register("term-resistant-test".to_string(), pid);

        let started = Instant::now();
        let report = tracker.reap_all(Duration::from_millis(300)).await;
        assert_eq!(report.initial, 1);
        assert_eq!(report.term_signaled, vec![pid]);
        assert_eq!(report.kill_signaled, vec![pid]);
        assert!(
            report.unreaped.is_empty(),
            "SIGKILL must be observed within grace"
        );
        assert!(tracker.tracked_children().is_empty());
        assert!(started.elapsed() < Duration::from_secs(1));
        eprintln!(
            "F06_REAP pid={pid} owner_pid={} term={:?} kill={:?} unreaped={:?} tracked_after={} elapsed_ms={}",
            tracker.owner_pid(),
            report.term_signaled,
            report.kill_signaled,
            report.unreaped,
            tracker.tracked_children().len(),
            started.elapsed().as_millis()
        );

        // The tracker uses waitpid(WNOHANG), so the std Child may already be
        // externally reaped. Either outcome proves it is not still running.
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => {}
            Ok(None) => panic!("child survived TERM then KILL"),
        }
    }
}
