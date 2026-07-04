// F5 (orchestration-hardening audit): the assign path must resolve friendly
// names, not just exact session IDs. The DM path already has this fallback
// (client_comm_message.rs resolve_dm_target_session) but it was never shared
// with resolve_assignment_target_session, so `assign_task` with a friendly
// name failed with "Unknown session '<name>'" while the same name worked
// for DMs. Live symptom: coordinators forced to use full session IDs
// (swarm-loop-safety.md invariant 5 exists because of this bug).

#[tokio::test]
async fn assign_task_resolves_friendly_name_target() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-friendly";
    let requester = "sess-coord-001";
    let worker_session = "sess-worker-8f3a2b1c";
    let worker_name = "falcon";
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
        (worker_session.to_string(), {
            let mut member = member(worker_session, swarm_id, "ready");
            member.friendly_name = Some(worker_name.to_string());
            member
        }),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), worker_session.to_string()]),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![plan_item("task-1", "queued", "high", &[])],
            version: 1,
            participants: HashSet::from([requester.to_string(), worker_session.to_string()]),
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

    handle_comm_assign_task(
        90,
        requester.to_string(),
        Some(worker_name.to_string()),
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
        ServerEvent::CommAssignTaskResponse {
            id,
            task_id,
            target_session,
        } => {
            assert_eq!(id, 90);
            assert_eq!(task_id, "task-1");
            assert_eq!(
                target_session, worker_session,
                "friendly name must resolve to the underlying session id"
            );
        }
        other => panic!(
            "expected CommAssignTaskResponse for friendly-name target, got {other:?}"
        ),
    }

    let plans = swarm_plans.read().await;
    let plan = plans.get(swarm_id).expect("plan exists");
    let item = plan
        .items
        .iter()
        .find(|item| item.id == "task-1")
        .expect("task exists");
    assert_eq!(
        item.assigned_to.as_deref(),
        Some(worker_session),
        "assignment must record the session id, never the friendly name"
    );
}

#[tokio::test]
async fn assign_task_rejects_ambiguous_friendly_name() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-friendly-ambig";
    let requester = "sess-coord-002";
    let worker_a = "sess-worker-aaaa";
    let worker_b = "sess-worker-bbbb";
    let shared_name = "falcon";
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
        (worker_a.to_string(), {
            let mut member = member(worker_a, swarm_id, "ready");
            member.friendly_name = Some(shared_name.to_string());
            member
        }),
        (worker_b.to_string(), {
            let mut member = member(worker_b, swarm_id, "ready");
            member.friendly_name = Some(shared_name.to_string());
            member
        }),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([
            requester.to_string(),
            worker_a.to_string(),
            worker_b.to_string(),
        ]),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![plan_item("task-1", "queued", "high", &[])],
            version: 1,
            participants: HashSet::from([
                requester.to_string(),
                worker_a.to_string(),
                worker_b.to_string(),
            ]),
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

    handle_comm_assign_task(
        91,
        requester.to_string(),
        Some(shared_name.to_string()),
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
        ServerEvent::Error { message, .. } => {
            assert!(
                message.contains("ambiguous"),
                "ambiguous friendly name must be called out, got: {message}"
            );
        }
        ServerEvent::CommAssignTaskResponse { target_session, .. } => panic!(
            "ambiguous friendly name must not silently pick a worker (picked {target_session})"
        ),
        other => panic!("expected Error for ambiguous friendly name, got {other:?}"),
    }
}
