struct ScopedEnvVar {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(previous) => crate::env::set_var(self.key, previous),
            None => crate::env::remove_var(self.key),
        }
    }
}

#[tokio::test]
async fn dead_pid_sweep_marks_swarm_member_crashed_without_picker() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home = ScopedEnvVar::set("JCODE_HOME", temp_home.path());

    let dead_pid = 99_999_999;
    let mut session =
        crate::session::Session::create_with_id("dead-visible-worker".to_string(), None, None);
    session.mark_active_with_pid(dead_pid);
    session.save().expect("persist active session");

    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["dead-visible-worker".to_string()]),
    )])));
    let (mut member, _rx) = swarm_member("dead-visible-worker", "agent", false);
    member.status = "ready".to_string();
    swarm_members
        .write()
        .await
        .insert("dead-visible-worker".to_string(), member);

    let changed = super::sweep_dead_pid_swarm_members(&swarm_members, &swarms_by_id).await;

    assert_eq!(changed, vec!["swarm-1".to_string()]);
    let members = swarm_members.read().await;
    let member = members.get("dead-visible-worker").expect("member");
    assert_eq!(member.status, "crashed");
    assert_eq!(member.detail.as_deref(), Some("client process exited"));
}

#[tokio::test]
async fn dead_pid_sweep_then_salvage_requeues_once_without_duplicate_assignment() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home = ScopedEnvVar::set("JCODE_HOME", temp_home.path());

    let dead_pid = 99_999_998;
    let worker_id = "dead-visible-worker-chain";
    let mut session =
        crate::session::Session::create_with_id(worker_id.to_string(), None, None);
    session.mark_active_with_pid(dead_pid);
    session.save().expect("persist active session");

    let swarm_id = "swarm-1";
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from(["coord".to_string(), worker_id.to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        "coord".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        VersionedPlan {
            items: vec![PlanItem {
                content: "task".to_string(),
                status: "running".to_string(),
                priority: "high".to_string(),
                id: "task-1".to_string(),
                subsystem: None,
                file_scope: Vec::new(),
                blocked_by: Vec::new(),
                assigned_to: Some(worker_id.to_string()),
            }],
            version: 1,
            participants: HashSet::from(["coord".to_string(), worker_id.to_string()]),
            task_progress: HashMap::from([(
                "task-1".to_string(),
                crate::server::SwarmTaskProgress {
                    assigned_session_id: Some(worker_id.to_string()),
                    last_heartbeat_unix_ms: Some(42),
                    last_detail: Some("old detail".to_string()),
                    checkpoint_summary: Some("old checkpoint".to_string()),
                    checkpoint_count: Some(3),
                    ..Default::default()
                },
            )]),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    let (coord, mut coord_rx) = swarm_member("coord", "coordinator", false);
    let (mut worker, _worker_rx) = swarm_member(worker_id, "agent", false);
    worker.status = "running".to_string();
    {
        let mut members = swarm_members.write().await;
        members.insert("coord".to_string(), coord);
        members.insert(worker_id.to_string(), worker);
    }

    let changed = super::sweep_dead_pid_swarm_members(&swarm_members, &swarms_by_id).await;

    assert_eq!(changed, vec![swarm_id.to_string()]);
    {
        let members = swarm_members.read().await;
        let member = members.get(worker_id).expect("member");
        assert_eq!(member.status, "crashed");
        assert_eq!(member.detail.as_deref(), Some("client process exited"));
    }

    let outcome = salvage_assignments_of_dead_member(
        worker_id,
        swarm_id,
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;

    assert_eq!(outcome.requeued_task_ids, vec!["task-1".to_string()]);
    assert!(outcome.failed_task_ids.is_empty());
    let plans = swarm_plans.read().await;
    let plan = plans.get(swarm_id).expect("plan");
    let task = plan.items.iter().find(|item| item.id == "task-1").unwrap();
    assert_eq!(task.status, "queued");
    assert_eq!(
        task.assigned_to, None,
        "no duplicate assignment remains after salvage"
    );
    let progress = plan.task_progress.get("task-1").expect("progress");
    assert_eq!(progress.assigned_session_id, None);
    assert_eq!(progress.dead_assignee_reclaims, Some(1));
    assert_eq!(progress.last_heartbeat_unix_ms, Some(42));
    assert_eq!(progress.last_detail.as_deref(), Some("old detail"));
    let checkpoint_summary = progress.checkpoint_summary.as_deref().unwrap_or_default();
    assert!(checkpoint_summary.contains("old checkpoint"));
    assert!(checkpoint_summary.contains("assignment reclaimed"));
    drop(plans);

    let coord_events: Vec<_> = std::iter::from_fn(|| coord_rx.try_recv().ok()).collect();
    assert!(
        coord_events.iter().any(|event| matches!(
            event,
            ServerEvent::Notification { message, .. }
                if message.contains("died") && message.contains("task-1")
        )),
        "coordinator should be notified of dead-PID salvage, got {coord_events:?}"
    );
}
