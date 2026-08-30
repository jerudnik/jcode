use super::{handle_comm_list, handle_comm_message};
use crate::agent::Agent;
use crate::message::{Message, ToolDefinition};
use crate::protocol::{CommDeliveryMode, NotificationType, ServerEvent};
use crate::provider::{EventStream, Provider};
use crate::server::client_comm_message::resolve_comm_delivery_mode;
use crate::server::{
    ClientConnectionInfo, SessionInterruptQueues, SwarmEvent, SwarmEventState, SwarmMember,
    SwarmState, VersionedPlan, register_session_interrupt_queue,
};
use crate::tool::Registry;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, atomic::AtomicU64};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};

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
        Err(anyhow::anyhow!(
            "test provider complete should not be called in client_comm tests"
        ))
    }

    fn name(&self) -> &str {
        "test"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(TestProvider)
    }
}

async fn test_agent() -> Arc<Mutex<Agent>> {
    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let registry = Registry::new(provider.clone()).await;
    Arc::new(Mutex::new(Agent::new(provider, registry)))
}

fn test_swarm_state(
    members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
) -> SwarmState {
    SwarmState {
        members: Arc::clone(members),
        swarms_by_id: Arc::clone(swarms_by_id),
        plans: Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new())),
        coordinators: Arc::new(RwLock::new(HashMap::new())),
    }
}

fn test_swarm_events(
    history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    counter: &Arc<AtomicU64>,
    tx: &broadcast::Sender<SwarmEvent>,
) -> SwarmEventState {
    SwarmEventState {
        history: Arc::clone(history),
        counter: Arc::clone(counter),
        tx: tx.clone(),
    }
}

#[tokio::test]

async fn comm_message_with_wake_queues_soft_interrupt_for_busy_connected_session() {
    let sender = test_agent().await;
    let target = test_agent().await;

    let sender_id = sender.lock().await.session_id().to_string();
    let target_id = target.lock().await.session_id().to_string();
    let target_queue = target.lock().await.soft_interrupt_queue();

    let sessions = Arc::new(RwLock::new(HashMap::from([
        (sender_id.clone(), sender.clone()),
        (target_id.clone(), target.clone()),
    ])));
    let soft_interrupt_queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));
    crate::server::register_session_interrupt_queue(
        &soft_interrupt_queues,
        &target_id,
        target_queue.clone(),
    )
    .await;

    let (sender_event_tx, _sender_event_rx) = mpsc::unbounded_channel();
    let (target_event_tx, mut target_event_rx) = mpsc::unbounded_channel();
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let swarm_id = "swarm-test".to_string();
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (
            sender_id.clone(),
            SwarmMember {
                session_id: sender_id.clone(),
                event_tx: sender_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some("falcon".to_string()),
                report_back_to_session_id: None,
                initial_prompt_delivered: None,
                latest_completion_report: None,
                role: "coordinator".to_string(),
                joined_at: Instant::now(),
                last_status_change: Instant::now(),
                is_headless: false,
                output_tail: None,
                todo_progress: None,
                todo_items: Vec::new(),
                runtime: crate::protocol::SwarmMemberRuntime::default(),
                task_label: None,
                subagent_type: None,
            },
        ),
        (
            target_id.clone(),
            SwarmMember {
                session_id: target_id.clone(),
                event_tx: target_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some("bear".to_string()),
                report_back_to_session_id: None,
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
                task_label: None,
                subagent_type: None,
            },
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        HashSet::from([sender_id.clone(), target_id.clone()]),
    )])));
    let event_history: Arc<RwLock<std::collections::VecDeque<SwarmEvent>>> =
        Arc::new(RwLock::new(std::collections::VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _) = broadcast::channel(16);
    let client_connections = Arc::new(RwLock::new(HashMap::from([(
        "client-1".to_string(),
        ClientConnectionInfo {
            client_id: "client-1".to_string(),
            session_id: target_id.clone(),
            client_instance_id: None,
            debug_client_id: None,
            connected_at: Instant::now(),
            last_seen: Instant::now(),
            is_processing: false,
            current_tool_name: None,
            terminal_env: Vec::new(),
            disconnect_tx: mpsc::unbounded_channel().0,
        },
    )])));

    let _busy_guard = target.lock().await;

    tokio::time::timeout(
        Duration::from_secs(2),
        handle_comm_message(
            1,
            sender_id.clone(),
            "hello now".to_string(),
            Some(target_id.clone()),
            Some(CommDeliveryMode::Wake),
            None,
            None,
            &client_event_tx,
            &sessions,
            &soft_interrupt_queues,
            &test_swarm_state(&swarm_members, &swarms_by_id),
            &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
            &client_connections,
        ),
    )
    .await
    .expect("comm message should not deadlock");

    match target_event_rx.recv().await.expect("target notification") {
        ServerEvent::Notification {
            from_session,
            from_name,
            notification_type,
            message,
        } => {
            assert_eq!(from_session, sender_id);
            assert_eq!(from_name.as_deref(), Some("falcon"));
            match notification_type {
                NotificationType::Message { scope, .. } => {
                    assert_eq!(scope.as_deref(), Some("dm"));
                }
                other => panic!("unexpected notification type: {:?}", other),
            }
            assert_eq!(message, "DM from falcon: hello now");
        }
        other => panic!("unexpected event: {:?}", other),
    }

    match client_event_rx.recv().await.expect("done event") {
        ServerEvent::Done { id } => assert_eq!(id, 1),
        other => panic!("unexpected client event: {:?}", other),
    }

    let pending = target_queue.lock().expect("target queue lock");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].content, "DM from falcon: hello now");
    assert_eq!(
        pending[0].source,
        jcode_agent_runtime::SoftInterruptSource::System
    );
}

