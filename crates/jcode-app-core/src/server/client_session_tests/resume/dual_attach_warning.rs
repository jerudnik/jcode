/// R05 gate: "Second attach to a live session either takes over with a banner
/// or surfaces a dual-attach warning on both clients."
///
/// A second client instance attaching to a live session is deliberately NOT
/// taken over (the existing owner is a different live client that may be
/// mid-turn). Before the fix, that refusal fell through silently and left two
/// clients attached with neither told, so each ran its own stall guard and
/// cancelled the other's turn (2026-07-20 incident). Both attached clients must
/// now receive a dual-attach warning.
#[tokio::test]
async fn handle_resume_session_warns_both_clients_on_refused_takeover() -> Result<()> {
    let _guard = crate::storage::lock_test_env();
    let (_runtime, prev_runtime) = setup_runtime_dir()?;

    let target_session_id = "session_dual_attach_warning_target";
    let temp_session_id = "session_dual_attach_warning_temp";

    let mut persisted = crate::session::Session::create_with_id(
        target_session_id.to_string(),
        None,
        Some("Dual Attach Warning".to_string()),
    );
    persisted.save()?;

    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let existing_registry = Registry::new(provider.clone()).await;
    let existing_agent = Arc::new(Mutex::new(build_test_agent_with_id(
        provider.clone(),
        existing_registry,
        target_session_id,
        Vec::new(),
    )));

    let new_registry = Registry::new(provider.clone()).await;
    let new_agent = Arc::new(Mutex::new(build_test_agent_with_id(
        provider.clone(),
        new_registry.clone(),
        temp_session_id,
        Vec::new(),
    )));

    let sessions = Arc::new(RwLock::new(HashMap::from([
        (target_session_id.to_string(), Arc::clone(&existing_agent)),
        (temp_session_id.to_string(), Arc::clone(&new_agent)),
    ])));
    let shutdown_signals = Arc::new(RwLock::new(HashMap::<String, InterruptSignal>::new()));
    let soft_interrupt_queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));
    let now = Instant::now();
    let (disconnect_tx, mut disconnect_rx) = mpsc::unbounded_channel();
    let client_connections = Arc::new(RwLock::new(HashMap::from([
        (
            "conn_existing".to_string(),
            ClientConnectionInfo {
                client_id: "conn_existing".to_string(),
                session_id: target_session_id.to_string(),
                client_instance_id: Some("client_instance_existing".to_string()),
                debug_client_id: Some("debug_existing".to_string()),
                connected_at: now,
                last_seen: now,
                is_processing: false,
                current_tool_name: None,
                terminal_env: Vec::new(),
                disconnect_tx,
            },
        ),
        (
            "conn_new".to_string(),
            ClientConnectionInfo {
                client_id: "conn_new".to_string(),
                session_id: temp_session_id.to_string(),
                client_instance_id: Some("client_instance_new".to_string()),
                debug_client_id: Some("debug_new".to_string()),
                connected_at: now,
                last_seen: now,
                is_processing: false,
                current_tool_name: None,
                terminal_env: Vec::new(),
                disconnect_tx: mpsc::unbounded_channel().0,
            },
        ),
    ])));
    let client_debug_state = Arc::new(RwLock::new(ClientDebugState::default()));

    // The already-attached client's event sender is registered on the live
    // session member, which is how the server fans events to every attached
    // connection. The warning must reach this client too, not just the joiner.
    let (existing_client_event_tx, mut existing_client_event_rx) =
        mpsc::unbounded_channel::<ServerEvent>();
    let swarm_members = Arc::new(RwLock::new(HashMap::from([(
        target_session_id.to_string(),
        SwarmMember {
            session_id: target_session_id.to_string(),
            event_tx: existing_client_event_tx.clone(),
            event_txs: HashMap::from([(
                "conn_existing".to_string(),
                existing_client_event_tx.clone(),
            )]),
            working_dir: None,
            swarm_id: None,
            swarm_enabled: false,
            status: "ready".to_string(),
            lifecycle: Default::default(),
            detail: None,
            task_label: None,
            subagent_type: None,
            friendly_name: Some("existing".to_string()),
            report_back_to_session_id: None,
            initial_prompt_delivered: None,
            latest_completion_report: None,
            role: "agent".to_string(),
            joined_at: now,
            last_status_change: now,
            is_headless: false,
            output_tail: None,
            todo_progress: None,
            todo_items: Vec::new(),
            runtime: crate::protocol::SwarmMemberRuntime::default(),
        },
    )])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::<String, HashSet<String>>::new()));
    let file_touch = FileTouchService::new();
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::<String, String>::new()));
    let client_count = Arc::new(RwLock::new(2usize));
    let (writer, _peer_stream) = test_writer()?;
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel::<ServerEvent>();
    let event_history = Arc::new(RwLock::new(VecDeque::<SwarmEvent>::new()));
    let event_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel::<SwarmEvent>(8);
    let mcp_pool = Arc::new(crate::mcp::SharedMcpPool::from_default_config());

    let mut client_selfdev = false;
    let mut client_session_id = temp_session_id.to_string();

    handle_resume_session(
        77,
        target_session_id.to_string(),
        None,
        Some("client_instance_new"),
        true,
        true,
        &mut client_selfdev,
        &mut client_session_id,
        "conn_new",
        &new_agent,
        &provider,
        &new_registry,
        &sessions,
        &shutdown_signals,
        &soft_interrupt_queues,
        &client_connections,
        &client_debug_state,
        &swarm_members,
        &swarms_by_id,
        &file_touch,
        &swarm_plans,
        &swarm_coordinators,
        &client_count,
        &writer,
        "test-server",
        "🌿",
        &client_event_tx,
        &mcp_pool,
        &event_history,
        &event_counter,
        &swarm_event_tx,
    )
    .await?;

    // Preserve the established behaviour this fix must not regress: the
    // existing live client is not kicked, and the joiner still attaches.
    assert!(
        disconnect_rx.try_recv().is_err(),
        "existing live client must not be kicked by a refused takeover"
    );
    assert_eq!(client_session_id, target_session_id);

    let is_dual_attach_warning = |event: &ServerEvent| {
        matches!(
            event,
            ServerEvent::Notification { message, .. }
                if message.contains("Two clients are attached")
        )
    };

    let joiner_events = collect_events_until_done(&mut client_event_rx, 77).await;
    assert!(
        joiner_events.iter().any(is_dual_attach_warning),
        "joining client must be warned about the dual attach, got {joiner_events:?}"
    );

    let mut existing_events = Vec::new();
    while let Ok(event) = existing_client_event_rx.try_recv() {
        existing_events.push(event);
    }
    assert!(
        existing_events.iter().any(is_dual_attach_warning),
        "already-attached client must be warned about the dual attach, got {existing_events:?}"
    );

    restore_runtime_dir(prev_runtime);
    Ok(())
}
