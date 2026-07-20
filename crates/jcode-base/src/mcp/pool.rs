//! Shared MCP Server Pool
//!
//! Manages a global pool of MCP server processes that are shared across
//! all jcode sessions. Instead of each session spawning its own set of
//! MCP servers (N sessions × M servers = N×M processes), sessions share
//! a single pool (M processes total).
//!
//! Sessions get lightweight `McpHandle` clones that can send concurrent
//! requests to shared server processes. Request/response correlation by
//! ID ensures no interference between sessions.

use super::client::{
    DEFAULT_MCP_REAP_GRACE, McpChildReapReport, McpChildTracker, McpClient, McpHandle,
    TrackedMcpChild,
};
use super::protocol::{McpConfig, McpServerConfig, McpToolDef};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, RwLock};

const FAILED_CONNECT_RETRY_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct FailedConnectRecord {
    message: String,
    failed_at: Instant,
}

type ConnectingMap = Arc<StdMutex<HashMap<String, Arc<Notify>>>>;

/// RAII ownership of one entry in the `connecting` map. The leader holds this
/// for the duration of its connect attempt; dropping it (normal return OR
/// future cancellation, e.g. an aborted tool call mid-reconnect) removes the
/// entry and wakes waiters, so a cancelled leader can never wedge subsequent
/// `ensure_connected` calls for the same server.
struct ConnectingEntryGuard {
    map: ConnectingMap,
    name: String,
    notify: Arc<Notify>,
}

impl Drop for ConnectingEntryGuard {
    fn drop(&mut self) {
        {
            let mut map = self
                .map
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if map
                .get(&self.name)
                .map(|current| Arc::ptr_eq(current, &self.notify))
                .unwrap_or(false)
            {
                map.remove(&self.name);
            }
        }
        self.notify.notify_waiters();
    }
}

enum ConnectAttempt {
    Connected,
    Leader(ConnectingEntryGuard),
    Wait(Arc<Notify>),
}

/// Global shared pool of MCP server processes.
///
/// Only one pool exists per jcode daemon. It owns the child processes
/// and hands out cheap `McpHandle` clones to sessions.
pub struct SharedMcpPool {
    clients: Mutex<HashMap<String, McpClient>>,
    /// Explicit owner/PID registry for every MCP child spawned by this daemon.
    child_tracker: Arc<McpChildTracker>,
    handles: RwLock<HashMap<String, McpHandle>>,
    config: RwLock<McpConfig>,
    ref_counts: Mutex<HashMap<String, usize>>,
    connecting: ConnectingMap,
    last_errors: RwLock<HashMap<String, FailedConnectRecord>>,
    /// Activity-lease authority (F01 C7): in-flight pool calls hold a lease
    /// so the daemon cannot idle-exit mid-call. Defaults to no-op; the
    /// daemon injects its real authority at the composition root.
    activity: std::sync::Arc<dyn jcode_core::activity::ActivityLeaseAuthority>,
}

impl SharedMcpPool {
    /// Create a new shared pool with the given config
    pub fn new(config: McpConfig) -> Self {
        Self::new_with_activity(config, jcode_core::activity::noop_activity_authority())
    }