#[tokio::test]
async fn comm_list_includes_member_status_and_detail() {
    let requester = test_agent().await;
    let peer = test_agent().await;

    let requester_id = requester.lock().await.session_id().to_string();
    let peer_id = peer.lock().await.session_id().to_string();
    let swarm_id = "swarm-test".to_string();

    let (requester_event_tx, _requester_event_rx) = mpsc::unbounded_channel();
    let (peer_event_tx, _peer_event_rx) = mpsc::unbounded_channel();
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (
            requester_id.clone(),
            SwarmMember {
                session_id: requester_id.clone(),
                event_tx: requester_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some("falcon".to_string()),
                report_back_to_session_id: None,
                initial_prompt_delivered: None,
                latest_completion_report: None,
                role: "coordinator".to_string(),
                joined_at: Instant::now(),
                last_status_change: Instant::now(),
                is_headless: false,
                output_tail: None,
                todo_progress: None,
                todo_items: Vec::new(),
                runtime: crate::protocol::SwarmMemberRuntime::default(),
                task_label: None,
                subagent_type: None,
            },
        ),
        (
            peer_id.clone(),
            SwarmMember {
                session_id: peer_id.clone(),
                event_tx: peer_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "running".to_string(),
                lifecycle: Default::default(),
                detail: Some("working on tests".to_string()),
                friendly_name: Some("bear".to_string()),
                report_back_to_session_id: None,
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
                task_label: None,
                subagent_type: None,
            },
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id,
        HashSet::from([requester_id.clone(), peer_id.clone()]),
    )])));
    let file_touch = crate::server::FileTouchService::new();
    let sessions = Arc::new(RwLock::new(HashMap::from([
        (requester_id.clone(), requester.clone()),
        (peer_id.clone(), peer.clone()),
    ])));
    let client_connections = Arc::new(RwLock::new(HashMap::new()));

    handle_comm_list(
        1,
        requester_id,
        &client_event_tx,
        &swarm_members,
        &swarms_by_id,
        &file_touch,
        &sessions,
        &client_connections,
    )
    .await;

    match client_event_rx.recv().await.expect("comm list response") {
        ServerEvent::CommMembers { id, members } => {
            assert_eq!(id, 1);
            let peer = members
                .into_iter()
                .find(|member| member.friendly_name.as_deref() == Some("bear"))
                .expect("peer entry present");
            assert_eq!(peer.status.as_deref(), Some("running"));
            assert_eq!(peer.detail.as_deref(), Some("working on tests"));
        }
        other => panic!("unexpected response: {other:?}"),
    }
}

