use super::*;

struct EnvVarRestore {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarRestore {
    fn set(key: &'static str, value: &std::path::Path) -> Self {
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

async fn test_agent() -> Agent {
    let provider: Arc<dyn Provider> = Arc::new(NativeAutoCompactionProvider);
    let registry = Registry::new(provider.clone()).await;
    Agent::new(provider, registry)
}

fn marker_exists(session_id: &str) -> bool {
    crate::storage::observe_session_pid_markers(session_id)
        .active
        .is_some()
}

#[tokio::test]
async fn new_agent_registers_active_pid_and_clear_swaps_it() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home = EnvVarRestore::set("JCODE_HOME", temp_home.path());
    let mut agent = test_agent().await;

    let first_session_id = agent.session_id().to_string();
    assert!(marker_exists(&first_session_id));

    agent.clear();

    let second_session_id = agent.session_id().to_string();
    assert_ne!(first_session_id, second_session_id);
    assert!(marker_exists(&second_session_id));
    assert!(!marker_exists(&first_session_id));

    drop(agent);
    assert!(!marker_exists(&second_session_id));

    let successor_test_agent = test_agent().await;
    let successor_session_id = successor_test_agent.session_id().to_string();
    crate::storage::register_active_pid(&successor_session_id, std::process::id());
    drop(successor_test_agent);
    assert!(
        marker_exists(&successor_session_id),
        "Agent drop must preserve a marker replaced by a successor"
    );
    crate::storage::unregister_active_pid(&successor_session_id);
}

#[tokio::test]
async fn agent_drop_tracks_client_pid_and_restore_marker_rewrites() {
    let _guard = crate::storage::lock_test_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home = EnvVarRestore::set("JCODE_HOME", temp_home.path());

    let mut client_agent = test_agent().await;
    let client_session_id = client_agent.session_id().to_string();
    client_agent.mark_active_with_client_pid(std::process::id());
    drop(client_agent);
    assert!(
        !marker_exists(&client_session_id),
        "Agent drop must clean the marker rewritten with a client PID"
    );

    let mut successor_client_agent = test_agent().await;
    let successor_client_session_id = successor_client_agent.session_id().to_string();
    successor_client_agent.mark_active_with_client_pid(std::process::id());
    crate::storage::register_active_pid(&successor_client_session_id, std::process::id());
    drop(successor_client_agent);
    assert!(
        marker_exists(&successor_client_session_id),
        "Agent drop must preserve a successor after a client-PID rewrite"
    );
    crate::storage::unregister_active_pid(&successor_client_session_id);

    let mut restored_session = crate::session::Session::create_with_id(
        "session_restore_marker_ownership".to_string(),
        None,
        None,
    );
    restored_session.save().expect("save restored session");

    let mut restore_agent = test_agent().await;
    let original_session_id = restore_agent.session_id().to_string();
    restore_agent
        .restore_session(&restored_session.id)
        .expect("restore session");
    assert!(!marker_exists(&original_session_id));
    assert!(marker_exists(&restored_session.id));

    drop(restore_agent);
    assert!(
        !marker_exists(&restored_session.id),
        "Agent drop must clean the restored session marker"
    );

    let mut successor_restore_agent = test_agent().await;
    successor_restore_agent
        .restore_session(&restored_session.id)
        .expect("restore session for successor check");
    crate::storage::register_active_pid(&restored_session.id, std::process::id());
    drop(successor_restore_agent);
    assert!(
        marker_exists(&restored_session.id),
        "Agent drop must preserve a successor after a restore marker rewrite"
    );
    crate::storage::unregister_active_pid(&restored_session.id);
}