    /// Create a new shared pool with an injected activity-lease authority.
    pub fn new_with_activity(
        config: McpConfig,
        activity: std::sync::Arc<dyn jcode_core::activity::ActivityLeaseAuthority>,
    ) -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            child_tracker: McpChildTracker::process(),
            handles: RwLock::new(HashMap::new()),
            config: RwLock::new(config),
            ref_counts: Mutex::new(HashMap::new()),
            connecting: Arc::new(StdMutex::new(HashMap::new())),
            last_errors: RwLock::new(HashMap::new()),
            activity,
        }
    }

    /// Create pool loading config from default locations
    pub fn from_default_config() -> Self {
        Self::new(McpConfig::load())
    }

    /// Create pool from default config with an injected activity authority.
    pub fn from_default_config_with_activity(
        activity: std::sync::Arc<dyn jcode_core::activity::ActivityLeaseAuthority>,
    ) -> Self {
        Self::new_with_activity(McpConfig::load(), activity)
    }

    /// Connect to all configured servers.
    /// Returns (successes, failures).
    pub async fn connect_all(&self) -> (usize, Vec<(String, String)>) {
        let config = self.config.read().await;
        let mut connect_futures = Vec::new();

        for (name, server_config) in &config.servers {
            // Disabled servers stay configured but are never auto-spawned
            // (issue #436); they can still be connected on demand by name.
            if !server_config.is_enabled() {
                continue;
            }
            let name = name.clone();
            let server_config = server_config.clone();
            connect_futures.push(async move {
                let result = self.ensure_connected(name.clone(), server_config).await;
                (name, result)
            });
        }
        drop(config);

        let mut successes = 0;
        let mut failures = Vec::new();

        for (name, result) in futures::future::join_all(connect_futures).await {
            match result {
                Ok(new_connection) => {
                    if new_connection {
                        successes += 1;
                    }
                }
                Err(error_msg) => {
                    crate::logging::error(&format!(
                        "Failed to connect to MCP server '{}': {}",
                        name, error_msg
                    ));
                    failures.push((name, error_msg));
                }
            }
        }

        if successes == 0 {
            successes = self.handles.read().await.len();
        }

        (successes, failures)
    }

    /// Connect to a specific server by name and config
    pub async fn connect_server(&self, name: &str, config: &McpServerConfig) -> Result<()> {
        self.ensure_connected(name.to_string(), config.clone())
            .await
            .map(|_| ())
            .map_err(|error_msg| anyhow::anyhow!(error_msg))
            .with_context(|| format!("Failed to connect to MCP server '{}'", name))
    }

    /// Disconnect a specific server
    pub async fn disconnect_server(&self, name: &str) {
        {
            let mut handles = self.handles.write().await;
            handles.remove(name);
        }
        {
            let mut clients = self.clients.lock().await;
            if let Some(mut client) = clients.remove(name) {
                client.shutdown().await;
            }
        }
        {
            let mut refs = self.ref_counts.lock().await;
            refs.remove(name);
        }
        {
            let mut errors = self.last_errors.write().await;
            errors.remove(name);
        }
    }

    /// Disconnect all pooled servers and return the child PIDs for a caller's
    /// bounded reap pass.
    pub async fn disconnect_all(&self) -> Vec<u32> {
        let mut child_pids = Vec::new();
        {
            let mut handles = self.handles.write().await;
            handles.clear();
        }
        {
            let mut clients = self.clients.lock().await;
            for (_, mut client) in clients.drain() {
                child_pids.push(client.request_shutdown());
            }
        }
        {
            let mut refs = self.ref_counts.lock().await;
            refs.clear();
        }
        {
            let mut errors = self.last_errors.write().await;
            errors.clear();
        }
        child_pids
    }

    /// Debug/introspection surface for owned child PID records.
    pub fn tracked_children(&self) -> Vec<TrackedMcpChild> {
        self.child_tracker.tracked_children()
    }

    /// Owning daemon PID injected into every spawned MCP child.
    pub fn owner_pid(&self) -> u32 {
        self.child_tracker.owner_pid()
    }

    /// Bounded graceful -> TERM -> KILL reap for every tracked MCP child,
    /// including per-session owned children registered through the same daemon
    /// tracker.
    pub async fn reap_tracked_children(&self, grace: Duration) -> McpChildReapReport {
        self.child_tracker.reap_all(grace).await
    }

    pub(crate) fn child_tracker(&self) -> Arc<McpChildTracker> {
        Arc::clone(&self.child_tracker)
    }

    /// Get handles for all connected servers (for a new session).
    /// Increments reference counts.
    pub async fn acquire_handles(&self, session_id: &str) -> HashMap<String, McpHandle> {
        let handles = self.handles.read().await;
        let result = handles.clone();

        let mut refs = self.ref_counts.lock().await;
        for name in result.keys() {
            *refs.entry(name.clone()).or_insert(0) += 1;
        }

        if !result.is_empty() {
            crate::logging::info(&format!(
                "MCP pool: session '{}' acquired {} server handle(s)",
                session_id,
                result.len()
            ));
        }

        result
    }

    /// Release handles when a session disconnects.
    /// Decrements reference counts.
    pub async fn release_handles(&self, session_id: &str, server_names: &[String]) {
        let mut refs = self.ref_counts.lock().await;
        for name in server_names {
            if let Some(count) = refs.get_mut(name) {
                *count = count.saturating_sub(1);
            }
        }

        if !server_names.is_empty() {
            crate::logging::info(&format!(
                "MCP pool: session '{}' released {} server handle(s)",
                session_id,
                server_names.len()
            ));
        }
    }

    /// Get a handle for a specific server
    pub async fn get_handle(&self, name: &str) -> Option<McpHandle> {
        let handles = self.handles.read().await;
        handles.get(name).cloned()
    }

    /// Get all available tools from all connected servers
    pub async fn all_tools(&self) -> Vec<(String, McpToolDef)> {
        let handles = self.handles.read().await;
        let mut tools = Vec::new();
        for (server_name, handle) in handles.iter() {
            for tool in handle.tools() {
                tools.push((server_name.clone(), tool));
            }
        }
        tools
    }

    /// Get list of connected server names
    pub async fn connected_servers(&self) -> Vec<String> {
        let handles = self.handles.read().await;
        handles.keys().cloned().collect()
    }

    /// Call a tool on a specific server
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
    ) -> Result<super::protocol::ToolCallResult> {
        // Activity lease (F01 C7): held for the full pooled call so the
        // daemon cannot idle-exit mid-call. A ShuttingDown refusal means no
        // new call may start during drain.
        let _lease = jcode_core::activity::ActivityLeaseGuard::acquire(
            &self.activity,
            jcode_core::activity::ActivityClass::McpCall,
            &format!("pool/{server}/{tool}"),
        )
        .map_err(|refused| anyhow::anyhow!("MCP call refused: {refused}"))?;
        let handle = {
            let handles = self.handles.read().await;
            handles.get(server).cloned()
        };
        let Some(handle) = handle else {
            // A recent death/connect failure fails fast here instead of
            // burning a fresh connect attempt every call (died-cooldown).
            if let Some(record) = self.recent_failure(server).await {
                anyhow::bail!(
                    "MCP server '{server}' unavailable: {} (reconnect suppressed briefly)",
                    record.message
                );
            }
            anyhow::bail!("MCP server '{server}' not connected");
        };
        if handle.is_dead() {
            let reason = handle.death_reason();
            let evicted = self.evict_dead_server(server, &handle).await;
            if !evicted {
                // A healthy replacement already exists (another session
                // reconnected): use it instead of erroring (BLOCKING-1).
                if let Some(fresh) = self.get_handle(server).await {
                    return fresh.call_tool(tool, arguments).await;
                }
            }
            // Dead pre-send: the call was never delivered, so one bounded
            // reconnect plus one retry cannot double-execute.
            return self
                .reconnect_and_retry_once(server, tool, arguments, &reason)
                .await;
        }
        let result = handle.call_tool(tool, arguments.clone()).await;
        if handle.is_dead() {
            // The call itself detected death (EOF, write failure, or health
            // deadline). Evict now so the next call reconnects cleanly.
            self.evict_dead_server(server, &handle).await;
            match result {
                Err(err) if super::client::error_permits_auto_retry(&err) => {
                    // The request provably never reached the server: safe to
                    // reconnect and retry without double execution.
                    let reason = handle.death_reason();
                    return self
                        .reconnect_and_retry_once(server, tool, arguments, &reason)
                        .await;
                }
                Err(err) => {
                    // Delivered-but-failed (or ambiguous): reconnect the pool
                    // eagerly so the NEXT call works, but never re-send this
                    // call (F07 review BLOCKING-2: double side effects).
                    self.reconnect_pool_only(server).await;
                    return Err(err);
                }
                Ok(value) => return Ok(value),
            }
        }
        result
    }

    /// One bounded reconnect after a dead-handle eviction, then one retry of
    /// the failed call on the fresh handle. Recovery never goes through
    /// `reload()`. Uses the pool's current config for the server (no stale
    /// caching). If the reconnect fails, `ensure_connected` has already
    /// recorded a failure so subsequent calls fail fast for the cooldown
    /// window; if the retried call dies too (crash loop), a died-cooldown
    /// record is written so the next call within the window fails fast
    /// without spawning another child.
    async fn reconnect_and_retry_once(
        &self,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
        death_reason: &str,
    ) -> Result<super::protocol::ToolCallResult> {
        let config = self.config.read().await.servers.get(server).cloned();
        let Some(config) = config else {
            anyhow::bail!(
                "MCP server '{server}' is dead ({death_reason}) and no longer configured; \
                 cannot reconnect"
            );
        };
        crate::logging::info(&format!(
            "MCP pool: reconnecting to dead server '{server}' once ({death_reason})"
        ));
        if let Err(error_msg) = self.ensure_connected(server.to_string(), config).await {
            anyhow::bail!(
                "MCP server '{server}' died ({death_reason}) and reconnect failed: {error_msg}"
            );
        }
        let handle = self
            .get_handle(server)
            .await
            .with_context(|| format!("MCP server '{server}' reconnected but exposed no handle"))?;
        let result = handle.call_tool(tool, arguments).await;
        if handle.is_dead() {
            // Died again right after a successful reconnect: crash loop.
            // Evict and enter died-cooldown so calls inside the window fail
            // fast without spawning more children.
            let reason = handle.death_reason();
            self.evict_dead_server(server, &handle).await;
            self.record_died_cooldown(server, &reason).await;
        }
        result
    }

    /// Eager pool-only reconnect after a delivered-but-failed call (the call
    /// itself is NOT retried; F07 review BLOCKING-2). Best-effort: failures
    /// are recorded by `ensure_connected` and surface on the next call.
    async fn reconnect_pool_only(&self, server: &str) {
        let config = self.config.read().await.servers.get(server).cloned();
        let Some(config) = config else { return };
        if let Err(error_msg) = self.ensure_connected(server.to_string(), config).await {
            crate::logging::warn(&format!(
                "MCP pool: eager reconnect of '{server}' failed: {error_msg}"
            ));
        }
    }

    /// Record a died-after-reconnect cooldown entry so `recent_failure` (and
    /// therefore `ensure_connected` + the call_tool fast-fail) suppresses
    /// reconnect attempts for `FAILED_CONNECT_RETRY_COOLDOWN`.
    async fn record_died_cooldown(&self, server: &str, reason: &str) {
        self.last_errors.write().await.insert(
            server.to_string(),
            FailedConnectRecord {
                message: format!("died shortly after reconnect: {reason}"),
                failed_at: Instant::now(),
            },
        );
    }

    /// Remove a dead server from the pool caches and reap its child through
    /// the tracker (F06 invariant: no leaked tracked children). Locks are
    /// never held across the awaited reap.
    ///
    /// Identity-checked (F07 review BLOCKING-1): eviction happens only if the
    /// pool's CURRENT handle is the same generation as `observed_dead`. A
    /// session holding a stale dead clone after another session already
    /// reconnected must not evict (and kill) the healthy replacement child.
    /// Returns true when an eviction actually happened.
    pub(crate) async fn evict_dead_server(&self, name: &str, observed_dead: &McpHandle) -> bool {
        {
            let mut handles = self.handles.write().await;
            match handles.get(name) {
                Some(current) if current.same_generation(observed_dead) => {
                    handles.remove(name);
                }
                Some(_healthy_replacement) => return false,
                // No pooled handle: another caller already evicted this
                // generation. Nothing further to do.
                None => return false,
            }
        }
        let client = {
            let mut clients = self.clients.lock().await;
            clients.remove(name)
        };
        {
            let mut refs = self.ref_counts.lock().await;
            refs.remove(name);
        }
        if let Some(mut client) = client {
            let pid = client.request_shutdown();
            let report = self
                .child_tracker
                .reap_pids(&[pid], DEFAULT_MCP_REAP_GRACE)
                .await;
            if !report.unreaped.is_empty() {
                crate::logging::warn(&format!(
                    "MCP pool eviction of '{name}': child PID(s) still live after bounded reap: {:?}",
                    report.unreaped
                ));
            }
        }
        crate::logging::warn(&format!("MCP pool: evicted dead server '{name}'"));
        true
    }

    /// Reload config and reconnect all servers
    pub async fn reload(&self) -> (usize, Vec<(String, String)>) {
        let child_pids = self.disconnect_all().await;
        let report = self
            .child_tracker
            .reap_pids(&child_pids, DEFAULT_MCP_REAP_GRACE)
            .await;
        if !report.unreaped.is_empty() {
            crate::logging::warn(&format!(
                "MCP pool reload: child PID(s) still live after bounded reap: {:?}",
                report.unreaped
            ));
        }
        *self.config.write().await = McpConfig::load();
        self.connect_all().await
    }

    /// Get current config
    pub async fn config(&self) -> McpConfig {
        self.config.read().await.clone()
    }

    /// Check if any servers are connected
    pub async fn has_connections(&self) -> bool {
        let handles = self.handles.read().await;
        !handles.is_empty()
    }

    /// Get reference counts (for debugging)
    pub async fn ref_counts(&self) -> HashMap<String, usize> {
        self.ref_counts.lock().await.clone()
    }

    async fn begin_connect(&self, name: &str) -> ConnectAttempt {
        // Handle check first: an existing handle means no connect is needed.
        if self.handles.read().await.contains_key(name) {
            return ConnectAttempt::Connected;
        }

        let mut connecting = self
            .connecting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(notify) = connecting.get(name) {
            return ConnectAttempt::Wait(Arc::clone(notify));
        }

        let notify = Arc::new(Notify::new());
        connecting.insert(name.to_string(), Arc::clone(&notify));
        ConnectAttempt::Leader(ConnectingEntryGuard {
            map: Arc::clone(&self.connecting),
            name: name.to_string(),
            notify,
        })
    }

    async fn finish_connect(
        &self,
        name: &str,
        guard: ConnectingEntryGuard,
        result: Result<McpClient>,
    ) {
        match result {
            Ok(client) => {
                let handle = client.handle();
                {
                    let mut handles = self.handles.write().await;
                    handles.insert(name.to_string(), handle);
                }
                {
                    let mut clients = self.clients.lock().await;
                    clients.insert(name.to_string(), client);
                }
                {
                    let mut errors = self.last_errors.write().await;
                    errors.remove(name);
                }
            }
            Err(error) => {
                let mut errors = self.last_errors.write().await;
                errors.insert(
                    name.to_string(),
                    FailedConnectRecord {
                        message: format!("{:#}", error),
                        failed_at: Instant::now(),
                    },
                );
            }
        }

        // Removes the connecting entry and wakes waiters (RAII drop).
        drop(guard);
    }

    async fn ensure_connected(
        &self,
        name: String,
        config: McpServerConfig,
    ) -> std::result::Result<bool, String> {
        if let Some(record) = self.recent_failure(&name).await {
            let retry_after = FAILED_CONNECT_RETRY_COOLDOWN
                .saturating_sub(record.failed_at.elapsed())
                .as_secs()
                .max(1);
            crate::logging::info(&format!(
                "MCP: Skipping reconnect to '{}' for {}s after recent failure",
                name, retry_after
            ));
            return Err(format!(
                "{} (retry suppressed for ~{}s after recent failure)",
                record.message, retry_after
            ));
        }

        match self.begin_connect(&name).await {
            ConnectAttempt::Connected => Ok(false),
            ConnectAttempt::Wait(notify) => {
                // Lost-wakeup guard (F07 review finding 3): `notify_waiters`
                // only wakes ALREADY-registered waiters. Register first
                // (enable), then confirm the connecting entry still exists;
                // a fast leader may have dropped its guard between
                // `begin_connect` releasing the map lock and our
                // registration, in which case waiting would hang forever.
                let notified = notify.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                let still_connecting = {
                    let map = self
                        .connecting
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    map.get(&name)
                        .map(|current| Arc::ptr_eq(current, &notify))
                        .unwrap_or(false)
                };
                if still_connecting {
                    notified.await;
                }
                if self.handles.read().await.contains_key(&name) {
                    Ok(false)
                } else {
                    let error = self
                        .last_errors
                        .read()
                        .await
                        .get(&name)
                        .map(|record| record.message.clone())
                        .unwrap_or_else(|| {
                            "Connection attempt did not produce a handle".to_string()
                        });
                    Err(error)
                }
            }
            ConnectAttempt::Leader(guard) => {
                // Re-check under leadership: another leader may have finished
                // connecting between our handle check and map insertion.
                if self.handles.read().await.contains_key(&name) {
                    drop(guard);
                    return Ok(false);
                }
                // `guard` is held across the connect await: if this future is
                // cancelled mid-connect (e.g. an aborted tool call), its Drop
                // removes the connecting entry and wakes waiters instead of
                // leaving them blocked forever.
                let result = McpClient::connect_with_tracker(
                    name.clone(),
                    &config,
                    Arc::clone(&self.child_tracker),
                )
                .await;
                let outcome = match &result {
                    Ok(_) => Ok(true),
                    Err(error) => Err(format!("{:#}", error)),
                };
                self.finish_connect(&name, guard, result).await;
                outcome
            }
        }
    }

    async fn recent_failure(&self, name: &str) -> Option<FailedConnectRecord> {
        if self.handles.read().await.contains_key(name) {
            return None;
        }

        self.last_errors
            .read()
            .await
            .get(name)
            .filter(|record| record.failed_at.elapsed() < FAILED_CONNECT_RETRY_COOLDOWN)
            .cloned()
    }
}

