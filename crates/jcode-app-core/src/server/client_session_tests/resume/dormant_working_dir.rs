// Characterization: what a resume does to a *dormant* session's recorded
// working directory. A live target short-circuits through
// `claim_live_target_agent`, so the restore path never runs and the stored
// directory is untouched; a dormant target goes through
// `restore_session_with_working_dir`, where the override is applied.
//
// The two sentinels below are chosen so they cannot coincide: the directory a
// session was created in and the directory a later client attaches from are
// different absolute paths that appear nowhere else in the harness.

const CREATED_IN: &str = "/sentinel/created/here";
const ATTACHED_FROM: &str = "/sentinel/attached/from";

#[allow(clippy::too_many_arguments)]
async fn resume_dormant_session_with_override(
    target_session_id: &str,
    working_dir_override: Option<&str>,
) -> Result<Option<String>> {
    let temp_session_id = "session_temp_dormant_workdir";

    let mut persisted = crate::session::Session::create_with_id(
        target_session_id.to_string(),
        None,
        Some("Dormant Working Dir".to_string()),
    );
    persisted.working_dir = Some(CREATED_IN.to_string());
    persisted.save()?;

    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let new_registry = Registry::new(provider.clone()).await;
    let new_agent = Arc::new(Mutex::new(build_test_agent_with_id(
        provider.clone(),
        new_registry.clone(),
        temp_session_id,
        Vec::new(),
    )));

    // The target is deliberately absent from `sessions`: that is what makes it
    // dormant, and what routes the resume through the restore path.
    let sessions = Arc::new(RwLock::new(HashMap::from([(
        temp_session_id.to_string(),
        Arc::clone(&new_agent),
    )])));
    let shutdown_signals = Arc::new(RwLock::new(HashMap::<String, InterruptSignal>::new()));
    let soft_interrupt_queues: SessionInterruptQueues = Arc::new(RwLock::new(HashMap::new()));
    let now = Instant::now();
    let client_connections = Arc::new(RwLock::new(HashMap::from([(
        "conn_new".to_string(),
        ClientConnectionInfo {
            client_id: "conn_new".to_string(),
            session_id: temp_session_id.to_string(),
            client_instance_id: None,
            debug_client_id: Some("debug_new".to_string()),
            connected_at: now,
            last_seen: now,
            is_processing: false,
            current_tool_name: None,
            terminal_env: Vec::new(),
            disconnect_tx: mpsc::unbounded_channel().0,
        },
    )])));
    let client_debug_state = Arc::new(RwLock::new(ClientDebugState::default()));
    let swarm_members = Arc::new(RwLock::new(HashMap::<String, SwarmMember>::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::<String, HashSet<String>>::new()));
    let file_touch = FileTouchService::new();
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::<String, String>::new()));
    let client_count = Arc::new(RwLock::new(1usize));
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
        working_dir_override,
        None,
        false,
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

    let events = collect_events_until_done(&mut client_event_rx, 77).await;
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, ServerEvent::Error { .. })),
        "resume of a dormant session should not error: {events:?}"
    );
    assert_eq!(client_session_id, target_session_id);

    Ok(crate::session::Session::load(target_session_id)?.working_dir)
}

#[tokio::test]
async fn resume_rewrites_a_dormant_sessions_recorded_working_dir() -> Result<()> {
    let _guard = crate::storage::lock_test_env();
    let (_runtime, prev_runtime) = setup_runtime_dir()?;

    let stored = resume_dormant_session_with_override(
        "session_dormant_workdir_overridden",
        Some(ATTACHED_FROM),
    )
    .await?;

    assert_eq!(
        stored.as_deref(),
        Some(ATTACHED_FROM),
        "resuming a dormant session from {ATTACHED_FROM} should replace the \
         directory it was created in ({CREATED_IN})"
    );

    restore_runtime_dir(prev_runtime);
    Ok(())
}

// Direction test: without an override the stored directory must survive, so the
// assertion above cannot be satisfied by unconditionally clobbering the field.
#[tokio::test]
async fn resume_without_an_override_keeps_the_recorded_working_dir() -> Result<()> {
    let _guard = crate::storage::lock_test_env();
    let (_runtime, prev_runtime) = setup_runtime_dir()?;

    let stored =
        resume_dormant_session_with_override("session_dormant_workdir_preserved", None).await?;

    assert_eq!(
        stored.as_deref(),
        Some(CREATED_IN),
        "a resume carrying no working directory must leave the recorded one alone"
    );

    restore_runtime_dir(prev_runtime);
    Ok(())
}
