use super::*;

struct EnvVarRestore {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarRestore {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarRestore {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            crate::env::set_var(self.key, previous);
        } else {
            crate::env::remove_var(self.key);
        }
    }
}

#[tokio::test]
async fn handle_comm_spawn_auto_fallback_preserves_history_and_detail_with_prompt() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home = EnvVarRestore::set("JCODE_HOME", temp_home.path());
    let _visible_error =
        EnvVarRestore::set("JCODE_TEST_VISIBLE_SPAWN_ERROR", "terminal unavailable");

    let swarm_id = "swarm-auto-fallback".to_string();
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    sessions.write().await.insert(
        "coord".to_string(),
        test_agent_with_working_dir("coord", temp_home.path().to_str().expect("utf8 temp home"))
            .await,
    );
    let (mut coord, _coord_events) = member("coord", Some(&swarm_id), "coordinator");
    coord.friendly_name = Some("coordinator".to_string());
    let swarm_members = Arc::new(RwLock::new(HashMap::from([("coord".to_string(), coord)])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        HashSet::from(["coord".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.clone(),
        "coord".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::new()));
    let event_history = Arc::new(RwLock::new(VecDeque::new()));
    let event_counter = Arc::new(AtomicU64::new(0));
    let (swarm_event_tx, mut swarm_event_rx) = broadcast::channel(8);
    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let global_session_id = Arc::new(RwLock::new("coord".to_string()));
    let mcp_pool = Arc::new(crate::mcp::SharedMcpPool::from_default_config());
    let soft_interrupt_queues = Arc::new(RwLock::new(HashMap::new()));
    let client_connections = Arc::new(RwLock::new(HashMap::new()));
    let mutation_runtime = SwarmMutationRuntime::default();

    handle_comm_spawn(
        77,
        "coord".to_string(),
        Some(temp_home.path().display().to_string()),
        Some("audit the fallback path".to_string()),
        Some("nonce-auto-fallback".to_string()),
        Some(SwarmSpawnMode::Auto),
        None,
        None,
        Some("fallback worker".to_string()),
        None,
        &client_event_tx,
        &sessions,
        &global_session_id,
        &provider,
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
        &event_history,
        &event_counter,
        &swarm_event_tx,
        &mcp_pool,
        &soft_interrupt_queues,
        &mutation_runtime,
        &client_connections,
    )
    .await;

    let response = client_event_rx.recv().await.expect("spawn response");
    let new_session_id = match response {
        ServerEvent::CommSpawnResponse {
            id,
            new_session_id,
            initial_prompt_delivered,
            ..
        } => {
            assert_eq!(id, 77);
            assert!(initial_prompt_delivered);
            new_session_id
        }
        other => panic!("expected spawn response, got {other:?}"),
    };

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let members = swarm_members.read().await;
            if let Some(member) = members.get(&new_session_id)
                && member
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("terminal unavailable"))
            {
                assert_eq!(member.initial_prompt_delivered, Some(true));
                assert!(member.is_headless);
                let detail = member.detail.as_deref().expect("detail");
                assert!(detail.contains("requested Auto -> resolved Headless"));
                assert!(detail.contains("terminal unavailable"));
                break;
            }
            drop(members);
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("fallback detail should survive prompt status updates");

    let history = event_history.read().await;
    assert!(history.iter().any(|event| {
        event.session_id == new_session_id
            && matches!(event.event, SwarmEventType::MemberChange { ref action } if action == "joined")
    }));
    drop(history);

    let live_event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = swarm_event_rx.recv().await.expect("swarm event");
            if event.session_id == new_session_id
                && matches!(event.event, SwarmEventType::MemberChange { ref action } if action == "joined")
            {
                break true;
            }
        }
    })
    .await
    .expect("swarm event stream should carry the fallback-created member join");
    assert!(live_event);
}