/// Global pool singleton
static SHARED_POOL: tokio::sync::OnceCell<Arc<SharedMcpPool>> = tokio::sync::OnceCell::const_new();

/// Initialize the global shared MCP pool. Call once at daemon startup.
pub async fn init_shared_pool() -> Arc<SharedMcpPool> {
    SHARED_POOL
        .get_or_init(|| async {
            let pool = SharedMcpPool::from_default_config();
            Arc::new(pool)
        })
        .await
        .clone()
}

/// Get the global shared pool, if initialized.
pub fn get_shared_pool() -> Option<Arc<SharedMcpPool>> {
    SHARED_POOL.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::{ConnectAttempt, SharedMcpPool};
    use crate::mcp::protocol::{McpConfig, McpServerConfig};
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::Arc;

    #[tokio::test]
    async fn begin_connect_deduplicates_concurrent_attempts() {
        let pool = Arc::new(SharedMcpPool::new(McpConfig::default()));

        let first = pool.begin_connect("demo").await;
        let second = pool.begin_connect("demo").await;

        let first_notify = match first {
            ConnectAttempt::Leader(guard) => Arc::clone(&guard.notify),
            _ => panic!("first attempt should lead"),
        };
        let second_notify = match second {
            ConnectAttempt::Wait(notify) => notify,
            _ => panic!("second attempt should wait"),
        };

        assert!(Arc::ptr_eq(&first_notify, &second_notify));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pooled_child_receives_and_records_owning_daemon_pid() {
        let temp = tempfile::tempdir().expect("tempdir");
        let capture = temp.path().join("owner-pid");
        let script = temp.path().join("owner-aware-mcp.sh");
        let body = r##"#!/bin/sh
printf '%s' "$JCODE_MCP_OWNER_PID" > "$OWNER_CAPTURE"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":'"$id"',"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"owner-aware","version":"1"}}}'
      ;;
    *'"tools/list"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":'"$id"',"result":{"tools":[]}}'
      ;;
    *'"shutdown"'*) exit 0 ;;
  esac