#[tokio::test]
async fn comm_message_accepts_friendly_name_dm_target() {
    let sender = test_agent().await;
    let target = test_agent().await;

    let sender_id = sender.lock().await.session_id().to_string();
    let target_id = target.lock().await.session_id().to_string();
    let swarm_id = "swarm-test".to_string();

    let sessions = Arc::new(RwLock::new(HashMap::from([
        (sender_id.clone(), sender.clone()),
        (target_id.clone(), target.clone()),
    ])));
    let soft_interrupt_queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));

    let (sender_event_tx, _sender_event_rx) = mpsc::unbounded_channel();
    let (target_event_tx, mut target_event_rx) = mpsc::unbounded_channel();
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (
            sender_id.clone(),
            SwarmMember {
                session_id: sender_id.clone(),
                event_tx: sender_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some("falcon".to_string()),
                report_back_to_session_id: None,
                initial_prompt_delivered: None,
                latest_completion_report: None,
                role: "coordinator".to_string(),
                joined_at: Instant::now(),
                last_status_change: Instant::now(),
                is_headless: false,
                output_tail: None,
                todo_progress: None,
                todo_items: Vec::new(),
                runtime: crate::protocol::SwarmMemberRuntime::default(),
                task_label: None,
                subagent_type: None,
            },
        ),
        (
            target_id.clone(),
            SwarmMember {
                session_id: target_id.clone(),
                event_tx: target_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some("bear".to_string()),
                report_back_to_session_id: None,
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
                task_label: None,
                subagent_type: None,
            },
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        HashSet::from([sender_id.clone(), target_id.clone()]),
    )])));
    let event_history: Arc<RwLock<std::collections::VecDeque<SwarmEvent>>> =
        Arc::new(RwLock::new(std::collections::VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _) = broadcast::channel(16);
    let client_connections = Arc::new(RwLock::new(HashMap::new()));

    handle_comm_message(
        1,
        sender_id.clone(),
        "hello bear".to_string(),
        Some("bear".to_string()),
        Some(CommDeliveryMode::Notify),
        None,
        None,
        &client_event_tx,
        &sessions,
        &soft_interrupt_queues,
        &test_swarm_state(&swarm_members, &swarms_by_id),
        &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
        &client_connections,
    )
    .await;

    match target_event_rx.recv().await.expect("target notification") {
        ServerEvent::Notification {
            from_session,
            from_name,
            notification_type,
            message,
        } => {
            assert_eq!(from_session, sender_id);
            assert_eq!(from_name.as_deref(), Some("falcon"));
            match notification_type {
                NotificationType::Message { scope, .. } => {
                    assert_eq!(scope.as_deref(), Some("dm"));
                }
                other => panic!("unexpected notification type: {:?}", other),
            }
            assert_eq!(message, "DM from falcon: hello bear");
        }
        other => panic!("unexpected event: {:?}", other),
    }

    match client_event_rx.recv().await.expect("done event") {
        ServerEvent::Done { id } => assert_eq!(id, 1),
        other => panic!("unexpected client event: {:?}", other),
    }
}

#[tokio::test]
async fn comm_message_rejects_ambiguous_friendly_name_dm_target() {
    let sender = test_agent().await;
    let target_one = test_agent().await;
    let target_two = test_agent().await;

    let sender_id = sender.lock().await.session_id().to_string();
    let target_one_id = target_one.lock().await.session_id().to_string();
    let target_two_id = target_two.lock().await.session_id().to_string();
    let swarm_id = "swarm-test".to_string();

    let sessions = Arc::new(RwLock::new(HashMap::from([
        (sender_id.clone(), sender.clone()),
        (target_one_id.clone(), target_one.clone()),
        (target_two_id.clone(), target_two.clone()),
    ])));
    let soft_interrupt_queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));

    let (sender_event_tx, _sender_event_rx) = mpsc::unbounded_channel();
    let (target_one_event_tx, _target_one_event_rx) = mpsc::unbounded_channel();
    let (target_two_event_tx, _target_two_event_rx) = mpsc::unbounded_channel();
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (
            sender_id.clone(),
            SwarmMember {
                session_id: sender_id.clone(),
                event_tx: sender_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some("falcon".to_string()),
                report_back_to_session_id: None,
                initial_prompt_delivered: None,
                latest_completion_report: None,
                role: "coordinator".to_string(),
                joined_at: Instant::now(),
                last_status_change: Instant::now(),
                is_headless: false,
                output_tail: None,
                todo_progress: None,
                todo_items: Vec::new(),
                runtime: crate::protocol::SwarmMemberRuntime::default(),
                task_label: None,
                subagent_type: None,
            },
        ),
        (
            target_one_id.clone(),
            SwarmMember {
                session_id: target_one_id.clone(),
                event_tx: target_one_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some("bear".to_string()),
                report_back_to_session_id: None,
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
                task_label: None,
                subagent_type: None,
            },
        ),
        (
            target_two_id.clone(),
            SwarmMember {
                session_id: target_two_id.clone(),
                event_tx: target_two_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some("bear".to_string()),
                report_back_to_session_id: None,
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
                task_label: None,
                subagent_type: None,
            },
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        HashSet::from([
            sender_id.clone(),
            target_one_id.clone(),
            target_two_id.clone(),
        ]),
    )])));
    let event_history: Arc<RwLock<std::collections::VecDeque<SwarmEvent>>> =
        Arc::new(RwLock::new(std::collections::VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _) = broadcast::channel(16);
    let client_connections = Arc::new(RwLock::new(HashMap::new()));

    handle_comm_message(
        1,
        sender_id,
        "hello bears".to_string(),
        Some("bear".to_string()),
        None,
        None,
        None,
        &client_event_tx,
        &sessions,
        &soft_interrupt_queues,
        &test_swarm_state(&swarm_members, &swarms_by_id),
        &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
        &client_connections,
    )
    .await;

    match client_event_rx.recv().await.expect("error event") {
        ServerEvent::Error { id, message, .. } => {
            assert_eq!(id, 1);
            assert!(message.contains("ambiguous in swarm"), "{message}");
            assert!(message.contains("Use an exact session id"), "{message}");
            assert!(message.contains(&target_one_id), "{message}");
            assert!(message.contains(&target_two_id), "{message}");
            assert!(message.contains("bear ["), "{message}");
        }
        other => panic!("unexpected client event: {:?}", other),
    }
}

