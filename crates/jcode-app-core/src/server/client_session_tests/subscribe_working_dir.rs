use super::*;
use anyhow::Result;

async fn subscribe_with_working_dir(
    agent: &Arc<Mutex<Agent>>,
    session_id: &str,
    working_dir: &str,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
) {
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let registry = Registry::new(provider).await;
    let swarm_members = Arc::new(RwLock::new(HashMap::<String, SwarmMember>::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::<String, HashSet<String>>::new()));
    let swarm_plans = Arc::new(RwLock::new(HashMap::<String, VersionedPlan>::new()));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::<String, String>::new()));
    let mcp_pool = Arc::new(crate::mcp::SharedMcpPool::from_default_config());
    let event_history = Arc::new(RwLock::new(VecDeque::<SwarmEvent>::new()));
    let event_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(64);
    let mut client_selfdev = false;

    super::super::handle_subscribe(
        1,
        Some(working_dir.to_string()),
        None,
        None,
        None,
        None,
        // MCP registration spawns real servers; this test only exercises the
        // working_dir transition, so keep it off.
        false,
        &mut client_selfdev,
        session_id,
        "conn_working_dir",
        &None,
        agent,
        &registry,
        false,
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
        client_event_tx,
        &mcp_pool,
        &event_history,
        &event_counter,
        &swarm_event_tx,
    )
    .await;
}

fn working_dir_change_notice(events: &[ServerEvent]) -> Option<String> {
    events.iter().find_map(|event| match event {
        ServerEvent::Notification { message, .. }
            if message.contains("working directory changed") =>
        {
            Some(message.clone())
        }
        _ => None,
    })
}

/// R05 gate: "A reconnect that rescopes working_dir is surfaced, never silent."
///
/// In the 2026-07-20 incident a reconnecting client's Subscribe moved a
/// 13-hour-old session from /Users/jrudnik/labs/jcode to /Users/jrudnik with no
/// user-visible trace, silently changing swarm identity and every relative path
/// the session resolved. The rescope is still applied (a client may legitimately
/// reopen the session elsewhere), but both a warn log and a client-visible
/// notification must accompany it.
#[tokio::test]
async fn subscribe_warns_when_reconnect_changes_established_working_dir() -> Result<()> {
    let _guard = crate::storage::lock_test_env();

    let session_id = "session_subscribe_working_dir_change";
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let registry = Registry::new(provider.clone()).await;
    let agent = Arc::new(Mutex::new(build_test_agent_with_id(
        provider.clone(),
        registry,
        session_id,
        Vec::new(),
    )));

    let established = tempfile::TempDir::new()?;
    let established_dir = established.path().to_string_lossy().to_string();
    {
        let mut guard = agent.lock().await;
        guard.set_working_dir(&established_dir);
    }

    let reconnect_dir_tmp = tempfile::TempDir::new()?;
    let reconnect_dir = reconnect_dir_tmp.path().to_string_lossy().to_string();
    assert_ne!(established_dir, reconnect_dir);

    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();
    subscribe_with_working_dir(&agent, session_id, &reconnect_dir, &client_event_tx).await;

    let mut events = Vec::new();
    while let Ok(event) = client_event_rx.try_recv() {
        events.push(event);
    }

    let notice = working_dir_change_notice(&events).unwrap_or_else(|| {
        panic!("reconnect working_dir change must notify the client, got {events:?}")
    });
    assert!(
        notice.contains(&established_dir) && notice.contains(&reconnect_dir),
        "notice must name both directories, got {notice}"
    );

    // The rescope is still applied: refusing it would strand the client against
    // a stale directory.
    let applied = {
        let guard = agent.lock().await;
        guard.working_dir().map(str::to_string)
    };
    assert_eq!(applied.as_deref(), Some(reconnect_dir.as_str()));

    Ok(())
}

