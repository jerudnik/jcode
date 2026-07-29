#![cfg_attr(test, allow(clippy::await_holding_lock))]

#[cfg(unix)]
use super::socket::{
    daemon_lock_path, detach_into_new_session, server_start_matches_existing_server,
    try_acquire_daemon_lock,
};
use super::socket::{endpoint_artifacts, sibling_socket_path};
use super::{
    ReloadPhase, ReloadState, ReloadWaitStatus, await_reload_handoff, cleanup_socket_pair,
    clear_reload_marker, inspect_reload_wait_status, publish_reload_socket_ready,
    reload_marker_active, reload_marker_path, reload_process_alive, write_reload_state,
};
#[cfg(unix)]
use super::{connect_socket, reap_stale_socket_if_dead};
#[cfg(unix)]
use crate::transport::Listener;
use std::time::Duration;

#[test]
fn sibling_socket_path_roundtrip() {
    let main = std::path::PathBuf::from("/tmp/jcode.sock");
    let debug = std::path::PathBuf::from("/tmp/jcode-debug.sock");

    assert_eq!(sibling_socket_path(&main), Some(debug.clone()));
    assert_eq!(sibling_socket_path(&debug), Some(main));
}

#[test]
fn cleanup_socket_pair_removes_main_and_debug_files() {
    let stamp = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let dir = std::env::temp_dir();
    let main = dir.join(format!("jcode-test-{}.sock", stamp));
    let debug = dir.join(format!("jcode-test-{}-debug.sock", stamp));

    std::fs::write(&main, b"").expect("create main socket placeholder");
    std::fs::write(&debug, b"").expect("create debug socket placeholder");

    cleanup_socket_pair(&main);

    assert!(!main.exists(), "main socket file should be removed");
    assert!(!debug.exists(), "debug socket file should be removed");
}

#[cfg(unix)]
fn write_endpoint_sidecars(socket: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let artifacts = endpoint_artifacts(socket);
    if !artifacts.main_socket.exists() {
        std::fs::write(&artifacts.main_socket, b"main-sidecar-matrix").expect("write main");
    }
    if !artifacts.debug_socket.exists() {
        std::fs::write(&artifacts.debug_socket, b"debug-sidecar-matrix").expect("write debug");
    }
    let entries = vec![
        (artifacts.hash, b"hash-sidecar-matrix".to_vec()),
        (
            artifacts.temporary_metadata,
            b"metadata-sidecar-matrix".to_vec(),
        ),
        (artifacts.daemon_lock, b"lock-sidecar-matrix".to_vec()),
    ];
    for (path, bytes) in &entries {
        std::fs::write(path, bytes).expect("write endpoint sidecar");
    }
    entries
}

#[cfg(unix)]
fn write_endpoint_metadata_sidecars(
    socket: &std::path::Path,
) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let artifacts = endpoint_artifacts(socket);
    let entries = vec![
        (artifacts.hash, b"hash-sidecar-matrix".to_vec()),
        (
            artifacts.temporary_metadata,
            b"metadata-sidecar-matrix".to_vec(),
        ),
        (artifacts.daemon_lock, b"lock-sidecar-matrix".to_vec()),
    ];
    for (path, bytes) in &entries {
        std::fs::write(path, bytes).expect("write endpoint metadata sidecar");
    }
    entries
}

#[cfg(unix)]
fn assert_endpoint_bytes(entries: &[(std::path::PathBuf, Vec<u8>)]) {
    for (path, expected) in entries {
        assert_eq!(
            std::fs::read(path).unwrap_or_default(),
            *expected,
            "{} bytes must be preserved exactly",
            path.display()
        );
    }
}

