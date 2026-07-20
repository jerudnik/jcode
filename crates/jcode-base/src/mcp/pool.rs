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
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, RwLock};

const FAILED_CONNECT_RETRY_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct FailedConnectRecord {
    message: String,
    failed_at: Instant,
}

enum ConnectAttempt {
    Connected,
    Leader(Arc<Notify>),
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
    connecting: Mutex<HashMap<String, Arc<Notify>>>,
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
            connecting: Mutex::new(HashMap::new()),
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
            handles
                .get(server)
                .cloned()
                .with_context(|| format!("MCP server '{}' not connected", server))?
        };
        if handle.is_dead() {
            let reason = handle.death_reason();
            self.evict_dead_server(server).await;
            anyhow::bail!("MCP server '{server}' is dead: {reason}");
        }
        let result = handle.call_tool(tool, arguments).await;
        if handle.is_dead() {
            // The call itself detected death (EOF, write failure, or health
            // deadline). Evict now so the next call reconnects cleanly.
            self.evict_dead_server(server).await;
        }
        result
    }

    /// Remove a dead server from the pool caches and reap its child through
    /// the tracker (F06 invariant: no leaked tracked children). Locks are
    /// never held across the awaited reap.
    pub(crate) async fn evict_dead_server(&self, name: &str) {
        {
            let mut handles = self.handles.write().await;
            handles.remove(name);
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
        let mut connecting = self.connecting.lock().await;
        if let Some(notify) = connecting.get(name) {
            return ConnectAttempt::Wait(Arc::clone(notify));
        }

        if self.handles.read().await.contains_key(name) {
            return ConnectAttempt::Connected;
        }

        let notify = Arc::new(Notify::new());
        connecting.insert(name.to_string(), Arc::clone(&notify));
        ConnectAttempt::Leader(notify)
    }

    async fn finish_connect(&self, name: &str, notify: Arc<Notify>, result: Result<McpClient>) {
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

        {
            let mut connecting = self.connecting.lock().await;
            if connecting
                .get(name)
                .map(|current| Arc::ptr_eq(current, &notify))
                .unwrap_or(false)
            {
                connecting.remove(name);
            }
        }

        notify.notify_waiters();
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
                notify.notified().await;
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
            ConnectAttempt::Leader(notify) => {
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
                self.finish_connect(&name, notify, result).await;
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
            ConnectAttempt::Leader(notify) => notify,
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
}