/// Broadcasts are subtree-scoped: a non-coordinator sender reaches only the
/// agents it (transitively) spawned, never unrelated peers, while a
/// coordinator retains whole-swarm reach.
#[tokio::test]
async fn comm_broadcast_reaches_only_senders_spawned_subtree() {
    fn member(
        session_id: &str,
        role: &str,
        report_back_to: Option<&str>,
        swarm_id: &str,
    ) -> (SwarmMember, mpsc::UnboundedReceiver<ServerEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        (
            SwarmMember {
                session_id: session_id.to_string(),
                event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.to_string()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                friendly_name: Some(session_id.to_string()),
                report_back_to_session_id: report_back_to.map(str::to_string),
                initial_prompt_delivered: None,
                latest_completion_report: None,
                role: role.to_string(),
                joined_at: Instant::now(),
                last_status_change: Instant::now(),
                is_headless: true,
                output_tail: None,
                todo_progress: None,
                todo_items: Vec::new(),
                runtime: crate::protocol::SwarmMemberRuntime::default(),
                task_label: None,
                subagent_type: None,
            },
            event_rx,
        )
    }

    let swarm_id = "swarm-subtree";
    // Tree: coord (coordinator, root)
    //       sender (root peer) -> child -> grandchild
    //       outsider (root peer, unrelated)
    let (coord, mut coord_rx) = member("coord", "coordinator", None, swarm_id);
    let (sender, _sender_rx) = member("sender", "agent", None, swarm_id);
    let (child, mut child_rx) = member("child", "agent", Some("sender"), swarm_id);
    let (grandchild, mut grandchild_rx) = member("grandchild", "agent", Some("child"), swarm_id);
    let (outsider, mut outsider_rx) = member("outsider", "agent", None, swarm_id);

    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        ("coord".to_string(), coord),
        ("sender".to_string(), sender),
        ("child".to_string(), child),
        ("grandchild".to_string(), grandchild),
        ("outsider".to_string(), outsider),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([
            "coord".to_string(),
            "sender".to_string(),
            "child".to_string(),
            "grandchild".to_string(),
            "outsider".to_string(),
        ]),
    )])));
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let soft_interrupt_queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));
    let event_history: Arc<RwLock<std::collections::VecDeque<SwarmEvent>>> =
        Arc::new(RwLock::new(std::collections::VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _) = broadcast::channel(16);
    let client_connections = Arc::new(RwLock::new(HashMap::new()));
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    handle_comm_message(
        1,
        "sender".to_string(),
        "subtree update".to_string(),
        None,
        None,
        None,
        None,
        &client_event_tx,
        &sessions,
        &soft_interrupt_queues,
        &test_swarm_state(&swarm_members, &swarms_by_id),
        &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
        &client_connections,
    )
    .await;

    match client_event_rx.recv().await.expect("done event") {
        ServerEvent::Done { id } => assert_eq!(id, 1),
        other => panic!("unexpected client event: {:?}", other),
    }

    // Direct child and transitive grandchild both receive the broadcast.
    assert!(matches!(
        child_rx.try_recv(),
        Ok(ServerEvent::Notification { .. })
    ));
    assert!(matches!(
        grandchild_rx.try_recv(),
        Ok(ServerEvent::Notification { .. })
    ));
    // Unrelated root peers and the coordinator do not.
    assert!(outsider_rx.try_recv().is_err());
    assert!(coord_rx.try_recv().is_err());

    // Coordinator broadcast still reaches the whole swarm.
    handle_comm_message(
        2,
        "coord".to_string(),
        "swarm-wide notice".to_string(),
        None,
        None,
        None,
        None,
        &client_event_tx,
        &sessions,
        &soft_interrupt_queues,
        &test_swarm_state(&swarm_members, &swarms_by_id),
        &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
        &client_connections,
    )
    .await;
    match client_event_rx.recv().await.expect("done event") {
        ServerEvent::Done { id } => assert_eq!(id, 2),
        other => panic!("unexpected client event: {:?}", other),
    }
    assert!(matches!(
        outsider_rx.try_recv(),
        Ok(ServerEvent::Notification { .. })
    ));
    assert!(matches!(
        child_rx.try_recv(),
        Ok(ServerEvent::Notification { .. })
    ));
}