#[cfg(unix)]
fn restore_runtime(prev_runtime: Option<std::ffi::OsString>) {
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn connect_socket_preserves_refused_socket_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("jcode.sock");

    {
        let _listener = Listener::bind(&socket_path).expect("bind listener");
    }

    assert!(
        socket_path.exists(),
        "listener drop should leave the socket path behind for stale-socket checks"
    );

    // On macOS, a connect can briefly succeed from the socket's pending queue
    // after the listening fd is closed. Wait until the kernel exposes the
    // stable stale-socket state that this regression is intended to exercise.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let err = loop {
        match connect_socket(&socket_path).await {
            Ok(stream) => {
                drop(stream);
                assert!(
                    std::time::Instant::now() < deadline,
                    "socket kept accepting connections after listener teardown"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(err) => break err,
        }
    };
    assert!(
        err.to_string().contains("refused the connection"),
        "unexpected error: {err:#}"
    );
    assert!(
        socket_path.exists(),
        "connect_socket should not unlink the socket path on connection refusal"
    );
}

#[cfg(unix)]
#[test]
fn daemon_lock_serializes_server_processes() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    let lock_path = daemon_lock_path();
    let first = try_acquire_daemon_lock(&lock_path)
        .expect("acquire first daemon lock")
        .expect("first daemon lock should succeed");
    let second = try_acquire_daemon_lock(&lock_path).expect("acquire second daemon lock");
    assert!(second.is_none(), "second daemon lock should fail");
    drop(first);

    let third = try_acquire_daemon_lock(&lock_path)
        .expect("acquire third daemon lock")
        .expect("third daemon lock should succeed after release");
    drop(third);

    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn reap_stale_socket_removes_dead_socket_pair_and_lock() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    let socket = temp.path().join("jcode.sock");
    let debug = temp.path().join("jcode-debug.sock");
    let lock = daemon_lock_path();

    // Simulate the post-upgrade/crash state: socket + debug + lock files left
    // behind, but no process is listening on the socket.
    std::fs::write(&socket, b"").expect("write stale socket");
    std::fs::write(&debug, b"").expect("write stale debug socket");
    std::fs::write(&lock, b"").expect("write stale lock");

    let reaped = reap_stale_socket_if_dead(&socket).await;
    assert!(reaped, "a dead socket with no listener should be reaped");
    assert!(!socket.exists(), "stale socket should be removed");
    assert!(!debug.exists(), "stale debug socket should be removed");
    assert!(!lock.exists(), "stale daemon lock should be removed");

    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn stale_reap_sidecar_matrix_removes_only_after_listener_lock_listener_proof() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    let prev_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());
    crate::env::set_var("JCODE_HOME", temp.path().join("home"));

    let socket = temp.path().join("jcode.sock");
    let stale_entries = write_endpoint_sidecars(&socket);
    let mut registry = crate::registry::ServerRegistry::default();
    registry.register(crate::registry::ServerInfo {
        id: "server-stale-f25".to_string(),
        name: "stale-f25".to_string(),
        icon: "x".to_string(),
        socket: socket.clone(),
        debug_socket: endpoint_artifacts(&socket).debug_socket,
        git_hash: "dead".to_string(),
        version: "test".to_string(),
        pid: u32::MAX,
        started_at: "1970-01-01T00:00:00Z".to_string(),
        sessions: Vec::new(),
    });
    registry.save().await.expect("seed stale registry entry");
    assert!(
        reap_stale_socket_if_dead(&socket).await,
        "dead listener + free lock should reap"
    );
    for (path, _) in &stale_entries {
        assert!(
            !path.exists(),
            "stale endpoint artifact should be removed: {}",
            path.display()
        );
    }
    let registry = crate::registry::ServerRegistry::load()
        .await
        .expect("reload registry after stale reap");
    assert!(
        registry.find_by_name("stale-f25").is_none(),
        "stale ownership proof must also remove the dead registry entry"
    );

    let live_entries = write_endpoint_metadata_sidecars(&socket);
    let listener = Listener::bind(&socket).expect("bind live listener");
    assert!(
        !reap_stale_socket_if_dead(&socket).await,
        "live listener must block reap"
    );
    drop(listener);
    assert_endpoint_bytes(&live_entries);
    assert!(socket.exists(), "live socket path must be preserved");

    let held_entries = write_endpoint_sidecars(&socket);
    let held = try_acquire_daemon_lock(&daemon_lock_path())
        .expect("acquire daemon lock")
        .expect("daemon lock free");
    assert!(
        !reap_stale_socket_if_dead(&socket).await,
        "held lock must block reap"
    );
    assert_endpoint_bytes(&held_entries);
    assert!(socket.exists(), "held-lock socket path must be preserved");
    drop(held);

    cleanup_socket_pair(&socket);
    let reload_artifacts = endpoint_artifacts(&socket);
    assert!(
        !reload_artifacts.main_socket.exists(),
        "reload removes main endpoint"
    );
    assert!(
        !reload_artifacts.debug_socket.exists(),
        "reload removes debug endpoint"
    );
    assert!(
        reload_artifacts.hash.exists(),
        "reload must preserve hash metadata"
    );
    assert!(
        reload_artifacts.temporary_metadata.exists(),
        "reload must preserve server metadata"
    );

    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
    restore_runtime(prev_runtime);
}

