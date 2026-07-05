// Failure scoreboard: red-first reproductions of the audited orchestration
// failures F1-F4 (orchestration-hardening proposal, code-audit-failures.md).
// Each test encodes the DESIRED behavior and fails against the buggy code.
// F5 lives in assign_friendly_name.rs.

use crate::server::comm_graph::handle_comm_complete_node as scoreboard_complete_node;
use crate::server::comm_sync::{CommResyncPlanContext, handle_comm_resync_plan};

/// F1 - run_plan driver stall: a queued task whose `assigned_to` references a
/// session that is no longer in the swarm is permanently unrunnable, because
/// `next_unassigned_runnable_item_id` requires `assigned_to.is_none()`
/// (jcode-plan/lib.rs) and nothing ever reclaims stale assignments. Live
/// symptom: "run_plan stalled: runnable task(s) could not be assigned" while
/// a fresh worker sits idle.
///
/// Desired: assign_next reclaims the stale assignment and binds the task to a
/// live worker instead of erroring.
#[tokio::test]
async fn f1_assign_next_reclaims_task_from_departed_assignee() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-f1-stale";
    let requester = "coord-f1";
    let live_worker = "worker-f1-live";
    let ghost = "worker-f1-ghost"; // assigned in the plan, but NOT a swarm member
    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let soft_interrupt_queues = Arc::new(RwLock::new(HashMap::new()));
    let client_connections = Arc::new(RwLock::new(HashMap::new()));
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (requester.to_string(), {
            let mut member = member(requester, swarm_id, "ready");
            member.role = "coordinator".to_string();
            member
        }),
        (
            live_worker.to_string(),
            owned_member(live_worker, swarm_id, "ready", requester),
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), live_worker.to_string()]),
    )])));
    let mut stuck = plan_item("stuck", "queued", "high", &[]);
    stuck.assigned_to = Some(ghost.to_string());
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![stuck],
            version: 1,
            participants: HashSet::from([requester.to_string(), live_worker.to_string()]),
            task_progress: HashMap::new(),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        requester.to_string(),
    )])));
    let event_history = Arc::new(RwLock::new(VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(1));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);
    let mutation_runtime = SwarmMutationRuntime::default();
    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let global_session_id = Arc::new(RwLock::new(String::new()));
    let mcp_pool = Arc::new(crate::mcp::SharedMcpPool::from_default_config());

    handle_comm_assign_next(
        201,
        requester.to_string(),
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

    match client_rx.recv().await.expect("response") {
        ServerEvent::CommAssignTaskResponse {
            id,
            task_id,
            target_session,
        } => {
            assert_eq!(id, 201);
            assert_eq!(
                task_id, "stuck",
                "stale-assigned runnable task must be reclaimed"
            );
            assert_eq!(
                target_session, live_worker,
                "reclaimed task must go to a live worker"
            );
        }
        ServerEvent::Error { message, .. } => panic!(
            "F1 stall reproduced: assign_next refused to reclaim a task whose \
             assignee left the swarm: {message}"
        ),
        other => panic!("expected CommAssignTaskResponse, got {other:?}"),
    }

    let plans = swarm_plans.read().await;
    let plan = plans.get(swarm_id).expect("plan exists");
    let item = plan.items.iter().find(|item| item.id == "stuck").unwrap();
    assert_eq!(
        item.assigned_to.as_deref(),
        Some(live_worker),
        "assignment must be moved off the departed session"
    );
    drop(plans);
    // W1 dual-write: fold(control log) must agree with the maps after the fix.
    crate::server::control_log_sync::test_support::assert_control_log_matches_maps(
        swarm_id,
        &swarm_members,
        &swarm_plans,
    )
    .await;
}

