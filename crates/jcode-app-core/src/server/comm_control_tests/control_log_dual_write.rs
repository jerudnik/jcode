// W1 step 3 (dual-write): equivalence between fold(control log) and the
// in-memory maps after real handler mutations. The sync points are
// `persist_swarm_state_for` (plan + members) and `broadcast_swarm_status`
// (members only), so any handler that goes through either must leave the
// log's fold agreeing with the maps.

use crate::server::comm_graph::{
    handle_comm_complete_node as dw_complete_node, handle_comm_seed_graph as dw_seed_graph,
};
use crate::server::control_log_sync::test_support::assert_control_log_matches_maps;
use crate::server::swarm::touch_swarm_task_progress as dw_touch_task_progress;
use crate::server::update_member_status as dw_update_member_status;

/// Drive a real mutation sequence through the actual handlers (seed ->
/// assign_next -> member status flips -> complete) and assert the fold
/// matches the maps after every step.
#[tokio::test]
async fn control_log_fold_tracks_maps_through_handler_sequence() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-dual-write";
    let coordinator = "coord-dw";
    let worker = "worker-dw";
    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let soft_interrupt_queues = Arc::new(RwLock::new(HashMap::new()));
    let client_connections = Arc::new(RwLock::new(HashMap::new()));
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (coordinator.to_string(), {
            let mut member = member(coordinator, swarm_id, "ready");
            member.role = "coordinator".to_string();
            member
        }),
        (
            worker.to_string(),
            owned_member(worker, swarm_id, "ready", coordinator),
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([coordinator.to_string(), worker.to_string()]),
    )])));
    let swarm_plans: Arc<RwLock<HashMap<String, VersionedPlan>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        coordinator.to_string(),
    )])));
    let event_history = Arc::new(RwLock::new(VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(1));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);
    let mutation_runtime = SwarmMutationRuntime::default();
    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let global_session_id = Arc::new(RwLock::new(String::new()));
    let mcp_pool = Arc::new(crate::mcp::SharedMcpPool::from_default_config());

    // 1. Seed a small graph.
    dw_seed_graph(
        301,
        coordinator.to_string(),
        None,
        vec![crate::protocol::TaskGraphNodeSpec {
            id: "n1".to_string(),
            content: "do the thing".to_string(),
            kind: None,
            depends_on: Vec::new(),
            priority: 0,
        }],
        &client_tx,
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
        &event_history,
        &event_counter,
        &swarm_event_tx,
    )
    .await;
    loop {
        match client_rx.recv().await.expect("seed response") {
            ServerEvent::Done { id } => {
                assert_eq!(id, 301);
                break;
            }
            ServerEvent::Error { message, .. } => panic!("seed failed: {message}"),
            _ => continue,
        }
    }
    assert_control_log_matches_maps(swarm_id, &swarm_members, &swarm_plans).await;

    // 2. Assign it to the worker.
    handle_comm_assign_next(
        302,
        coordinator.to_string(),
        None,
        None,
        None,
        None,
        None,
        &client_tx,
        &sessions,
        &global_session_id,
        &provider,
        &soft_interrupt_queues,
        &client_connections,
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
        &event_history,
        &event_counter,
        &swarm_event_tx,
        &mcp_pool,
        &mutation_runtime,
    )
    .await;
    loop {
        match client_rx.recv().await.expect("assign response") {
            ServerEvent::CommAssignTaskResponse { id, task_id, .. } => {
                assert_eq!(id, 302);
                assert_eq!(task_id, "n1");
                break;
            }
            ServerEvent::Error { message, .. } => panic!("assign failed: {message}"),
            _ => continue,
        }
    }
    assert_control_log_matches_maps(swarm_id, &swarm_members, &swarm_plans).await;

    // 3. Member status change flows through broadcast_swarm_status (a path
    //    that never persists a snapshot) and must still hit the log.
    dw_update_member_status(
        worker,
        "running",
        Some("working n1".to_string()),
        &swarm_members,
        &swarms_by_id,
        Some(&event_history),
        Some(&event_counter),
        Some(&swarm_event_tx),
    )
    .await;
    assert_control_log_matches_maps(swarm_id, &swarm_members, &swarm_plans).await;

    // 4. Complete the node (as the worker, who owns it after dispatch).
    dw_complete_node(
        303,
        worker.to_string(),
        "n1".to_string(),
        serde_json::json!({ "findings": "did the thing" }).to_string(),
        &client_tx,
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
        &event_history,
        &event_counter,
        &swarm_event_tx,
    )
    .await;
    loop {
        match client_rx.recv().await.expect("complete response") {
            ServerEvent::Done { id } => {
                assert_eq!(id, 303);
                break;
            }
            ServerEvent::Error { message, .. } => panic!("complete failed: {message}"),
            _ => continue,
        }
    }
    assert_control_log_matches_maps(swarm_id, &swarm_members, &swarm_plans).await;

    // 5. Heartbeat path.
    dw_touch_task_progress(
        swarm_id,
        "n1",
        Some(worker),
        Some("post-completion touch".to_string()),
        None,
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;
    assert_control_log_matches_maps(swarm_id, &swarm_members, &swarm_plans).await;

    // The log itself must contain a coherent history: the derived coordinator
    // matches, and the task went through assignment to a terminal status.
    let folded = crate::server::control_log_sync::fold_swarm_control_log(swarm_id);
    assert_eq!(folded.coordinator(), Some(coordinator));
    let task = &folded.tasks["n1"];
    assert!(
        matches!(task.status.as_str(), "completed" | "done"),
        "folded task must be terminal, got '{}'",
        task.status
    );
}