done
"##;
        let mut file = std::fs::File::create(&script).expect("create fixture");
        file.write_all(body.as_bytes()).expect("write fixture");
        drop(file);
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let pool = SharedMcpPool::new(McpConfig::default());
        let config = McpServerConfig {
            command: script.to_string_lossy().into_owned(),
            args: Vec::new(),
            env: HashMap::from([
                (
                    "OWNER_CAPTURE".to_string(),
                    capture.to_string_lossy().into_owned(),
                ),
                // The daemon-owned value must overwrite config spoofing.
                ("JCODE_MCP_OWNER_PID".to_string(), "1".to_string()),
            ]),
            shared: true,
            transport: None,
            url: None,
            enabled: None,
            disabled: None,
        };

        pool.connect_server("owner-pid-fixture", &config)
            .await
            .expect("connect fixture");
        let owner_pid = pool.owner_pid();
        assert_eq!(
            std::fs::read_to_string(&capture).expect("captured owner PID"),
            owner_pid.to_string()
        );
        let tracked = pool
            .tracked_children()
            .into_iter()
            .find(|child| child.server_name == "owner-pid-fixture")
            .expect("pooled child tracking record");
        assert_eq!(tracked.owner_pid, owner_pid);
        assert!(tracked.pid > 0);
        eprintln!(
            "F06_OWNER server={} child_pid={} owner_pid={} env_owner_pid={}",
            tracked.server_name,
            tracked.pid,
            tracked.owner_pid,
            std::fs::read_to_string(&capture).unwrap()
        );

        pool.disconnect_server("owner-pid-fixture").await;
        assert!(
            pool.tracked_children()
                .into_iter()
                .all(|child| child.pid != tracked.pid)
        );
    }

    /// F07 phase 2 cancellation-leak guard: a leader connect future aborted
    /// mid-connect must release the `connecting` entry (drop guard) so a
    /// subsequent ensure_connected for the same server completes instead of
    /// hanging forever behind a never-notified waiter.
    #[cfg(unix)]
    #[tokio::test]
    async fn cancelled_leader_connect_does_not_wedge_later_connects() {
        use std::io::Write as _;
        let temp = tempfile::tempdir().expect("tempdir");
        // Slow fixture: never answers initialize, so the leader connect is
        // guaranteed to be in-flight when we abort it.
        let slow = temp.path().join("slow-mcp.sh");
        std::fs::File::create(&slow)
            .and_then(|mut f| f.write_all(b"#!/bin/sh\nsleep 30\n"))
            .expect("write slow fixture");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&slow).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&slow, perms).unwrap();

        let base_config = McpServerConfig {
            command: slow.to_string_lossy().into_owned(),
            args: vec![],
            env: HashMap::new(),
            shared: true,
            transport: None,
            url: None,
            enabled: None,
            disabled: None,
        };

        let pool = Arc::new(SharedMcpPool::new(McpConfig::default()));
        let leader_pool = Arc::clone(&pool);
        let leader_config = base_config.clone();
        let leader = tokio::spawn(async move {
            let _ = leader_pool
                .connect_server("cancel-victim", &leader_config)
                .await;
        });
        // Let the leader claim the connecting entry and start its connect.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        leader.abort();
        let _ = leader.await; // drop of the leader future has completed

        // A follow-up connect must complete (here: fail fast on a dead
        // command) rather than hang waiting on the aborted leader's Notify.
        let fast_config = McpServerConfig {
            command: "true".to_string(),
            ..base_config
        };
        let followup = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            pool.connect_server("cancel-victim", &fast_config),
        )
        .await
        .expect("ensure_connected after cancelled leader must not hang");
        assert!(
            followup.is_err(),
            "'true' exits immediately, connect should fail cleanly"
        );
    }

    /// F07 review BLOCKING-1: a session holding a STALE dead handle clone
    /// must not evict (and kill) the healthy replacement child that another
    /// session already reconnected. Identity-checked eviction: same name,
    /// different generation => no-op, and the caller's next call flows to
    /// the healthy replacement.
    #[cfg(unix)]
    #[tokio::test]
    async fn stale_dead_clone_does_not_evict_healthy_replacement() {
        use std::io::Write as _;
        let temp = tempfile::tempdir().expect("tempdir");
        let script = temp.path().join("stale-gen-mcp.sh");
        let body = r##"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":'"$id"',"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"stale-gen","version":"1"}}}'
      ;;
    *'"tools/list"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":'"$id"',"result":{"tools":[{"name":"noop","description":"noop","inputSchema":{"type":"object"}}]}}'
      ;;
    *'"tools/call"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":'"$id"',"result":{"content":[{"type":"text","text":"ok"}],"isError":false}}'
      ;;
    *'"shutdown"'*) exit 0 ;;
  esac