/// F2 - premature wake: `update_member_status` sets "ready" at every turn end
/// (comm_session.rs), and `await_members` treats status=="ready" as done
/// (comm_await.rs) with no evidence check. A mid-task turn boundary therefore
/// wakes the coordinator with a truncated "report".
///
/// Desired: a member whose assigned plan task is still non-terminal is NOT
/// done, even if its status string momentarily reads "ready".
#[tokio::test]
async fn f2_await_members_ignores_mid_task_turn_boundary() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-f2-wake";
    let requester = "coord-f2";
    let worker = "worker-f2";
    let await_runtime = AwaitMembersRuntime::default();

    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (requester.to_string(), member(requester, swarm_id, "ready")),
        (worker.to_string(), member(worker, swarm_id, "running")),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), worker.to_string()]),
    )])));
    // The worker's assigned task is mid-flight in the plan.
    let mut task = plan_item("t1", "running", "high", &[]);
    task.assigned_to = Some(worker.to_string());
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![task],
            version: 1,
            participants: HashSet::from([requester.to_string(), worker.to_string()]),
            task_progress: HashMap::new(),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);

    handle_comm_await_members(
        202,
        requester.to_string(),
        vec![
            "ready".to_string(),
            "completed".to_string(),
            "stopped".to_string(),
            "failed".to_string(),
        ],
        vec![worker.to_string()],
        Some("all".to_string()),
        Some(1),
        false,
        false,
        false,
        CommAwaitMembersContext {
            client_event_tx: &client_tx,
            swarm_members: &swarm_members,
            swarms_by_id: &swarms_by_id,
            swarm_plans: &swarm_plans,
            swarm_event_tx: &swarm_event_tx,
            await_members_runtime: &await_runtime,
        },
    )
    .await;

    // Mid-task turn boundary: the worker's status flips to "ready" while its
    // plan task is still running (this is exactly what comm_session.rs does at
    // every turn end).
    {
        let mut members = swarm_members.write().await;
        members.get_mut(worker).expect("worker exists").status = "ready".to_string();
    }
    let _ = swarm_event_tx.send(swarm_event(
        worker,
        swarm_id,
        SwarmEventType::StatusChange {
            old_status: "running".to_string(),
            new_status: "ready".to_string(),
        },
    ));

    let response = tokio::time::timeout(Duration::from_secs(3), client_rx.recv())
        .await
        .expect("await should respond (timeout path at worst)")
        .expect("channel should stay open");

    match response {
        ServerEvent::CommAwaitMembersResponse { completed, .. } => {
            assert!(
                !completed,
                "F2 premature wake reproduced: await_members treated a mid-task \
                 turn boundary (status 'ready', plan task 't1' still running) as done"
            );
        }
        other => panic!("expected CommAwaitMembersResponse, got {other:?}"),
    }
}

/// F3 - owner-only complete deadlock: `dag::complete_node` rejects any actor
/// but the owner (NotOwner, dag/ops.rs), and there is no coordinator override
/// or salvage path. When a worker dies mid-node, the node wedges forever:
/// nobody can complete it, fail it (owner-only too), or requeue it (requires
/// Failed status).
///
/// Desired: the coordinator can salvage-complete a node whose owner is no
/// longer a live swarm member.
#[tokio::test]
async fn f3_coordinator_can_salvage_node_of_departed_owner() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-f3-salvage";
    let requester = "coord-f3";
    let ghost = "worker-f3-ghost"; // owns the running node, no longer a member
    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let swarm_members = Arc::new(RwLock::new(HashMap::from([(requester.to_string(), {
        let mut member = member(requester, swarm_id, "ready");
        member.role = "coordinator".to_string();
        member
    })])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string()]),
    )])));
    let mut orphaned = plan_item("orphaned", "running", "high", &[]);
    orphaned.assigned_to = Some(ghost.to_string());
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![orphaned],
            version: 1,
            participants: HashSet::from([requester.to_string()]),
            task_progress: HashMap::new(),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        requester.to_string(),
    )])));
    let event_history = Arc::new(RwLock::new(VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(1));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);

    scoreboard_complete_node(
        203,
        requester.to_string(),
        "orphaned".to_string(),
        serde_json::json!({
            "findings": "salvaged from departed worker; work verified complete",
        })
        .to_string(),
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

    // Drain until we hit the terminal response for this request.
    loop {
        match client_rx.recv().await.expect("response") {
            ServerEvent::Done { id } => {
                assert_eq!(id, 203);
                break;
            }
            ServerEvent::Error { message, .. } => panic!(
                "F3 deadlock reproduced: coordinator could not salvage-complete \
                 a node owned by a departed worker: {message}"
            ),
            _ => continue,
        }
    }

    let plans = swarm_plans.read().await;
    let plan = plans.get(swarm_id).expect("plan exists");
    let item = plan.items.iter().find(|item| item.id == "orphaned").unwrap();
    assert!(
        matches!(item.status.as_str(), "completed" | "done"),
        "salvaged node must be terminal, got '{}'",
        item.status
    );
    drop(plans);
    // W1 dual-write: fold(control log) must agree with the maps after salvage.
    crate::server::control_log_sync::test_support::assert_control_log_matches_maps(
        swarm_id,
        &swarm_members,
        &swarm_plans,
    )
    .await;
}

