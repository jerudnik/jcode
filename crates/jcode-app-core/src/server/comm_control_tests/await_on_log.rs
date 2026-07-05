// W2 (await-on-log): the control log is the TRUTH for await wakes; the
// swarm_event_tx broadcast is only a nudge. These scoreboard-style tests drive
// completions through the production LOG funnels (persist_swarm_state_for,
// broadcast_swarm_status, complete_node) while sending NO nudge on the
// watcher's broadcast channel, and require the await to wake anyway.
//
// Pre-W2 (status sampling parked on swarm_event_tx-as-truth) both tests time
// out with completed=false: a dropped/lagged/never-sent broadcast silently
// swallows the wake, which is the F2 failure class these retire structurally.
//
// Coverage fork (the W2 design decision): the wake predicate must NOT be
// keyed purely on ArtifactFiled. Light-mode auto-complete and salvage-shaped
// departures reach terminal state without the awaited member ever filing an
// artifact, so an artifact-only await would hang forever on them. Each test
// pins one of those uncovered paths.

use crate::server::persist_swarm_state_for as w2_persist_swarm_state_for;
use crate::server::swarm::broadcast_swarm_status as w2_broadcast_swarm_status;

/// Light-mode auto-complete: at turn end the plan item flips straight to
/// "done" (no ArtifactFiled is ever appended; see turn_end_disposition in
/// comm_control.rs) and the member flips to "ready". Both transitions reach
/// the control log via the production funnels. The await must wake from the
/// log alone.
#[tokio::test]
async fn w2_await_wakes_on_light_auto_complete_without_broadcast_nudge() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-w2-light";
    let requester = "coord-w2l";
    let worker = "worker-w2l";
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
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        requester.to_string(),
    )])));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);

    // Seed the log with the pre-completion view (production state: the swarm
    // has been dual-writing all along; the await's cursor anchors at this tail).
    let swarm_state = crate::server::SwarmState {
        members: Arc::clone(&swarm_members),
        swarms_by_id: Arc::clone(&swarms_by_id),
        plans: Arc::clone(&swarm_plans),
        coordinators: Arc::clone(&swarm_coordinators),
    };
    w2_persist_swarm_state_for(swarm_id, &swarm_state).await;

    handle_comm_await_members(
        301,
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

    // Let the watcher pass its initial level check and park, so the wake can
    // only come from what happens next (the same parking await_lagged relies on).
    tokio::time::timeout(Duration::from_secs(1), async {
        while swarm_event_tx.receiver_count() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("watcher should subscribe to swarm events");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Light-mode turn end: task auto-completes, worker returns to ready. The
    // ONLY signals are the log funnels; no swarm_event_tx nudge is sent.
    {
        let mut plans = swarm_plans.write().await;
        plans
            .get_mut(swarm_id)
            .expect("plan exists")
            .items
            .iter_mut()
            .find(|item| item.id == "t1")
            .expect("task exists")
            .status = "done".to_string();
    }
    w2_persist_swarm_state_for(swarm_id, &swarm_state).await;
    {
        let mut members = swarm_members.write().await;
        members.get_mut(worker).expect("worker exists").status = "ready".to_string();
    }
    w2_broadcast_swarm_status(swarm_id, &swarm_members, &swarms_by_id).await;

    let response = tokio::time::timeout(Duration::from_secs(4), client_rx.recv())
        .await
        .expect("await should respond (deadline path at worst)")
        .expect("channel should stay open");

    match response {
        ServerEvent::CommAwaitMembersResponse {
            completed, members, ..
        } => {
            assert!(
                completed,
                "W2 lost-wake reproduced: light-mode auto-complete reached the \
                 control log (TaskStatusChanged 'done' + MemberStatusChanged \
                 'ready') but the await never woke because no broadcast nudge \
                 was sent; the log must be the wake source"
            );
            assert_eq!(members.len(), 1);
            assert!(members[0].done, "worker must be reported done");
        }
        other => panic!("expected CommAwaitMembersResponse, got {other:?}"),
    }
}

/// Salvage shape: the awaited worker departs (crash/evict -> MemberLeft in the
/// log) and the coordinator salvage-completes its orphaned node
/// (ArtifactFiled is appended by the COORDINATOR's session, not the awaited
/// member's, plus a derived terminal TaskStatusChanged). The complete_node
/// call runs with a detached broadcast channel to model a lost/lagged nudge:
/// the log alone must wake the await.
#[tokio::test]
async fn w2_await_wakes_on_salvage_of_departed_owner_without_broadcast_nudge() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-w2-salvage";
    let requester = "coord-w2s";
    let ghost = "worker-w2s-ghost";
    let await_runtime = AwaitMembersRuntime::default();

    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (requester.to_string(), {
            let mut member = member(requester, swarm_id, "ready");
            member.role = "coordinator".to_string();
            member
        }),
        (ghost.to_string(), member(ghost, swarm_id, "running")),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), ghost.to_string()]),
    )])));
    let mut orphaned = plan_item("orphaned", "running", "high", &[]);
    orphaned.assigned_to = Some(ghost.to_string());
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![orphaned],
            version: 1,
            participants: HashSet::from([requester.to_string(), ghost.to_string()]),
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

    let swarm_state = crate::server::SwarmState {
        members: Arc::clone(&swarm_members),
        swarms_by_id: Arc::clone(&swarms_by_id),
        plans: Arc::clone(&swarm_plans),
        coordinators: Arc::clone(&swarm_coordinators),
    };
    w2_persist_swarm_state_for(swarm_id, &swarm_state).await;

    handle_comm_await_members(
        302,
        requester.to_string(),
        vec![
            "ready".to_string(),
            "completed".to_string(),
            "stopped".to_string(),
            "failed".to_string(),
        ],
        vec![ghost.to_string()],
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

    tokio::time::timeout(Duration::from_secs(1), async {
        while swarm_event_tx.receiver_count() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("watcher should subscribe to swarm events");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // The worker departs: map removal + the member-view log funnel appends
    // MemberLeft. No swarm_event_tx nudge.
    {
        let mut members = swarm_members.write().await;
        members.remove(ghost);
    }
    {
        let mut swarms = swarms_by_id.write().await;
        swarms.get_mut(swarm_id).expect("swarm exists").remove(ghost);
    }
    w2_broadcast_swarm_status(swarm_id, &swarm_members, &swarms_by_id).await;

    // Coordinator salvage-completes the orphaned node. The nudge channel here
    // is DETACHED (a lost broadcast); only the log records the completion
    // (ArtifactFiled by the coordinator + derived TaskStatusChanged 'done').
    let (detached_tx, _detached_rx) = broadcast::channel(32);
    let (salvage_tx, mut salvage_rx) = mpsc::unbounded_channel();
    scoreboard_complete_node(
        303,
        requester.to_string(),
        "orphaned".to_string(),
        serde_json::json!({
            "findings": "salvaged from departed worker; work verified complete",
        })
        .to_string(),
        &salvage_tx,
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
        &event_history,
        &event_counter,
        &detached_tx,
    )
    .await;
    loop {
        match salvage_rx.recv().await.expect("salvage response") {
            ServerEvent::Done { id } => {
                assert_eq!(id, 303);
                break;
            }
            ServerEvent::Error { message, .. } => panic!("salvage failed: {message}"),
            _ => continue,
        }
    }

    let response = tokio::time::timeout(Duration::from_secs(4), client_rx.recv())
        .await
        .expect("await should respond (deadline path at worst)")
        .expect("channel should stay open");

    match response {
        ServerEvent::CommAwaitMembersResponse {
            completed, members, ..
        } => {
            assert!(
                completed,
                "W2 lost-wake reproduced: the departed owner (MemberLeft) and \
                 salvage completion (ArtifactFiled + terminal TaskStatusChanged) \
                 all reached the control log, but the await never woke because \
                 the broadcast nudge was lost; the log must be the wake source"
            );
            assert_eq!(members.len(), 1);
            assert!(members[0].done, "departed+salvaged worker must be done");
        }
        other => panic!("expected CommAwaitMembersResponse, got {other:?}"),
    }

    // The salvage persisted: fold(log) must agree with the maps.
    crate::server::control_log_sync::test_support::assert_control_log_matches_maps(
        swarm_id,
        &swarm_members,
        &swarm_plans,
    )
    .await;
}

/// Migration contract: a resumed legacy/pre-W2 pending state (scan_offset 0,
/// the serde default) must RE-ANCHOR its cursor at the current log tail, not
/// replay from zero. Replay-from-zero would grind through the swarm's whole
/// pre-await history re-checking on every historical event. The observable
/// contract is the persisted cursor: after the watcher spawns, the state on
/// disk carries the tail offset. New awaits anchor at creation the same way
/// (ensure_pending_state), which the second half pins.
#[tokio::test]
async fn w2_legacy_zero_cursor_reanchors_to_tail_not_zero() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-w2-anchor";
    let requester = "coord-w2a";
    let worker = "worker-w2a";
    let await_runtime = AwaitMembersRuntime::default();

    // Pre-await history in the log: the tail is non-zero before any await.
    crate::server::control_log_sync::append_control_event(
        swarm_id,
        jcode_swarm_core::control_log::SwarmControlEvent::TaskAssigned {
            task_id: "old".to_string(),
            assigned_to: Some(worker.to_string()),
        },
    )
    .expect("append pre-await event");
    let tail = crate::server::control_log_sync::current_control_log_offset(swarm_id);
    assert!(tail > 0);

    // A legacy pending state: persisted before scan_offset existed (0).
    let key = crate::server::await_members_state::request_key(
        requester,
        swarm_id,
        &[worker.to_string()],
        &["completed".to_string()],
        None,
    );
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    crate::server::await_members_state::save_state(
        &crate::server::await_members_state::PersistedAwaitMembersState {
            key: key.clone(),
            session_id: requester.to_string(),
            swarm_id: swarm_id.to_string(),
            target_status: vec!["completed".to_string()],
            requested_ids: vec![worker.to_string()],
            mode: None,
            created_at_unix_ms: now_ms,
            deadline_unix_ms: now_ms + 60_000,
            background: true,
            notify: false,
            wake: false,
            scan_offset: 0,
            final_response: None,
        },
    );

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

    crate::server::comm_await::resume_background_awaits(
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_event_tx,
        &await_runtime,
    )
    .await;

    // The watcher re-anchors and persists the tail cursor.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(state) = crate::server::await_members_state::load_state(&key)
                && state.scan_offset == tail
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("resumed legacy await must re-anchor its cursor at the log tail, not 0");

    // Creation-time anchoring: a brand-new pending state starts at the tail.
    let fresh = crate::server::await_members_state::ensure_pending_state(
        "fresh-key-w2a",
        requester,
        swarm_id,
        &[worker.to_string()],
        &["completed".to_string()],
        None,
        now_ms + 60_000,
        false,
        false,
        false,
    );
    assert_eq!(
        fresh.scan_offset, tail,
        "new awaits must anchor their scan cursor at the current log tail"
    );
}

/// W2 low-confidence routing: a satisfied await whose done member filed
/// low-confidence artifact evidence must say so in the completion summary and
/// point the awaiter at the gate/inject_gap path, instead of presenting the
/// completion as final. (Enforcement lives in the engine: validate_gate_pass
/// rejects a deep gate that skips a low-confidence sibling. The await's job
/// is the routing signal.)
#[tokio::test]
async fn w2_satisfied_await_flags_low_confidence_evidence() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let swarm_id = "swarm-w2-lowconf";
    let requester = "coord-w2lc";
    let worker = "worker-w2lc";
    let await_runtime = AwaitMembersRuntime::default();

    // Evidence trail: the worker completed node n1 with LOW confidence
    // (complete_node appends this before the derived status flip).
    crate::server::control_log_sync::append_control_event(
        swarm_id,
        jcode_swarm_core::control_log::SwarmControlEvent::ArtifactFiled {
            task_id: "n1".to_string(),
            session_id: worker.to_string(),
            confidence: Some("low".to_string()),
        },
    )
    .expect("append evidence");

    let (client_tx, mut client_rx) = mpsc::unbounded_channel();
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        (requester.to_string(), member(requester, swarm_id, "ready")),
        (worker.to_string(), member(worker, swarm_id, "completed")),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([requester.to_string(), worker.to_string()]),
    )])));
    // The node is terminal in the plan: the await is satisfied immediately.
    let mut done_task = plan_item("n1", "done", "high", &[]);
    done_task.assigned_to = Some(worker.to_string());
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![done_task],
            version: 1,
            participants: HashSet::from([requester.to_string(), worker.to_string()]),
            task_progress: HashMap::new(),
            mode: "deep".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(32);

    handle_comm_await_members(
        304,
        requester.to_string(),
        vec!["completed".to_string()],
        vec![worker.to_string()],
        Some("all".to_string()),
        Some(5),
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

    let response = tokio::time::timeout(Duration::from_secs(2), client_rx.recv())
        .await
        .expect("satisfied await should answer inline")
        .expect("channel should stay open");

    match response {
        ServerEvent::CommAwaitMembersResponse {
            completed, summary, ..
        } => {
            assert!(completed, "await must still WAKE on low-confidence work");
            assert!(
                summary.contains("LOW-CONFIDENCE") && summary.contains("n1"),
                "summary must flag the low-confidence node and route toward the \
                 gate path, got: {summary}"
            );
            assert!(
                summary.contains("inject_gap"),
                "summary must name the follow-up mechanism, got: {summary}"
            );
        }
        other => panic!("expected CommAwaitMembersResponse, got {other:?}"),
    }
}