/// A reconnect that keeps the same working_dir is the common case and must stay
/// quiet: the warning has to mean something when it fires.
#[tokio::test]
async fn subscribe_is_quiet_when_working_dir_is_unchanged() -> Result<()> {
    let _guard = crate::storage::lock_test_env();

    let session_id = "session_subscribe_working_dir_same";
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let registry = Registry::new(provider.clone()).await;
    let agent = Arc::new(Mutex::new(build_test_agent_with_id(
        provider.clone(),
        registry,
        session_id,
        Vec::new(),
    )));

    let established = tempfile::TempDir::new()?;
    let established_dir = established.path().to_string_lossy().to_string();
    {
        let mut guard = agent.lock().await;
        guard.set_working_dir(&established_dir);
    }

    let (client_event_tx, mut client_event_rx) = mpsc::unbounded_channel();
    subscribe_with_working_dir(&agent, session_id, &established_dir, &client_event_tx).await;

    let mut events = Vec::new();
    while let Ok(event) = client_event_rx.try_recv() {
        events.push(event);
    }

    assert!(
        working_dir_change_notice(&events).is_none(),
        "unchanged working_dir must not warn, got {events:?}"
    );

    Ok(())
}

/// The notification is the *only* user-visible trace of the rescope: the warn
/// above it goes to the log, not the client. Discarding a failed send with
/// `let _ =` therefore reproduces exactly the silent move this module exists to
/// prevent, just one layer down. When the client is already gone, the loss of
/// that trace must itself be recorded.
///
/// Regression test for the R05 follow-up: adjudicated 2026-07-29.
#[tokio::test]
async fn subscribe_records_when_the_working_dir_notice_cannot_be_delivered() -> Result<()> {
    let _guard = crate::storage::lock_test_env();

    // Redirect the log so this test reads its own process-global output.
    let home = tempfile::TempDir::new()?;
    let previous_home = std::env::var("JCODE_HOME").ok();
    jcode_core::env::set_var("JCODE_HOME", home.path());
    crate::logging::init();

    // NOTE: deliberately does NOT contain the asserted phase string; an earlier
    // draft used one that did, and the assertion matched its own fixture name.
    let session_id = "session_subscribe_wd_undeliverable";
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let registry = Registry::new(provider.clone()).await;
    let agent = Arc::new(Mutex::new(build_test_agent_with_id(
        provider.clone(),
        registry,
        session_id,
        Vec::new(),
    )));

    let established = tempfile::TempDir::new()?;
    let established_dir = established.path().to_string_lossy().to_string();
    {
        let mut guard = agent.lock().await;
        guard.set_working_dir(&established_dir);
    }

    let reconnect_dir_tmp = tempfile::TempDir::new()?;
    let reconnect_dir = reconnect_dir_tmp.path().to_string_lossy().to_string();
    assert_ne!(established_dir, reconnect_dir);

    // Drop the receiver first: this is the "client already disconnected" case,
    // which is the only way an unbounded send can fail.
    let (client_event_tx, client_event_rx) = mpsc::unbounded_channel();
    drop(client_event_rx);

    subscribe_with_working_dir(&agent, session_id, &reconnect_dir, &client_event_tx).await;

    // The rescope is still applied even though nobody could be told.
    let applied = {
        let guard = agent.lock().await;
        guard.working_dir().map(str::to_string)
    };
    assert_eq!(applied.as_deref(), Some(reconnect_dir.as_str()));

    let logs = home.path().join("logs");
    let mut found = String::new();
    if let Ok(entries) = std::fs::read_dir(&logs) {
        for entry in entries.flatten() {
            if let Ok(text) = std::fs::read_to_string(entry.path()) {
                found.push_str(&text);
            }
        }
    }

    match previous_home {
        Some(value) => jcode_core::env::set_var("JCODE_HOME", value),
        None => jcode_core::env::remove_var("JCODE_HOME"),
    }

    assert!(
        found.contains("phase=subscribe_working_dir_notify_failed"),
        "an undeliverable working_dir notice must be recorded, not swallowed; log was: {found}"
    );
    Ok(())
}