#[cfg(unix)]
#[tokio::test]
async fn reap_stale_socket_spares_live_listener() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    let socket = temp.path().join("jcode.sock");
    // A live listener means a daemon is bound; reaping must be a no-op.
    let listener = Listener::bind(&socket).expect("bind listener");

    let reaped = reap_stale_socket_if_dead(&socket).await;
    assert!(!reaped, "a live listener must never be reaped");
    assert!(socket.exists(), "live socket must be left intact");

    drop(listener);
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn reap_stale_socket_spares_socket_when_lock_is_held() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    let socket = temp.path().join("jcode.sock");
    std::fs::write(&socket, b"").expect("write stale-looking socket");

    // Hold the daemon lock, emulating a live daemon whose socket probe happens
    // to be momentarily unanswerable. The reaper must not unlink the socket.
    let lock_path = daemon_lock_path();
    let held = try_acquire_daemon_lock(&lock_path)
        .expect("acquire lock")
        .expect("lock should be free");

    let reaped = reap_stale_socket_if_dead(&socket).await;
    assert!(
        !reaped,
        "socket must be spared while the daemon lock is held"
    );
    assert!(
        socket.exists(),
        "socket must be left intact while lock is held"
    );

    drop(held);
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[cfg(unix)]
#[test]
fn existing_server_start_errors_are_detected() {
    assert!(server_start_matches_existing_server(
        "Error: Another jcode server process is already running for runtime dir /run/user/1000"
    ));
    assert!(server_start_matches_existing_server(
        "Error: Refusing to replace active server socket at /run/user/1000/jcode.sock"
    ));
    assert!(!server_start_matches_existing_server(
        "Error: failed to bind socket: permission denied"
    ));
}

#[test]
fn reload_marker_active_expires_stale_marker() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    let marker = reload_marker_path();
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    write_reload_state("test-request", "test-hash", ReloadPhase::Starting, None);
    assert!(reload_marker_active(Duration::from_secs(30)));
    std::thread::sleep(Duration::from_millis(5));
    assert!(!reload_marker_active(Duration::ZERO));
    assert!(!marker.exists(), "stale reload marker should be cleaned up");

    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[test]
fn reload_marker_active_for_recent_socket_ready_marker() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    write_reload_state("test-request", "test-hash", ReloadPhase::SocketReady, None);
    assert!(reload_marker_active(Duration::from_secs(30)));

    clear_reload_marker();
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[test]
fn publish_reload_socket_ready_updates_current_process_marker() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    write_reload_state(
        "test-request",
        "test-hash",
        ReloadPhase::Starting,
        Some("detail".to_string()),
    );
    publish_reload_socket_ready();

    let state = ReloadState::load().expect("reload state should exist");
    assert_eq!(state.phase, ReloadPhase::SocketReady);
    assert_eq!(state.request_id, "test-request");
    assert_eq!(state.hash, "test-hash");
    assert_eq!(state.detail.as_deref(), Some("detail"));

    clear_reload_marker();
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[test]
fn publish_reload_socket_ready_clears_marker_for_foreign_pid() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    ReloadState {
        request_id: "test-request".to_string(),
        hash: "test-hash".to_string(),
        phase: ReloadPhase::Starting,
        pid: std::process::id().saturating_add(1_000_000),
        timestamp: chrono::Utc::now().to_rfc3339(),
        detail: None,
        runtime_identity: None,
    }
    .write();

    publish_reload_socket_ready();
    assert!(
        ReloadState::load().is_none(),
        "foreign reload marker should be cleared"
    );

    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[tokio::test]
async fn inspect_reload_wait_status_reports_ready_for_socket_ready_marker() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    write_reload_state("test-request", "test-hash", ReloadPhase::SocketReady, None);

    let socket_path = temp.path().join("missing.sock");
    let status = inspect_reload_wait_status(&socket_path, Duration::from_secs(30), None).await;
    assert_eq!(status, ReloadWaitStatus::Ready);

    clear_reload_marker();
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn inspect_reload_wait_status_keeps_waiting_while_starting_marker_is_active_even_if_socket_is_live()
 {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    ReloadState {
        request_id: "test-request".to_string(),
        hash: "test-hash".to_string(),
        phase: ReloadPhase::Starting,
        pid: std::process::id(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        detail: None,
        runtime_identity: None,
    }
    .write();

    let socket_path = temp.path().join("jcode.sock");
    let _listener = Listener::bind(&socket_path).expect("bind listener");

    let status = inspect_reload_wait_status(&socket_path, Duration::from_secs(30), None).await;
    assert_eq!(
        status,
        ReloadWaitStatus::Waiting {
            pid: Some(std::process::id())
        }
    );

    clear_reload_marker();
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[tokio::test]
async fn wait_for_reload_handoff_event_returns_promptly_when_no_event_arrives() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    let socket_path = temp.path().join("missing.sock");
    let started = std::time::Instant::now();
    crate::server::wait_for_reload_handoff_event(Some(std::process::id()), &socket_path).await;
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "reload handoff event wait should be a bounded edge wait, not an indefinite block"
    );

    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[tokio::test]
async fn inspect_reload_wait_status_reports_idle_without_marker_or_listener() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("missing.sock");

    let status = inspect_reload_wait_status(&socket_path, Duration::from_secs(30), None).await;
    assert_eq!(status, ReloadWaitStatus::Idle);
}