done
"##;
        std::fs::File::create(&script)
            .and_then(|mut f| f.write_all(body.as_bytes()))
            .expect("write fixture");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let server = "stale-gen-fixture";
        let config = McpServerConfig {
            command: script.to_string_lossy().into_owned(),
            args: vec![],
            env: HashMap::new(),
            shared: true,
            transport: None,
            url: None,
            enabled: None,
            disabled: None,
        };
        let mut mcp_config = McpConfig::default();
        mcp_config
            .servers
            .insert(server.to_string(), config.clone());
        let pool = SharedMcpPool::new(mcp_config);

        // Generation 1: connect and keep a "session B" stale clone.
        pool.connect_server(server, &config)
            .await
            .expect("gen1 connect");
        let stale_clone = pool.get_handle(server).await.expect("gen1 handle");
        let gen1_pid = pool
            .tracked_children()
            .into_iter()
            .find(|child| child.server_name == server)
            .expect("gen1 tracked")
            .pid;

        // Kill gen1 and let session A's path evict + reconnect (gen 2).
        unsafe {
            libc::kill(gen1_pid as libc::pid_t, libc::SIGKILL);
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        assert!(stale_clone.is_dead(), "gen1 clone must observe death");
        let evicted = pool.evict_dead_server(server, &stale_clone).await;
        assert!(evicted, "same-generation eviction proceeds");
        pool.connect_server(server, &config)
            .await
            .expect("gen2 reconnect");
        let gen2_pid = pool
            .tracked_children()
            .into_iter()
            .find(|child| child.server_name == server)
            .expect("gen2 tracked")
            .pid;
        assert_ne!(gen1_pid, gen2_pid, "fresh child after reconnect");

        // Session B wakes up with its STALE dead clone and tries to evict.
        // Identity check must refuse: the healthy gen2 child survives.
        let evicted_again = pool.evict_dead_server(server, &stale_clone).await;
        assert!(
            !evicted_again,
            "stale-generation eviction must no-op (BLOCKING-1)"
        );
        let still_tracked = pool
            .tracked_children()
            .into_iter()
            .any(|child| child.server_name == server && child.pid == gen2_pid);
        assert!(
            still_tracked,
            "healthy gen2 child must NOT be killed by a stale clone"
        );

        // And the full call path with the stale clone gone: a pooled call
        // succeeds against gen2 without spawning a third child.
        let result = pool
            .call_tool(server, "noop", serde_json::json!({}))
            .await
            .expect("call against healthy replacement succeeds");
        assert!(!result.is_error);

        pool.disconnect_server(server).await;
    }
}
