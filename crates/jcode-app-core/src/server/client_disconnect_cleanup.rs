use super::{
    ClientConnectionInfo, ClientDebugState, FileTouchService, SessionInterruptQueues, SwarmEvent,
    SwarmEventType, SwarmMember, VersionedPlan, record_swarm_event, remove_background_tool_signal,
    remove_session_from_swarm, remove_session_interrupt_queue, unregister_session_event_sender,
    update_member_status,
};
use crate::agent::Agent;
use anyhow::Result;
use jcode_agent_runtime::InterruptSignal;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, broadcast};

#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

type SessionAgents = Arc<RwLock<HashMap<String, Arc<Mutex<Agent>>>>>;

const RELOAD_DISCONNECT_MARKER_MAX_AGE: Duration = Duration::from_secs(30);
const AGENT_LOCK_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(test)]
static TEST_AGENT_LOCK_TIMEOUT_MS: AtomicU64 = AtomicU64::new(u64::MAX);

fn agent_lock_timeout() -> Duration {
    #[cfg(test)]
    {
        let millis = TEST_AGENT_LOCK_TIMEOUT_MS.load(Ordering::SeqCst);
        if millis != u64::MAX {
            return Duration::from_millis(millis);
        }
    }
    AGENT_LOCK_TIMEOUT
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisconnectDisposition {
    Closed,
    Crashed,
    Reloading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CleanupClientConnectionOutcome {
    pub terminal_persistence: TerminalPersistenceOutcome,
    pub runtime_cleanup_completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TerminalPersistenceOutcome {
    Persisted,
    NotRequired,
    Failed,
    SkippedLockTimeout,
}

impl CleanupClientConnectionOutcome {
    fn full() -> Self {
        Self {
            terminal_persistence: TerminalPersistenceOutcome::Persisted,
            runtime_cleanup_completed: true,
        }
    }

    fn no_terminal_persistence_required() -> Self {
        Self {
            terminal_persistence: TerminalPersistenceOutcome::NotRequired,
            runtime_cleanup_completed: true,
        }
    }

    fn partial_without_terminal_persistence() -> Self {
        Self {
            terminal_persistence: TerminalPersistenceOutcome::Failed,
            runtime_cleanup_completed: true,
        }
    }

    fn skipped_terminal_persistence_on_lock_timeout() -> Self {
        Self {
            terminal_persistence: TerminalPersistenceOutcome::SkippedLockTimeout,
            runtime_cleanup_completed: true,
        }
    }

    pub(super) fn terminal_persistence_incomplete(self) -> bool {
        matches!(
            self.terminal_persistence,
            TerminalPersistenceOutcome::Failed | TerminalPersistenceOutcome::SkippedLockTimeout
        )
    }
}

fn disconnect_disposition(disconnected_while_processing: bool) -> DisconnectDisposition {
    if !disconnected_while_processing {
        return DisconnectDisposition::Closed;
    }

    if crate::server::reload_marker_active(RELOAD_DISCONNECT_MARKER_MAX_AGE) {
        DisconnectDisposition::Reloading
    } else {
        DisconnectDisposition::Crashed
    }
}

async fn session_has_live_successor(
    client_connections: &Arc<RwLock<HashMap<String, ClientConnectionInfo>>>,
    session_id: &str,
) -> bool {
    client_connections
        .read()
        .await
        .values()
        .any(|info| info.session_id == session_id)
}

#[expect(
    clippy::too_many_arguments,
    reason = "disconnect cleanup updates sessions, swarms, files, debug state, and shutdown signals together"
)]
pub(super) async fn cleanup_client_connection(
    sessions: &SessionAgents,
    client_session_id: &str,
    client_is_processing: bool,
    processing_task: &mut Option<tokio::task::JoinHandle<()>>,
    event_handle: tokio::task::JoinHandle<()>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    file_touch: &FileTouchService,
    client_debug_state: &Arc<RwLock<ClientDebugState>>,
    client_debug_id: &str,
    client_connections: &Arc<RwLock<HashMap<String, ClientConnectionInfo>>>,
    client_connection_id: &str,
    shutdown_signals: &Arc<RwLock<HashMap<String, InterruptSignal>>>,
    soft_interrupt_queues: &SessionInterruptQueues,
    event_history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    event_counter: &Arc<std::sync::atomic::AtomicU64>,
    swarm_event_tx: &broadcast::Sender<SwarmEvent>,
) -> Result<CleanupClientConnectionOutcome> {
    let disconnected_while_processing = client_is_processing
        || processing_task
            .as_ref()
            .map(|handle| !handle.is_finished())
            .unwrap_or(false);
    let disposition = disconnect_disposition(disconnected_while_processing);

    {
        let mut debug_state = client_debug_state.write().await;
        debug_state.unregister(client_debug_id);
    }
    {
        let mut connections = client_connections.write().await;
        connections.remove(client_connection_id);
    }
    unregister_session_event_sender(swarm_members, client_session_id, client_connection_id).await;

    // Release stale live ownership before slower cleanup so a reconnecting TUI can
    // reclaim the same session without tripping duplicate-attach guards.
    tokio::task::yield_now().await;

    let successor_connected =
        session_has_live_successor(client_connections, client_session_id).await;
    if successor_connected {
        crate::logging::info(&format!(
            "Skipping destructive disconnect cleanup for {} because another client is still attached",
            client_session_id
        ));
        event_handle.abort();
        return Ok(CleanupClientConnectionOutcome::no_terminal_persistence_required());
    }

    let mut terminal_persistence = TerminalPersistenceOutcome::NotRequired;
    {
        let mut sessions_guard = sessions.write().await;
        if let Some(agent_arc) = sessions_guard.remove(client_session_id) {
            drop(sessions_guard);
            let lock_result = tokio::time::timeout(agent_lock_timeout(), agent_arc.lock()).await;

            match lock_result {
                Ok(mut agent) => {
                    match disposition {
                        DisconnectDisposition::Closed => {
                            if let Err(error) = agent.mark_closed() {
                                terminal_persistence = TerminalPersistenceOutcome::Failed;
                                crate::logging::warn(&format!(
                                    "Failed to persist closed disconnect state for session {}: {}",
                                    client_session_id, error
                                ));
                            } else {
                                terminal_persistence = TerminalPersistenceOutcome::Persisted;
                            }
                        }
                        DisconnectDisposition::Reloading => {
                            if let Err(error) = agent.mark_crashed(Some(
                                "Server reload interrupted processing".to_string(),
                            )) {
                                terminal_persistence = TerminalPersistenceOutcome::Failed;
                                crate::logging::warn(&format!(
                                    "Failed to persist reloading disconnect state for session {}: {}",
                                    client_session_id, error
                                ));
                            } else {
                                terminal_persistence = TerminalPersistenceOutcome::Persisted;
                            }
                        }
                        DisconnectDisposition::Crashed => {
                            if let Err(error) = agent.mark_crashed(Some(
                                "Client disconnected while processing".to_string(),
                            )) {
                                terminal_persistence = TerminalPersistenceOutcome::Failed;
                                crate::logging::warn(&format!(
                                    "Failed to persist crashed disconnect state for session {}: {}",
                                    client_session_id, error
                                ));
                            } else {
                                terminal_persistence = TerminalPersistenceOutcome::Persisted;
                            }
                        }
                    }

                    let memory_enabled = agent.memory_enabled();
                    let transcript = if memory_enabled {
                        Some(agent.build_transcript_for_extraction())
                    } else {
                        None
                    };
                    let sid = client_session_id.to_string();
                    let working_dir = agent.working_dir().map(|dir| dir.to_string());
                    drop(agent);
                    let event = match disposition {
                        DisconnectDisposition::Closed => {
                            crate::runtime_memory_log::RuntimeMemoryLogEvent::new(
                                "session_closed",
                                "client_disconnected",
                            )
                        }
                        DisconnectDisposition::Crashed => {
                            crate::runtime_memory_log::RuntimeMemoryLogEvent::new(
                                "session_crashed",
                                "client_disconnected_while_processing",
                            )
                        }
                        DisconnectDisposition::Reloading => {
                            crate::runtime_memory_log::RuntimeMemoryLogEvent::new(
                                "session_reloading",
                                "server_reload_disconnect",
                            )
                        }
                    }
                    .with_session_id(sid.clone())
                    .force_attribution();
                    crate::runtime_memory_log::emit_event(event);
                    if let Some(transcript) = transcript {
                        crate::memory_agent::trigger_final_extraction_with_dir(
                            transcript,
                            sid,
                            working_dir,
                        );
                    }
                }
                Err(_) => {
                    terminal_persistence = TerminalPersistenceOutcome::SkippedLockTimeout;
                    crate::logging::warn(&format!(
                        "Session {} cleanup timed out waiting for agent lock (stuck task); skipping graceful shutdown",
                        client_session_id
                    ));
                }
            }
        }
    }

    {
        let (status, detail) = match disposition {
            DisconnectDisposition::Closed => ("stopped", Some("disconnected".to_string())),
            DisconnectDisposition::Crashed => {
                ("crashed", Some("disconnect while running".to_string()))
            }
            DisconnectDisposition::Reloading => {
                ("stopped", Some("server reload in progress".to_string()))
            }
        };
        update_member_status(
            client_session_id,
            status,
            detail,
            swarm_members,
            swarms_by_id,
            Some(event_history),
            Some(event_counter),
            Some(swarm_event_tx),
        )
        .await;

        let (swarm_id, removed_name) = {
            let mut members = swarm_members.write().await;
            if let Some(member) = members.remove(client_session_id) {
                (member.swarm_id, member.friendly_name)
            } else {
                (None, None)
            }
        };
        crate::session_metrics::forget(client_session_id);
        crate::session_effort::forget_session_effort(client_session_id);

        if let Some(ref swarm_id) = swarm_id {
            record_swarm_event(
                event_history,
                event_counter,
                swarm_event_tx,
                client_session_id.to_string(),
                removed_name.clone(),
                Some(swarm_id.clone()),
                SwarmEventType::MemberChange {
                    action: "left".to_string(),
                },
            )
            .await;
            remove_session_from_swarm(
                client_session_id,
                swarm_id,
                swarm_members,
                swarms_by_id,
                swarm_coordinators,
                swarm_plans,
            )
            .await;
        }
        file_touch.clear_session(client_session_id).await;
    }

    {
        let mut signals = shutdown_signals.write().await;
        signals.remove(client_session_id);
    }
    remove_background_tool_signal(client_session_id);
    remove_session_interrupt_queue(soft_interrupt_queues, client_session_id).await;

    if let Some(handle) = processing_task.take() {
        handle.abort();
    }

    event_handle.abort();
    Ok(match terminal_persistence {
        TerminalPersistenceOutcome::Persisted => CleanupClientConnectionOutcome::full(),
        TerminalPersistenceOutcome::NotRequired => {
            CleanupClientConnectionOutcome::no_terminal_persistence_required()
        }
        TerminalPersistenceOutcome::Failed => {
            CleanupClientConnectionOutcome::partial_without_terminal_persistence()
        }
        TerminalPersistenceOutcome::SkippedLockTimeout => {
            CleanupClientConnectionOutcome::skipped_terminal_persistence_on_lock_timeout()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ClientConnectionInfo, ClientDebugState, DisconnectDisposition, FileTouchService,
        InterruptSignal, SessionAgents, SwarmEvent, SwarmMember, TEST_AGENT_LOCK_TIMEOUT_MS,
        TerminalPersistenceOutcome, VersionedPlan, cleanup_client_connection,
        disconnect_disposition,
    };
    use crate::agent::Agent;
    use crate::message::{Message, StreamEvent, ToolDefinition};
    use crate::provider::{EventStream, Provider};
    use crate::session::{Session, SessionStatus};
    use crate::tool::Registry;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::time::Instant;
    use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
    use tokio_stream::wrappers::ReceiverStream;

    struct EnvGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let prev = std::env::var_os(key);
            crate::env::set_var(key, value);
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(prev) = &self.prev {
                crate::env::set_var(self.key, prev);
            } else {
                crate::env::remove_var(self.key);
            }
        }
    }

    struct TestProvider;

    #[async_trait]
    impl Provider for TestProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
            _system: &str,
            _resume_session_id: Option<&str>,
        ) -> Result<EventStream> {
            let (_tx, rx) = mpsc::channel::<Result<StreamEvent>>(1);
            Ok(Box::pin(ReceiverStream::new(rx)))
        }

        fn name(&self) -> &str {
            "test"
        }

        fn fork(&self) -> Arc<dyn Provider> {
            Arc::new(Self)
        }
    }

    fn test_swarm_member(session_id: &str) -> SwarmMember {
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        SwarmMember {
            session_id: session_id.to_string(),
            event_tx,
            event_txs: HashMap::new(),
            working_dir: None,
            swarm_id: Some("swarm-r04".to_string()),
            swarm_enabled: true,
            status: "running".to_string(),
            detail: None,
            task_label: None,
            subagent_type: None,
            friendly_name: Some(session_id.to_string()),
            report_back_to_session_id: Some("coordinator".to_string()),
            initial_prompt_delivered: None,
            latest_completion_report: None,
            role: "agent".to_string(),
            joined_at: Instant::now(),
            last_status_change: Instant::now(),
            is_headless: false,
            output_tail: None,
            todo_progress: None,
            todo_items: Vec::new(),
            runtime: crate::protocol::SwarmMemberRuntime::default(),
        }
    }

    struct CleanupHarness {
        sessions: SessionAgents,
        swarm_members: Arc<RwLock<HashMap<String, SwarmMember>>>,
        swarms_by_id: Arc<RwLock<HashMap<String, HashSet<String>>>>,
        swarm_coordinators: Arc<RwLock<HashMap<String, String>>>,
        swarm_plans: Arc<RwLock<HashMap<String, VersionedPlan>>>,
        file_touch: FileTouchService,
        client_debug_state: Arc<RwLock<ClientDebugState>>,
        client_connections: Arc<RwLock<HashMap<String, ClientConnectionInfo>>>,
        shutdown_signals: Arc<RwLock<HashMap<String, InterruptSignal>>>,
        soft_interrupt_queues: super::super::SessionInterruptQueues,
        event_history: Arc<RwLock<VecDeque<SwarmEvent>>>,
        event_counter: Arc<std::sync::atomic::AtomicU64>,
        swarm_event_tx: broadcast::Sender<SwarmEvent>,
    }

    impl CleanupHarness {
        async fn new(session_id: &str) -> Result<(Self, Arc<Mutex<Agent>>)> {
            let provider: Arc<dyn Provider> = Arc::new(TestProvider);
            let registry = Registry::new(provider.clone()).await;
            let mut session = Session::create_with_id(
                session_id.to_string(),
                None,
                Some("r04 disconnect fixture".to_string()),
            );
            session.mark_active();
            session.save()?;
            let agent = Arc::new(Mutex::new(Agent::new_with_session(
                provider, registry, session, None,
            )));
            let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(8);
            let (disconnect_tx, _disconnect_rx) = mpsc::unbounded_channel();
            let sessions = Arc::new(RwLock::new(HashMap::from([(
                session_id.to_string(),
                Arc::clone(&agent),
            )])));
            let swarm_members = Arc::new(RwLock::new(HashMap::from([(
                session_id.to_string(),
                test_swarm_member(session_id),
            )])));
            let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
                "swarm-r04".to_string(),
                HashSet::from([session_id.to_string()]),
            )])));
            let client_connections = Arc::new(RwLock::new(HashMap::from([(
                "conn-old".to_string(),
                ClientConnectionInfo {
                    client_id: "client-old".to_string(),
                    session_id: session_id.to_string(),
                    client_instance_id: None,
                    debug_client_id: Some("debug-old".to_string()),
                    connected_at: Instant::now(),
                    last_seen: Instant::now(),
                    is_processing: false,
                    current_tool_name: None,
                    terminal_env: Vec::new(),
                    disconnect_tx,
                },
            )])));
            Ok((
                Self {
                    sessions,
                    swarm_members,
                    swarms_by_id,
                    swarm_coordinators: Arc::new(RwLock::new(HashMap::new())),
                    swarm_plans: Arc::new(RwLock::new(HashMap::new())),
                    file_touch: FileTouchService::new(),
                    client_debug_state: Arc::new(RwLock::new(ClientDebugState::default())),
                    client_connections,
                    shutdown_signals: Arc::new(RwLock::new(HashMap::from([(
                        session_id.to_string(),
                        InterruptSignal::new(),
                    )]))),
                    soft_interrupt_queues: Arc::new(RwLock::new(HashMap::new())),
                    event_history: Arc::new(RwLock::new(VecDeque::new())),
                    event_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                    swarm_event_tx,
                },
                agent,
            ))
        }

        async fn cleanup(
            &self,
            session_id: &str,
            processing: bool,
        ) -> Result<super::CleanupClientConnectionOutcome> {
            let mut processing_task = processing.then(|| {
                tokio::spawn(async {
                    std::future::pending::<()>().await;
                })
            });
            let event_handle = tokio::spawn(async {
                std::future::pending::<()>().await;
            });
            cleanup_client_connection(
                &self.sessions,
                session_id,
                processing,
                &mut processing_task,
                event_handle,
                &self.swarm_members,
                &self.swarms_by_id,
                &self.swarm_coordinators,
                &self.swarm_plans,
                &self.file_touch,
                &self.client_debug_state,
                "debug-old",
                &self.client_connections,
                "conn-old",
                &self.shutdown_signals,
                &self.soft_interrupt_queues,
                &self.event_history,
                &self.event_counter,
                &self.swarm_event_tx,
            )
            .await
        }
    }

    fn marker_path(session_id: &str) -> Result<std::path::PathBuf> {
        Ok(crate::storage::active_pids_dir()
            .ok_or_else(|| anyhow::anyhow!("active marker dir unavailable"))?
            .join(session_id))
    }

    fn assert_live_successor_marker(session_id: &str) -> Result<()> {
        assert_eq!(
            std::fs::read_to_string(marker_path(session_id)?)?,
            std::process::id().to_string()
        );
        Ok(())
    }

    #[tokio::test]
    async fn crashed_disconnect_save_failure_retains_successor_marker_and_cleans_runtime_state()
    -> Result<()> {
        let _lock = crate::storage::lock_test_env();
        let home = tempfile::TempDir::new()?;
        let _home_guard = EnvGuard::set("JCODE_HOME", home.path());
        let session_id = "session_r04_disconnect_crash_failure";
        let _replace_guard = EnvGuard::set(
            "JCODE_TEST_REPLACE_TERMINAL_MARKER_AFTER_OBSERVE_FOR_SESSION",
            session_id,
        );
        let _fail_guard = EnvGuard::set("JCODE_TEST_FAIL_TERMINAL_SAVE_FOR_SESSION", session_id);
        let (harness, _agent) = CleanupHarness::new(session_id).await?;

        let outcome = harness.cleanup(session_id, true).await?;

        assert_eq!(
            outcome,
            super::CleanupClientConnectionOutcome {
                terminal_persistence: TerminalPersistenceOutcome::Failed,
                runtime_cleanup_completed: true,
            },
            "callers must distinguish partial runtime cleanup from terminal persistence"
        );
        assert!(matches!(
            Session::load(session_id)?.status,
            SessionStatus::Active
        ));
        assert_live_successor_marker(session_id)?;
        assert!(harness.swarm_members.read().await.get(session_id).is_none());
        assert!(
            harness
                .shutdown_signals
                .read()
                .await
                .get(session_id)
                .is_none()
        );
        assert!(harness.sessions.read().await.get(session_id).is_none());
        Ok(())
    }

    #[tokio::test]
    async fn reloading_disconnect_save_failure_retains_successor_marker_and_active_session()
    -> Result<()> {
        let _lock = crate::storage::lock_test_env();
        let home = tempfile::TempDir::new()?;
        let runtime = tempfile::TempDir::new()?;
        let _home_guard = EnvGuard::set("JCODE_HOME", home.path());
        let _runtime_guard = EnvGuard::set("JCODE_RUNTIME_DIR", runtime.path());
        crate::server::clear_reload_marker();
        crate::server::write_reload_state(
            "r04-reload",
            "test-hash",
            crate::server::ReloadPhase::Starting,
            None,
        );
        let session_id = "session_r04_disconnect_reload_failure";
        let _replace_guard = EnvGuard::set(
            "JCODE_TEST_REPLACE_TERMINAL_MARKER_AFTER_OBSERVE_FOR_SESSION",
            session_id,
        );
        let _fail_guard = EnvGuard::set("JCODE_TEST_FAIL_TERMINAL_SAVE_FOR_SESSION", session_id);
        let (harness, _agent) = CleanupHarness::new(session_id).await?;

        let outcome = harness.cleanup(session_id, true).await?;

        assert_eq!(
            outcome.terminal_persistence,
            TerminalPersistenceOutcome::Failed
        );
        assert!(outcome.runtime_cleanup_completed);
        assert!(matches!(
            Session::load(session_id)?.status,
            SessionStatus::Active
        ));
        assert_live_successor_marker(session_id)?;
        assert!(harness.swarm_members.read().await.get(session_id).is_none());
        crate::server::clear_reload_marker();
        Ok(())
    }

    #[tokio::test]
    async fn idle_closed_disconnect_persists_closed_before_preserving_successor_marker()
    -> Result<()> {
        let _lock = crate::storage::lock_test_env();
        let home = tempfile::TempDir::new()?;
        let _home_guard = EnvGuard::set("JCODE_HOME", home.path());
        let session_id = "session_r04_disconnect_idle_closed";
        let _replace_guard = EnvGuard::set(
            "JCODE_TEST_REPLACE_TERMINAL_MARKER_AFTER_OBSERVE_FOR_SESSION",
            session_id,
        );
        let (harness, _agent) = CleanupHarness::new(session_id).await?;

        let outcome = harness.cleanup(session_id, false).await?;

        assert_eq!(
            outcome.terminal_persistence,
            TerminalPersistenceOutcome::Persisted
        );
        assert!(outcome.runtime_cleanup_completed);
        assert!(matches!(
            Session::load(session_id)?.status,
            SessionStatus::Closed
        ));
        assert_live_successor_marker(session_id)?;
        assert!(harness.swarm_members.read().await.get(session_id).is_none());
        Ok(())
    }

    #[tokio::test]
    async fn disconnect_agent_lock_timeout_is_observable_without_terminal_persistence() -> Result<()>
    {
        let _lock = crate::storage::lock_test_env();
        let home = tempfile::TempDir::new()?;
        let _home_guard = EnvGuard::set("JCODE_HOME", home.path());
        TEST_AGENT_LOCK_TIMEOUT_MS.store(0, Ordering::SeqCst);
        let session_id = "session_r04_disconnect_lock_timeout";
        let (harness, agent) = CleanupHarness::new(session_id).await?;
        let held_agent_lock = agent.lock().await;

        let result = harness.cleanup(session_id, false).await;
        TEST_AGENT_LOCK_TIMEOUT_MS.store(u64::MAX, Ordering::SeqCst);
        drop(held_agent_lock);
        let outcome = result?;

        assert_eq!(
            outcome.terminal_persistence,
            TerminalPersistenceOutcome::SkippedLockTimeout
        );
        assert!(outcome.runtime_cleanup_completed);
        assert!(matches!(
            Session::load(session_id)?.status,
            SessionStatus::Active
        ));
        assert!(
            marker_path(session_id)?.exists(),
            "timeout branch must not claim or remove persisted terminal marker state"
        );
        assert!(harness.swarm_members.read().await.get(session_id).is_none());
        Ok(())
    }

    #[tokio::test]
    async fn disconnect_cleanup_outcome_contract_distinguishes_all_terminal_states() -> Result<()> {
        let _lock = crate::storage::lock_test_env();

        let home = tempfile::TempDir::new()?;
        let _home_guard = EnvGuard::set("JCODE_HOME", home.path());

        let persisted_session = "session_r04_outcome_persisted";
        let (persisted_harness, _agent) = CleanupHarness::new(persisted_session).await?;
        let persisted = persisted_harness.cleanup(persisted_session, false).await?;
        assert_eq!(
            persisted.terminal_persistence,
            TerminalPersistenceOutcome::Persisted
        );

        let successor_session = "session_r04_outcome_successor";
        let (successor_harness, _agent) = CleanupHarness::new(successor_session).await?;
        let (disconnect_tx, _disconnect_rx) = mpsc::unbounded_channel();
        successor_harness.client_connections.write().await.insert(
            "conn-successor".to_string(),
            ClientConnectionInfo {
                client_id: "client-successor".to_string(),
                session_id: successor_session.to_string(),
                client_instance_id: None,
                debug_client_id: Some("debug-successor".to_string()),
                connected_at: Instant::now(),
                last_seen: Instant::now(),
                is_processing: false,
                current_tool_name: None,
                terminal_env: Vec::new(),
                disconnect_tx,
            },
        );
        let not_required = successor_harness.cleanup(successor_session, false).await?;
        assert_eq!(
            not_required.terminal_persistence,
            TerminalPersistenceOutcome::NotRequired
        );

        let failed_session = "session_r04_outcome_failed";
        let _fail_guard =
            EnvGuard::set("JCODE_TEST_FAIL_TERMINAL_SAVE_FOR_SESSION", failed_session);
        let (failed_harness, _agent) = CleanupHarness::new(failed_session).await?;
        let failed = failed_harness.cleanup(failed_session, true).await?;
        assert_eq!(
            failed.terminal_persistence,
            TerminalPersistenceOutcome::Failed
        );

        let timeout_session = "session_r04_outcome_timeout";
        let (timeout_harness, timeout_agent) = CleanupHarness::new(timeout_session).await?;
        TEST_AGENT_LOCK_TIMEOUT_MS.store(0, Ordering::SeqCst);
        let held_agent_lock = timeout_agent.lock().await;
        let timeout_result = timeout_harness.cleanup(timeout_session, false).await;
        TEST_AGENT_LOCK_TIMEOUT_MS.store(u64::MAX, Ordering::SeqCst);
        drop(held_agent_lock);
        let timeout = timeout_result?;
        assert_eq!(
            timeout.terminal_persistence,
            TerminalPersistenceOutcome::SkippedLockTimeout
        );

        for outcome in [persisted, not_required, failed, timeout] {
            assert!(outcome.runtime_cleanup_completed);
        }
        Ok(())
    }

    #[tokio::test]
    async fn successor_connected_cleanup_reports_terminal_not_required() -> Result<()> {
        let _lock = crate::storage::lock_test_env();
        let home = tempfile::TempDir::new()?;
        let _home_guard = EnvGuard::set("JCODE_HOME", home.path());
        let session_id = "session_r04_disconnect_successor";
        let (harness, _agent) = CleanupHarness::new(session_id).await?;
        let (disconnect_tx, _disconnect_rx) = mpsc::unbounded_channel();
        harness.client_connections.write().await.insert(
            "conn-successor".to_string(),
            ClientConnectionInfo {
                client_id: "client-successor".to_string(),
                session_id: session_id.to_string(),
                client_instance_id: None,
                debug_client_id: Some("debug-successor".to_string()),
                connected_at: Instant::now(),
                last_seen: Instant::now(),
                is_processing: false,
                current_tool_name: None,
                terminal_env: Vec::new(),
                disconnect_tx,
            },
        );

        let outcome = harness.cleanup(session_id, false).await?;

        assert_eq!(
            outcome.terminal_persistence,
            TerminalPersistenceOutcome::NotRequired,
            "successor connection skips destructive terminal persistence rather than claiming it happened"
        );
        assert!(outcome.runtime_cleanup_completed);
        assert!(
            harness.sessions.read().await.get(session_id).is_some(),
            "successor branch must not remove the live session agent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn missing_agent_cleanup_reports_terminal_not_required() -> Result<()> {
        let _lock = crate::storage::lock_test_env();
        let home = tempfile::TempDir::new()?;
        let _home_guard = EnvGuard::set("JCODE_HOME", home.path());
        let session_id = "session_r04_disconnect_missing_agent";
        let (harness, _agent) = CleanupHarness::new(session_id).await?;
        harness.sessions.write().await.remove(session_id);

        let outcome = harness.cleanup(session_id, false).await?;

        assert_eq!(
            outcome.terminal_persistence,
            TerminalPersistenceOutcome::NotRequired,
            "missing agent has no terminal persistence attempt to report as persisted"
        );
        assert!(outcome.runtime_cleanup_completed);
        assert!(harness.swarm_members.read().await.get(session_id).is_none());
        Ok(())
    }

    #[test]
    fn idle_disconnect_is_closed() {
        assert_eq!(disconnect_disposition(false), DisconnectDisposition::Closed);
    }

    #[test]
    fn running_disconnect_without_reload_is_crash() {
        let _guard = crate::storage::lock_test_env();
        crate::server::clear_reload_marker();
        assert_eq!(disconnect_disposition(true), DisconnectDisposition::Crashed);
    }

    #[test]
    fn running_disconnect_during_reload_is_expected() {
        let _guard = crate::storage::lock_test_env();
        let runtime = tempfile::TempDir::new().expect("create runtime dir");
        crate::env::set_var("JCODE_RUNTIME_DIR", runtime.path());
        crate::server::clear_reload_marker();
        crate::server::write_reload_state(
            "test-request",
            "test-hash",
            crate::server::ReloadPhase::Starting,
            None,
        );
        assert_eq!(
            disconnect_disposition(true),
            DisconnectDisposition::Reloading
        );
        crate::server::clear_reload_marker();
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }

    #[test]
    fn running_disconnect_during_recent_socket_ready_reload_is_expected() {
        let _guard = crate::storage::lock_test_env();
        let runtime = tempfile::TempDir::new().expect("create runtime dir");
        crate::env::set_var("JCODE_RUNTIME_DIR", runtime.path());
        crate::server::clear_reload_marker();
        crate::server::write_reload_state(
            "test-request",
            "test-hash",
            crate::server::ReloadPhase::SocketReady,
            None,
        );
        assert_eq!(
            disconnect_disposition(true),
            DisconnectDisposition::Reloading
        );
        crate::server::clear_reload_marker();
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}