/// W3c (orchestration-hardening): a Wake DM parked as a soft interrupt while
/// the target was transiently busy must still be DELIVERED once the target is
/// idle again. Today the queue is only drained by mid-turn injection points,
/// so a message parked during a lock-contention window (no real turn running)
/// sits undelivered forever and the worker idles with a pending assignment.
/// Riptide's nudge pattern: queued messages are delivered on ready_for_input.
#[tokio::test]
async fn comm_message_wake_delivers_parked_interrupt_once_target_is_idle() {
    // This test runs a REAL delivery turn, which persists the target's
    // session. Serialize with env-mutating tests (JCODE_HOME temp dirs +
    // empty-sessions-dir asserts) so the save cannot land in their sandbox.
    let _env_lock = crate::storage::lock_test_env();
    let sender = test_agent().await;

    // Target agent with a mock stream so the delivery turn can actually run.
    #[derive(Default, Clone)]
    struct NudgeStreamProvider {
        responses: Arc<std::sync::Mutex<Vec<Vec<crate::message::StreamEvent>>>>,
    }
    #[async_trait]
    impl Provider for NudgeStreamProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
            _system: &str,
            _resume_session_id: Option<&str>,
        ) -> Result<EventStream> {
            let mut guard = self.responses.lock().expect("responses lock");
            let response = if guard.is_empty() {
                vec![crate::message::StreamEvent::MessageEnd { stop_reason: None }]
            } else {
                guard.remove(0)
            };
            drop(guard);
            Ok(Box::pin(futures::stream::iter(
                response.into_iter().map(Ok),
            )))
        }
        fn name(&self) -> &str {
            "test-nudge"
        }
        fn fork(&self) -> Arc<dyn Provider> {
            Arc::new(self.clone())
        }
    }
    let streaming = NudgeStreamProvider::default();
    streaming
        .responses
        .lock()
        .expect("responses lock")
        .push(vec![
            crate::message::StreamEvent::TextDelta("Parked DM processed.".to_string()),
            crate::message::StreamEvent::MessageEnd { stop_reason: None },
        ]);
    let streaming_dyn: Arc<dyn Provider> = Arc::new(streaming);
    let registry = Registry::new(streaming_dyn.clone()).await;
    let target = Arc::new(Mutex::new(Agent::new(streaming_dyn, registry)));

    let sender_id = sender.lock().await.session_id().to_string();
    let target_id = target.lock().await.session_id().to_string();
    let target_queue = target.lock().await.soft_interrupt_queue();

    let sessions = Arc::new(RwLock::new(HashMap::from([
        (sender_id.clone(), sender.clone()),
        (target_id.clone(), target.clone()),
    ])));
    let soft_interrupt_queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));
    crate::server::register_session_interrupt_queue(
        &soft_interrupt_queues,
        &target_id,
        target_queue.clone(),
    )
    .await;

    let (sender_event_tx, _sender_event_rx) = mpsc::unbounded_channel();
    let (target_event_tx, mut target_event_rx) = mpsc::unbounded_channel();
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let swarm_id = "swarm-nudge".to_string();
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (
            sender_id.clone(),
            SwarmMember {
                session_id: sender_id.clone(),
                event_tx: sender_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                task_label: None,
                subagent_type: None,
                friendly_name: Some("falcon".to_string()),
                report_back_to_session_id: None,
                initial_prompt_delivered: None,
                latest_completion_report: None,
                role: "coordinator".to_string(),
                joined_at: Instant::now(),
                last_status_change: Instant::now(),
                is_headless: false,
                output_tail: None,
                todo_progress: None,
                todo_items: Vec::new(),
                runtime: crate::protocol::SwarmMemberRuntime::default(),
            },
        ),
        (
            target_id.clone(),
            SwarmMember {
                session_id: target_id.clone(),
                event_tx: target_event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.clone()),
                swarm_enabled: true,
                status: "ready".to_string(),
                lifecycle: Default::default(),
                detail: None,
                task_label: None,
                subagent_type: None,
                friendly_name: Some("bear".to_string()),
                report_back_to_session_id: None,
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
            },
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        HashSet::from([sender_id.clone(), target_id.clone()]),
    )])));
    let event_history: Arc<RwLock<std::collections::VecDeque<SwarmEvent>>> =
        Arc::new(RwLock::new(std::collections::VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _) = broadcast::channel(16);
    let client_connections = Arc::new(RwLock::new(HashMap::new()));

    // Transient busy window: the lock is held (status query, summary, etc.)
    // but NO turn is running, so no injection point will ever drain the queue.
    let busy_guard = target.lock().await;

    tokio::time::timeout(
        Duration::from_secs(2),
        handle_comm_message(
            1,
            sender_id.clone(),
            "urgent assignment: fix the build".to_string(),
            Some(target_id.clone()),
            Some(CommDeliveryMode::Wake),
            None,
            None,
            &client_event_tx,
            &sessions,
            &soft_interrupt_queues,
            &test_swarm_state(&swarm_members, &swarms_by_id),
            &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
            &client_connections,
        ),
    )
    .await
    .expect("comm message should not deadlock");

    // Message parked while busy.
    assert_eq!(
        target_queue.lock().expect("queue lock").len(),
        1,
        "wake to a busy target should park the message"
    );

    // Busy window ends with NO turn having run: the parked message must now
    // be delivered rather than rot in the queue.
    drop(busy_guard);

    let delivered = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            {
                let guard = target.lock().await;
                let has_dm_turn = guard.messages().iter().any(|message| {
                    message.role == crate::message::Role::User
                        && message
                            .content_preview()
                            .contains("urgent assignment: fix the build")
                });
                if has_dm_turn {
                    return true;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or(false);

    assert!(
        delivered,
        "W3c reproduced: parked wake DM was never delivered after the target \
         became idle (queue len now {})",
        target_queue.lock().expect("queue lock").len()
    );

    // Drain ancillary channels so they do not look like leaks.
    let _ = client_event_rx.try_recv();
    let _ = target_event_rx.try_recv();
}

/// `delivery: notify` must reach a headless recipient's model at its next turn
/// boundary. The `Interrupt` arm remains the control: same fixture and queue,
/// with only the delivery mode changed.
#[tokio::test]
async fn comm_message_notify_to_headless_member_queues_for_next_turn() {
    let sender = test_agent().await;
    let target = test_agent().await;

    let sender_id = sender.lock().await.session_id().to_string();
    let target_id = target.lock().await.session_id().to_string();
    let target_queue = target.lock().await.soft_interrupt_queue();
    let swarm_id = "swarm-notify".to_string();

    let sessions = Arc::new(RwLock::new(HashMap::from([
        (sender_id.clone(), sender.clone()),
        (target_id.clone(), target.clone()),
    ])));

    let (sender_event_tx, _sender_event_rx) = mpsc::unbounded_channel();
    // A headless member's event_tx is drained by a discard loop. Model that
    // faithfully: the send succeeds, and nothing observes the event.
    let (target_event_tx, mut target_event_rx) = mpsc::unbounded_channel();

    let member = |session_id: &str,
                  event_tx: mpsc::UnboundedSender<ServerEvent>,
                  name: &str,
                  role: &str,
                  is_headless: bool| SwarmMember {
        session_id: session_id.to_string(),
        event_tx,
        event_txs: HashMap::new(),
        working_dir: None,
        swarm_id: Some(swarm_id.clone()),
        swarm_enabled: true,
        status: "ready".to_string(),
        lifecycle: Default::default(),
        detail: None,
        friendly_name: Some(name.to_string()),
        report_back_to_session_id: None,
        initial_prompt_delivered: None,
        latest_completion_report: None,
        role: role.to_string(),
        joined_at: Instant::now(),
        last_status_change: Instant::now(),
        is_headless,
        output_tail: None,
        todo_progress: None,
        todo_items: Vec::new(),
        runtime: crate::protocol::SwarmMemberRuntime::default(),
        task_label: None,
        subagent_type: None,
    };

    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (
            sender_id.clone(),
            member(&sender_id, sender_event_tx, "falcon", "coordinator", false),
        ),
        (
            target_id.clone(),
            member(&target_id, target_event_tx, "bear", "agent", true),
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        HashSet::from([sender_id.clone(), target_id.clone()]),
    )])));
    let event_history: Arc<RwLock<std::collections::VecDeque<SwarmEvent>>> =
        Arc::new(RwLock::new(std::collections::VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _) = broadcast::channel(16);
    let client_connections = Arc::new(RwLock::new(HashMap::new()));
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));
    register_session_interrupt_queue(&queues, &target_id, target_queue.clone()).await;

    // Control arm: Interrupt delivery. Same fixture, same body.
    handle_comm_message(
        1,
        sender_id.clone(),
        "control body".to_string(),
        Some("bear".to_string()),
        Some(CommDeliveryMode::Interrupt),
        None,
        None,
        &client_event_tx,
        &sessions,
        &queues,
        &test_swarm_state(&swarm_members, &swarms_by_id),
        &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
        &client_connections,
    )
    .await;
    assert!(matches!(
        client_event_rx.recv().await,
        Some(ServerEvent::Done { id: 1 })
    ));

    {
        let pending = target_queue.lock().expect("queue lock");
        assert_eq!(
            pending.len(),
            1,
            "control arm: interrupt delivery must reach the recipient's queue"
        );
        assert!(
            pending[0].content.contains("control body"),
            "control arm: queued body was {:?}",
            pending[0].content
        );
    }
    target_queue.lock().expect("queue lock").clear();
    // Drain the control arm's UI event so the notify arm reads its own.
    assert!(matches!(
        target_event_rx.try_recv(),
        Ok(ServerEvent::Notification { .. })
    ));

    // Treatment arm: Notify delivery, identical in every other respect.
    handle_comm_message(
        2,
        sender_id.clone(),
        "notify body".to_string(),
        Some("bear".to_string()),
        Some(CommDeliveryMode::Notify),
        None,
        None,
        &client_event_tx,
        &sessions,
        &queues,
        &test_swarm_state(&swarm_members, &swarms_by_id),
        &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
        &client_connections,
    )
    .await;

    // The send reports success ...
    assert!(matches!(
        client_event_rx.recv().await,
        Some(ServerEvent::Done { id: 2 })
    ));
    // ... and a notification really was emitted on the session channel, which
    // for a headless member is the discard loop.
    match target_event_rx.try_recv() {
        Ok(ServerEvent::Notification { message, .. }) => {
            assert!(
                message.contains("notify body"),
                "unexpected notification body: {message:?}"
            );
        }
        other => panic!("expected a UI notification, got {other:?}"),
    }

    // ... and the body is queued for the recipient's next turn boundary.
    let pending = target_queue.lock().expect("queue lock");
    assert_eq!(
        pending.len(),
        1,
        "notify delivery must reach the recipient's queue, but it held {:?}",
        pending
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        pending[0].content.contains("notify body"),
        "notify delivery queued the wrong body: {:?}",
        pending[0].content
    );
}

