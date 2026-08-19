#[tokio::test]
async fn frozen_plan_rejects_expand_and_inject() {
    let (_env, _runtime) = RuntimeEnvGuard::new();
    let mut fx = graph_fixture_named("swarm-frozen", "coord-frozen", "worker-frozen").await;
    fx.seed("light", vec![node_spec("root", "explore", &[])])
        .await;
    assert_eq!(
        fx.swarm_plans.read().await[&fx.swarm_id].max_nodes,
        Some(crate::config::config().agents.swarm_max_graph_nodes),
        "seeding should copy the configured graph budget into the persisted plan"
    );
    while fx.client_rx.try_recv().is_ok() {}

    handle_comm_task_control(
        2,
        fx.worker.clone(),
        "freeze".to_string(),
        String::new(),
        None,
        None,
        &fx.client_tx,
        &fx.sessions,
        &fx.soft_interrupt_queues,
        &fx.client_connections,
        &fx.swarm_members,
        &fx.swarms_by_id,
        &fx.swarm_plans,
        &fx.swarm_coordinators,
        &fx.event_history,
        &fx.event_counter,
        &fx.swarm_event_tx,
        &fx.mutation_runtime,
    )
    .await;
    assert!(matches!(
        fx.client_rx.try_recv(),
        Ok(ServerEvent::Error { message, .. }) if message.contains("Only the coordinator")
    ));
    assert!(!fx.swarm_plans.read().await[&fx.swarm_id].frozen);

    handle_comm_task_control(
        3,
        fx.coord.clone(),
        "freeze".to_string(),
        String::new(),
        None,
        None,
        &fx.client_tx,
        &fx.sessions,
        &fx.soft_interrupt_queues,
        &fx.client_connections,
        &fx.swarm_members,
        &fx.swarms_by_id,
        &fx.swarm_plans,
        &fx.swarm_coordinators,
        &fx.event_history,
        &fx.event_counter,
        &fx.swarm_event_tx,
        &fx.mutation_runtime,
    )
    .await;
    while fx.client_rx.try_recv().is_ok() {}

    fx.seed_replacing(
        "light",
        true,
        vec![node_spec("replacement", "explore", &[])],
    )
    .await;
    handle_comm_expand_node(
        4,
        fx.coord.clone(),
        "root".to_string(),
        vec![node_spec("child", "explore", &[])],
        &fx.client_tx,
        &fx.swarm_members,
        &fx.swarms_by_id,
        &fx.swarm_plans,
        &fx.swarm_coordinators,
        &fx.event_history,
        &fx.event_counter,
        &fx.swarm_event_tx,
    )
    .await;
    crate::server::comm_graph::handle_comm_inject_gap(
        5,
        fx.coord.clone(),
        "missing-gate".to_string(),
        vec![node_spec("gap", "fix", &[])],
        &fx.client_tx,
        &fx.swarm_members,
        &fx.swarms_by_id,
        &fx.swarm_plans,
        &fx.swarm_coordinators,
        &fx.event_history,
        &fx.event_counter,
        &fx.swarm_event_tx,
    )
    .await;

    let frozen_events: Vec<_> = std::iter::from_fn(|| fx.client_rx.try_recv().ok()).collect();
    assert_eq!(
        frozen_events
            .iter()
            .filter(|event| matches!(
                event,
                ServerEvent::Error { message, .. } if message.contains("frozen")
            ))
            .count(),
        3,
        "seed, expand, and inject should all be rejected by the frozen policy: {frozen_events:?}"
    );

    handle_comm_task_control(
        6,
        fx.coord.clone(),
        "unfreeze".to_string(),
        String::new(),
        None,
        None,
        &fx.client_tx,
        &fx.sessions,
        &fx.soft_interrupt_queues,
        &fx.client_connections,
        &fx.swarm_members,
        &fx.swarms_by_id,
        &fx.swarm_plans,
        &fx.swarm_coordinators,
        &fx.event_history,
        &fx.event_counter,
        &fx.swarm_event_tx,
        &fx.mutation_runtime,
    )
    .await;
    while fx.client_rx.try_recv().is_ok() {}

    handle_comm_expand_node(
        7,
        fx.coord.clone(),
        "root".to_string(),
        vec![node_spec("child", "explore", &[])],
        &fx.client_tx,
        &fx.swarm_members,
        &fx.swarms_by_id,
        &fx.swarm_plans,
        &fx.swarm_coordinators,
        &fx.event_history,
        &fx.event_counter,
        &fx.swarm_event_tx,
    )
    .await;

    assert!(
        fx.swarm_plans.read().await[&fx.swarm_id]
            .items
            .iter()
            .any(|item| item.id == "child"),
        "unfreeze should restore graph growth"
    );
}
