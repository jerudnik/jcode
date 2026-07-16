#[tokio::test]
async fn assign_task_without_task_id_picks_highest_priority_runnable_task() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-assign";
    let requester = "coord";
    let worker = "worker";
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
        (worker.to_string(), member(worker, swarm_id, "ready")),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), worker.to_string()]),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![
                plan_item("done", "completed", "high", &[]),
                plan_item("blocked", "queued", "high", &["high-ready"]),
                plan_item("low-ready", "queued", "low", &["done"]),
                plan_item("high-ready", "queued", "high", &["done"]),
            ],
            version: 1,
            participants: HashSet::from([requester.to_string(), worker.to_string()]),
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
        77,
        requester.to_string(),
        Some(worker.to_string()),
        None,
        Some("Pick the next task".to_string()),
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

    let response = client_rx.recv().await.expect("response");
    match response {
        ServerEvent::CommAssignTaskResponse {
            id,
            task_id,
            target_session,
        } => {
            assert_eq!(id, 77);
            assert_eq!(task_id, "high-ready");
            assert_eq!(target_session, worker);
        }
        other => panic!("expected CommAssignTaskResponse, got {other:?}"),
    }

    let plans = swarm_plans.read().await;
    let plan = plans.get(swarm_id).expect("plan exists");
    let selected = plan
        .items
        .iter()
        .find(|item| item.id == "high-ready")
        .expect("selected task exists");
    assert_eq!(selected.assigned_to.as_deref(), Some(worker));
    assert_eq!(selected.status, "queued");

    let blocked = plan
        .items
        .iter()
        .find(|item| item.id == "blocked")
        .expect("blocked task exists");
    assert!(
        blocked.assigned_to.is_none(),
        "blocked task should not be auto-assigned"
    );

    let members = swarm_members.read().await;
    let worker_member = members.get(worker).expect("worker member exists");
    assert_eq!(
        worker_member.status, "queued",
        "assigned worker should stop looking completed/ready before async execution starts"
    );
}

#[tokio::test]
async fn assign_task_marks_completed_worker_queued_before_returning() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-assign-completed-worker";
    let requester = "coord";
    let worker = "worker-completed";
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
        (worker.to_string(), member(worker, swarm_id, "completed")),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), worker.to_string()]),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![plan_item("next", "queued", "high", &[])],
            version: 1,
            participants: HashSet::from([requester.to_string(), worker.to_string()]),
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
        78,
        requester.to_string(),
        Some(worker.to_string()),
        Some("next".to_string()),
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
            assert_eq!(id, 78);
            assert_eq!(task_id, "next");
            assert_eq!(target_session, worker);
        }
        other => panic!("expected CommAssignTaskResponse, got {other:?}"),
    }

    let members = swarm_members.read().await;
    let worker_member = members.get(worker).expect("worker member exists");
    assert_eq!(worker_member.status, "queued");
    assert!(
        worker_member
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("task next")),
        "queued member should include the assigned task in its detail"
    );
}

#[tokio::test]
async fn assign_task_stale_direct_takeover_preserves_progress_history() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-direct-takeover-history";
    let requester = "coord";
    let old_worker = "old-worker";
    let new_worker = "new-worker";
    let task_id = "stale-task";
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
            old_worker.to_string(),
            member(old_worker, swarm_id, "crashed"),
        ),
        (
            new_worker.to_string(),
            member(new_worker, swarm_id, "ready"),
        ),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([
            requester.to_string(),
            old_worker.to_string(),
            new_worker.to_string(),
        ]),
    )])));
    let mut stale_item = plan_item(task_id, "running_stale", "high", &[]);
    stale_item.assigned_to = Some(old_worker.to_string());
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![stale_item],
            version: 1,
            participants: HashSet::from([requester.to_string(), old_worker.to_string()]),
            task_progress: HashMap::from([(
                task_id.to_string(),
                SwarmTaskProgress {
                    assigned_session_id: Some(old_worker.to_string()),
                    assignment_summary: Some("old assignment".to_string()),
                    assigned_at_unix_ms: Some(11),
                    last_heartbeat_unix_ms: Some(42),
                    last_detail: Some("old detail".to_string()),
                    checkpoint_summary: Some("old checkpoint".to_string()),
                    checkpoint_count: Some(3),
                    dead_assignee_reclaims: Some(2),
                    stale_since_unix_ms: Some(99),
                    ..SwarmTaskProgress::default()
                },
            )]),
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
        79,
        requester.to_string(),
        Some(new_worker.to_string()),
        Some(task_id.to_string()),
        Some("take over stale work".to_string()),
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
            task_id: assigned_task_id,
            target_session,
        } => {
            assert_eq!(id, 79);
            assert_eq!(assigned_task_id, task_id);
            assert_eq!(target_session, new_worker);
        }
        other => panic!("expected CommAssignTaskResponse, got {other:?}"),
    }

    let plans = swarm_plans.read().await;
    let plan = plans.get(swarm_id).expect("plan exists");
    let item = plan.items.iter().find(|item| item.id == task_id).unwrap();
    assert_eq!(item.assigned_to.as_deref(), Some(new_worker));
    assert_eq!(item.status, "queued");

    let progress = plan.task_progress.get(task_id).expect("progress preserved");
    assert_eq!(progress.assigned_session_id.as_deref(), Some(new_worker));
    assert_eq!(progress.last_heartbeat_unix_ms, Some(42));
    assert_eq!(progress.last_detail.as_deref(), Some("old detail"));
    assert_eq!(progress.checkpoint_count, Some(3));
    assert_eq!(progress.dead_assignee_reclaims, Some(2));
    assert_eq!(progress.stale_since_unix_ms, None);
    let checkpoint_summary = progress.checkpoint_summary.as_deref().unwrap_or_default();
    assert!(checkpoint_summary.contains("old checkpoint"));
    assert!(checkpoint_summary.contains("assignment takeover"));
    assert!(checkpoint_summary.contains(old_worker));
    assert!(checkpoint_summary.contains(new_worker));
}
