// Deadline-path freshness: the watcher loop samples member state at the top of
// each iteration and then parks in `select!`, so its snapshot is exactly as old
// as its last wake. In a quiet swarm there IS no last wake, and the snapshot
// dates from when the await was created -- which is the moment the caller
// already knows about.
//
// W2 made the control log the wake source, retiring the lost-nudge class for
// transitions that reach a funnel. These tests pin the residual: a transition
// that reaches NEITHER funnel before the deadline. The deadline arm must decide
// on a fresh read, or it reports a member's start-of-wait status as though it
// were current and calls a satisfied condition a timeout.

/// The awaited worker finishes without either wake source firing. At the
/// deadline the condition is TRUE, so the await must resolve completed --
/// not report the stale "running" it sampled at creation.
#[tokio::test]
async fn deadline_decides_on_fresh_state_not_the_creation_snapshot() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-deadline-fresh";
    let requester = "coord-df";
    let worker = "worker-df";
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
    let swarm_plans = Arc::new(RwLock::new(HashMap::new()));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);

    handle_comm_await_members(
        401,
        requester.to_string(),
        vec![
            "ready".to_string(),
            "completed".to_string(),
            "stopped".to_string(),
            "failed".to_string(),
        ],
        vec![worker.to_string()],
        Some("all".to_string()),
        Some(2),
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

    // Let the watcher take its creation snapshot ("running") and park.
    tokio::time::timeout(Duration::from_secs(1), async {
        while swarm_event_tx.receiver_count() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("watcher should subscribe to swarm events");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // The worker finishes. No log persist, no broadcast: neither wake source
    // fires, so the loop stays parked until its deadline.
    {
        let mut members = swarm_members.write().await;
        members.get_mut(worker).expect("worker exists").status = "ready".to_string();
    }

    let response = tokio::time::timeout(Duration::from_secs(6), client_rx.recv())
        .await
        .expect("await should respond on the deadline path")
        .expect("channel should stay open");

    match response {
        ServerEvent::CommAwaitMembersResponse {
            completed,
            members,
            summary,
            ..
        } => {
            assert!(
                completed,
                "stale-deadline reproduced: the worker was ready before the \
                 deadline but the await reported the snapshot it took at \
                 creation. Summary was {summary:?}"
            );
            assert_eq!(members.len(), 1);
            assert!(
                members[0].done,
                "worker reached a target status and must be reported done"
            );
        }
        other => panic!("expected CommAwaitMembersResponse, got {other:?}"),
    }
}

/// A genuine timeout must still read as a timeout, and must describe the
/// member as it is AT THE DEADLINE rather than at creation. Without this the
/// fix above could be satisfied by reporting everything as done.
#[tokio::test]
async fn deadline_still_times_out_and_reports_the_status_at_the_deadline() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-deadline-stuck";
    let requester = "coord-ds";
    let worker = "worker-ds";
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
    let swarm_plans = Arc::new(RwLock::new(HashMap::new()));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);

    handle_comm_await_members(
        402,
        requester.to_string(),
        vec![
            "ready".to_string(),
            "completed".to_string(),
            "stopped".to_string(),
            "failed".to_string(),
        ],
        vec![worker.to_string()],
        Some("all".to_string()),
        Some(2),
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

    // The worker moves, but not to a target status: still pending at the
    // deadline, and the summary must say so with the CURRENT status.
    tokio::time::timeout(Duration::from_secs(1), async {
        while swarm_event_tx.receiver_count() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("watcher should subscribe to swarm events");
    tokio::time::sleep(Duration::from_millis(50)).await;
    {
        let mut members = swarm_members.write().await;
        members.get_mut(worker).expect("worker exists").status = "blocked".to_string();
    }

    let response = tokio::time::timeout(Duration::from_secs(6), client_rx.recv())
        .await
        .expect("await should respond on the deadline path")
        .expect("channel should stay open");

    match response {
        ServerEvent::CommAwaitMembersResponse {
            completed, summary, ..
        } => {
            assert!(
                !completed,
                "no target status was reached; this must remain a timeout"
            );
            assert!(
                summary.contains("blocked"),
                "the timeout must describe the member at the deadline, not at \
                 creation (expected 'blocked', got {summary:?})"
            );
        }
        other => panic!("expected CommAwaitMembersResponse, got {other:?}"),
    }
}