/// `resolve_comm_delivery_mode` decides what a caller who named neither
/// `delivery` nor `wake` actually gets. Every `swarm` tool action passes both
/// straight through as `Option`, so these defaults are what ships whenever a
/// caller does not opt in.
///
/// Pinned as a unit because DMs default to waking an idle recipient while
/// broadcasts default to queueing for the next turn boundary.
#[test]
fn resolve_comm_delivery_mode_defaults_split_by_scope() {
    assert_eq!(
        resolve_comm_delivery_mode("dm", None, None),
        CommDeliveryMode::Wake,
        "a DM with no delivery and no wake must default to a mode the recipient's model reads"
    );
    assert_eq!(
        resolve_comm_delivery_mode("broadcast", None, None),
        CommDeliveryMode::Notify,
        "a broadcast with no delivery and no wake defaults to queued Notify delivery"
    );
    // An explicit mode always wins, and `wake: true` upgrades a broadcast.
    assert_eq!(
        resolve_comm_delivery_mode("dm", Some(CommDeliveryMode::Notify), None),
        CommDeliveryMode::Notify,
        "an explicit delivery mode must win over the scope default"
    );
    assert_eq!(
        resolve_comm_delivery_mode("broadcast", None, Some(true)),
        CommDeliveryMode::Wake,
        "wake: true must upgrade a broadcast off the Notify default"
    );
}

