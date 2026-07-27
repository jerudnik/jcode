/// A new workflow must never merge with persisted nodes from an older graph.
/// Non-empty reseeds are rejected unless the caller explicitly opts into an
/// atomic replacement, which also clears stale metadata and progress, preserves
/// current swarm participants, and keeps the version monotonic.
#[tokio::test]
async fn e2e_seed_requires_explicit_replacement_and_clears_stale_state() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let mut fx = graph_fixture_named("swarm-reseed", "coord-reseed", "worker-reseed").await;

    fx.seed("deep", vec![node_spec("old", "explore", &[])])
        .await;
    let _ = fx.client_rx.recv().await.expect("initial seed response");

    let (initial_version, initial_ids) = {
        let mut plans = fx.swarm_plans.write().await;
        let plan = plans.get_mut(&fx.swarm_id).expect("seeded plan");
        plan.task_progress.insert("old".to_string(), Default::default());
        plan.participants.insert("stale-participant".to_string());
        assert!(plan.node_meta.contains_key("old"));
        (
            plan.version,
            plan.items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
        )
    };

    fx.seed_replacing("light", false, vec![node_spec("new", "implement", &[])])
        .await;
    let rejected = fx.client_rx.recv().await.expect("reseed rejection");
    match rejected {
        ServerEvent::Error { message, .. } => {
            assert!(message.contains("replace_existing=true"), "{message}");
        }
        other => panic!("expected reseed error, got {other:?}"),
    }
    {
        let plans = fx.swarm_plans.read().await;
        let plan = &plans[&fx.swarm_id];
        assert_eq!(plan.version, initial_version, "rejection must not mutate plan");
        assert_eq!(
            plan.items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
            initial_ids,
            "rejection must preserve the complete deep graph, including its gate"
        );
        assert!(plan.task_progress.contains_key("old"));
        assert!(plan.participants.contains("stale-participant"));
    }

    {
        let mut plans = fx.swarm_plans.write().await;
        for item in &mut plans.get_mut(&fx.swarm_id).expect("seeded plan").items {
            item.status = "completed".to_string();
            item.assigned_to = Some(fx.worker.clone());
        }
    }

    fx.seed_replacing("light", true, vec![node_spec("new", "implement", &[])])
        .await;
    let _ = fx.client_rx.recv().await.expect("replacement response");

    let plans = fx.swarm_plans.read().await;
    let plan = &plans[&fx.swarm_id];
    assert_eq!(plan.version, initial_version + 1);
    assert_eq!(plan.mode, "light");
    assert_eq!(plan.items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), vec!["new"]);
    assert!(!plan.node_meta.contains_key("old"));
    assert!(plan.node_meta.contains_key("new"));
    assert!(plan.task_progress.is_empty());
    assert_eq!(
        plan.participants,
        HashSet::from([fx.coord.clone(), fx.worker.clone()])
    );
}

#[tokio::test]
async fn e2e_replacement_rejects_in_flight_work_without_mutating_roles() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let mut fx = graph_fixture_named("swarm-active-reseed", "coord-active", "worker-active").await;
    fx.seed("light", vec![node_spec("active", "implement", &[])])
        .await;
    while fx.client_rx.try_recv().is_ok() {}

    let version = {
        let mut plans = fx.swarm_plans.write().await;
        let plan = plans.get_mut(&fx.swarm_id).expect("seeded plan");
        plan.items[0].status = "running".to_string();
        plan.items[0].assigned_to = Some(fx.worker.clone());
        plan.version
    };
    fx.swarm_coordinators
        .write()
        .await
        .insert(fx.swarm_id.clone(), "stale-coordinator".to_string());
    fx.swarm_members.write().await.get_mut(&fx.coord).unwrap().role = "agent".to_string();

    fx.seed_replacing("light", true, vec![node_spec("new", "implement", &[])])
        .await;
    let rejected = fx.client_rx.recv().await.expect("active replacement rejection");
    assert!(matches!(
        rejected,
        ServerEvent::Error { message, .. }
            if message.contains("in-flight") && message.contains("active")
    ));

    let plans = fx.swarm_plans.read().await;
    let plan = &plans[&fx.swarm_id];
    assert_eq!(plan.version, version);
    assert_eq!(plan.items[0].id, "active");
    assert_eq!(plan.items[0].assigned_to.as_deref(), Some(fx.worker.as_str()));
    drop(plans);
    assert_eq!(fx.swarm_coordinators.read().await[&fx.swarm_id], "stale-coordinator");
    assert_eq!(fx.swarm_members.read().await[&fx.coord].role, "agent");
}

#[tokio::test]
async fn e2e_identical_seed_without_replace_is_rejected_without_node_churn() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let mut fx = graph_fixture_named("swarm-seed-replay", "coord-replay", "worker-replay").await;
    let nodes = vec![
        node_spec("explore", "explore", &[]),
        node_spec("synth", "synthesize", &["explore"]),
    ];

    fx.seed("deep", nodes.clone()).await;
    while fx.client_rx.try_recv().is_ok() {}
    let (version, item_count) = {
        let plans = fx.swarm_plans.read().await;
        let plan = &plans[&fx.swarm_id];
        (plan.version, plan.items.len())
    };

    fx.seed("deep", nodes).await;

    let plans = fx.swarm_plans.read().await;
    let plan = &plans[&fx.swarm_id];
    assert_eq!(plan.version, version, "a rejected replay must not bump plan version");
    assert_eq!(plan.items.len(), item_count, "a rejected replay must not add nodes");
    drop(plans);
    let events: Vec<_> = std::iter::from_fn(|| fx.client_rx.try_recv().ok()).collect();
    assert!(
        events.iter().any(|event| matches!(
            event,
            ServerEvent::Error { message, .. } if message.contains("replace_existing=true")
        )),
        "an identical raw-protocol replay must require an explicit lifecycle choice: {events:?}"
    );
}