#[tokio::test]
async fn inspect_reload_wait_status_uses_last_known_pid_when_marker_missing() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("missing.sock");

    let status = inspect_reload_wait_status(
        &socket_path,
        Duration::from_secs(30),
        Some(std::process::id()),
    )
    .await;
    assert_eq!(
        status,
        ReloadWaitStatus::Waiting {
            pid: Some(std::process::id())
        }
    );
}

#[tokio::test]
async fn inspect_reload_wait_status_reports_failed_when_reload_pid_is_dead() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());
    let dead_pid = std::process::id().saturating_add(1_000_000);
    assert!(
        !reload_process_alive(dead_pid),
        "test requires a definitely-dead pid"
    );

    ReloadState {
        request_id: "test-request".to_string(),
        hash: "test-hash".to_string(),
        phase: ReloadPhase::Starting,
        pid: dead_pid,
        timestamp: chrono::Utc::now().to_rfc3339(),
        detail: None,
        runtime_identity: None,
    }
    .write();

    let socket_path = temp.path().join("missing.sock");
    let status = inspect_reload_wait_status(&socket_path, Duration::from_secs(30), None).await;
    assert!(matches!(status, ReloadWaitStatus::Failed(Some(_))));

    clear_reload_marker();
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[tokio::test]
async fn await_reload_handoff_returns_ready_after_marker_transition() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    ReloadState {
        request_id: "test-request".to_string(),
        hash: "test-hash".to_string(),
        phase: ReloadPhase::Starting,
        pid: std::process::id(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        detail: None,
        runtime_identity: None,
    }
    .write();

    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        write_reload_state("test-request", "test-hash", ReloadPhase::SocketReady, None);
    });

    let socket_path = temp.path().join("missing.sock");
    let status = tokio::time::timeout(
        Duration::from_secs(2),
        await_reload_handoff(&socket_path, Duration::from_secs(30)),
    )
    .await
    .expect("await reload handoff should finish");
    assert_eq!(status, ReloadWaitStatus::Ready);

    clear_reload_marker();
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[tokio::test]
async fn await_reload_handoff_returns_failed_after_marker_transition() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());

    ReloadState {
        request_id: "test-request".to_string(),
        hash: "test-hash".to_string(),
        phase: ReloadPhase::Starting,
        pid: std::process::id(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        detail: None,
        runtime_identity: None,
    }
    .write();

    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        write_reload_state(
            "test-request",
            "test-hash",
            ReloadPhase::Failed,
            Some("boom".to_string()),
        );
    });

    let socket_path = temp.path().join("missing.sock");
    let status = tokio::time::timeout(
        Duration::from_secs(2),
        await_reload_handoff(&socket_path, Duration::from_secs(30)),
    )
    .await
    .expect("await reload handoff should finish");
    assert_eq!(status, ReloadWaitStatus::Failed(Some("boom".to_string())));

    clear_reload_marker();
    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

/// The daemon must detach into its own session, and a `setsid()` that fails
/// must abort the spawn instead of quietly leaving the server in the caller's
/// process group (where later `kill(-pid, ...)` shutdowns report ESRCH).
#[cfg(unix)]
#[test]
fn detach_into_new_session_failure_aborts_spawn() {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    // Happy path: the child leads its own process group, so a process-group
    // signal can still reach helper descendants it spawns.
    let mut ok_cmd = Command::new("/bin/sleep");
    ok_cmd.arg("30").stdout(Stdio::null()).stderr(Stdio::null());
    unsafe {
        ok_cmd.pre_exec(detach_into_new_session);
    }
    let mut child = ok_cmd.spawn().expect("detaching spawn should succeed");
    let child_pid = child.id();
    assert_eq!(
        unsafe { libc::getpgid(child_pid as i32) },
        child_pid as i32,
        "detached child should lead its own process group"
    );
    let _ = child.kill();
    let _ = child.wait();

    // Failure path: `setsid()` returns EPERM for a process that already leads a
    // group, so the first call wins and the second must fail the spawn.
    let mut failing_cmd = Command::new("/bin/sleep");
    failing_cmd
        .arg("30")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        failing_cmd.pre_exec(|| {
            detach_into_new_session()?;
            detach_into_new_session()
        });
    }
    let err = failing_cmd
        .spawn()
        .expect_err("a failed setsid() must surface as a spawn error");
    assert_eq!(
        err.raw_os_error(),
        Some(libc::EPERM),
        "spawn error should carry the setsid() errno, got {err}"
    );
}