/// A default `swarm broadcast` names no delivery mode and no wake flag, so it
/// resolves to `Notify` and must be queued for a headless recipient's next turn.
/// The explicit `Interrupt` arm proves the fixture can observe queue delivery.
#[tokio::test]
async fn comm_message_default_broadcast_to_headless_member_queues_for_next_turn() {
    let sender = test_agent().await;
    let target = test_agent().await;

    let sender_id = sender.lock().await.session_id().to_string();
    let target_id = target.lock().await.session_id().to_string();
    let target_queue = target.lock().await.soft_interrupt_queue();
    let swarm_id = "swarm-default-broadcast".to_string();

    let sessions = Arc::new(RwLock::new(HashMap::from([
        (sender_id.clone(), sender.clone()),
        (target_id.clone(), target.clone()),
    ])));

    let (sender_event_tx, _sender_event_rx) = mpsc::unbounded_channel();
    // A headless member's event_tx is drained by a discard loop. Keep a receiver
    // here only to prove event-channel acceptance is not model delivery.
    let (target_event_tx, mut target_event_rx) = mpsc::unbounded_channel();

    let member = |session_id: &str,
                  event_tx: mpsc::UnboundedSender<ServerEvent>,
                  name: &str,
                  role: &str,
                  is_headless: bool| SwarmMember {
        session_id: session_id.to_string(),
        event_tx,
        event_txs: HashMap::new(),
        working_dir: None,
        swarm_id: Some(swarm_id.clone()),
        swarm_enabled: true,
        status: "ready".to_string(),
        lifecycle: Default::default(),
        detail: None,
        friendly_name: Some(name.to_string()),
        report_back_to_session_id: None,
        initial_prompt_delivered: None,
        latest_completion_report: None,
        role: role.to_string(),
        joined_at: Instant::now(),
        last_status_change: Instant::now(),
        is_headless,
        output_tail: None,
        todo_progress: None,
        todo_items: Vec::new(),
        runtime: crate::protocol::SwarmMemberRuntime::default(),
        task_label: None,
        subagent_type: None,
    };

    // The sender is the coordinator, so its broadcast reaches the whole swarm
    // rather than a spawned subtree it does not have.
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (
            sender_id.clone(),
            member(&sender_id, sender_event_tx, "falcon", "coordinator", false),
        ),
        (
            target_id.clone(),
            member(&target_id, target_event_tx, "bear", "agent", true),
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        HashSet::from([sender_id.clone(), target_id.clone()]),
    )])));
    let event_history: Arc<RwLock<std::collections::VecDeque<SwarmEvent>>> =
        Arc::new(RwLock::new(std::collections::VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, _) = broadcast::channel(16);
    let client_connections = Arc::new(RwLock::new(HashMap::new()));
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();

    let queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));
    register_session_interrupt_queue(&queues, &target_id, target_queue.clone()).await;

    // Control arm: an explicit Interrupt broadcast. Same fixture, same body,
    // no target -- only the delivery mode differs from the treatment.
    handle_comm_message(
        1,
        sender_id.clone(),
        "control body".to_string(),
        None,
        Some(CommDeliveryMode::Interrupt),
        None,
        None,
        &client_event_tx,
        &sessions,
        &queues,
        &test_swarm_state(&swarm_members, &swarms_by_id),
        &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
        &client_connections,
    )
    .await;
    assert!(matches!(
        client_event_rx.recv().await,
        Some(ServerEvent::Done { id: 1 })
    ));

    {
        let pending = target_queue.lock().expect("queue lock");
        assert_eq!(
            pending.len(),
            1,
            "control arm: an Interrupt broadcast must reach the recipient's queue"
        );
        assert!(
            pending[0].content.contains("control body"),
            "control arm: queued body was {:?}",
            pending[0].content
        );
    }
    target_queue.lock().expect("queue lock").clear();
    // Drain the control arm's UI event so the treatment arm reads its own.
    assert!(matches!(
        target_event_rx.try_recv(),
        Ok(ServerEvent::Notification { .. })
    ));

    // Treatment arm: the default a `swarm broadcast` actually sends -- no
    // delivery mode, no wake flag.
    handle_comm_message(
        2,
        sender_id.clone(),
        "default broadcast body".to_string(),
        None,
        None,
        None,
        None,
        &client_event_tx,
        &sessions,
        &queues,
        &test_swarm_state(&swarm_members, &swarms_by_id),
        &test_swarm_events(&event_history, &event_counter, &swarm_event_tx),
        &client_connections,
    )
    .await;

    // The send reports success ...
    assert!(matches!(
        client_event_rx.recv().await,
        Some(ServerEvent::Done { id: 2 })
    ));
    // ... and the recipient's event channel accepts it ...
    match target_event_rx.try_recv() {
        Ok(ServerEvent::Notification { message, .. }) => {
            assert!(
                message.contains("default broadcast body"),
                "unexpected notification body: {message:?}"
            );
        }
        other => panic!("expected a UI notification, got {other:?}"),
    }

    // ... and the body is independently queued for the recipient model's next
    // turn boundary.
    let pending = target_queue.lock().expect("queue lock");
    assert_eq!(
        pending.len(),
        1,
        "a default broadcast must reach the recipient's queue, but it held {:?}",
        pending
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        pending[0].content.contains("default broadcast body"),
        "default broadcast queued the wrong body: {:?}",
        pending[0].content
    );
}