/// F4 - coordinator desync: coordinatorship lives in TWO places, the
/// `swarm_coordinators` map (authoritative for permission checks) and the
/// member's `role` string (what `swarm list` shows). When they diverge the
/// operator sees "coordinator" but every assign fails with "Only the
/// coordinator can assign tasks", and `resync_plan` (comm_sync.rs) only
/// repairs plan.participants - never the coordinators map.
///
/// Desired: resync_plan reconciles the coordinators map from the member role
/// state, after which assignment works again.
#[tokio::test]
async fn f4_resync_plan_repairs_coordinator_map_desync() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-f4-desync";
    let requester = "coord-f4";
    let worker = "worker-f4";
    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let soft_interrupt_queues = Arc::new(RwLock::new(HashMap::new()));
    let client_connections = Arc::new(RwLock::new(HashMap::new()));
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (requester.to_string(), {
            let mut member = member(requester, swarm_id, "ready");
            member.role = "coordinator".to_string(); // swarm list says coordinator
            member
        }),
        (
            worker.to_string(),
            owned_member(worker, swarm_id, "ready", requester),
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), worker.to_string()]),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![plan_item("task-1", "queued", "high", &[])],
            version: 1,
            participants: HashSet::from([requester.to_string(), worker.to_string()]),
            task_progress: HashMap::new(),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    // Desync: the coordinators map lost its entry (restart, race, stale
    // restore) while the member role string still says coordinator.
    let swarm_coordinators: Arc<RwLock<HashMap<String, String>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let event_history = Arc::new(RwLock::new(VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(1));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);
    let mutation_runtime = SwarmMutationRuntime::default();

    // The documented recovery step. Today it only touches plan.participants.
    handle_comm_resync_plan(
        204,
        requester.to_string(),
        &CommResyncPlanContext {
            client_event_tx: &client_tx,
            swarm_members: &swarm_members,
            swarms_by_id: &swarms_by_id,
            swarm_plans: &swarm_plans,
            swarm_coordinators: &swarm_coordinators,
            event_history: &event_history,
            event_counter: &event_counter,
            swarm_event_tx: &swarm_event_tx,
        },
    )
    .await;
    // Drain the resync response(s).
    while let Ok(event) = client_rx.try_recv() {
        if let ServerEvent::Error { message, .. } = event {
            panic!("resync_plan itself failed: {message}");
        }
    }

    assert_eq!(
        swarm_coordinators.read().await.get(swarm_id),
        Some(&requester.to_string()),
        "F4 reproduced: resync_plan did not repair the coordinators map from \
         the member's coordinator role"
    );

    handle_comm_assign_task(
        205,
        requester.to_string(),
        Some(worker.to_string()),
        Some("task-1".to_string()),
        None,
        &client_tx,
        &sessions,
        &soft_interrupt_queues,
        &client_connections,
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
        &event_history,
        &event_counter,
        &swarm_event_tx,
        &mutation_runtime,
    )
    .await;

    match client_rx.recv().await.expect("response") {
        ServerEvent::CommAssignTaskResponse { id, task_id, .. } => {
            assert_eq!(id, 205);
            assert_eq!(task_id, "task-1");
        }
        ServerEvent::Error { message, .. } => {
            panic!("assignment still blocked after resync: {message}")
        }
        other => panic!("expected CommAssignTaskResponse, got {other:?}"),
    }

    // W1 dual-write: fold(control log) must agree with the maps after the
    // repaired assignment persisted.
    crate::server::control_log_sync::test_support::assert_control_log_matches_maps(
        swarm_id,
        &swarm_members,
        &swarm_plans,
    )
    .await;
}
