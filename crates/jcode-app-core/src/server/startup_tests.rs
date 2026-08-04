#![cfg_attr(test, allow(clippy::await_holding_lock))]

use super::runtime::ServerRuntime;
use super::socket::wait_for_existing_server;
use super::{Client, Server, is_server_ready};
use crate::message::{Message, ToolDefinition};
use crate::provider::{EventStream, Provider};
use crate::transport::Listener;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

struct TestProvider;

#[async_trait]
impl Provider for TestProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        Err(anyhow::anyhow!(
            "test provider complete should not be called in startup tests"
        ))
    }

    fn name(&self) -> &str {
        "test"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(TestProvider)
    }
}

#[tokio::test]
async fn server_run_refuses_to_replace_live_socket() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_runtime = std::env::var_os("JCODE_RUNTIME_DIR");
    crate::env::set_var("JCODE_RUNTIME_DIR", temp.path());
    let socket_path = temp.path().join("jcode.sock");
    let debug_socket_path = temp.path().join("jcode-debug.sock");
    let _listener = Listener::bind(&socket_path).expect("bind existing live socket");
    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let server = Server::new_with_paths(provider, socket_path, debug_socket_path);

    let error = server
        .run()
        .await
        .expect_err("should refuse live socket takeover");
    assert!(
        error
            .to_string()
            .contains("Refusing to replace active server socket"),
        "unexpected error: {error:#}"
    );

    if let Some(prev_runtime) = prev_runtime {
        crate::env::set_var("JCODE_RUNTIME_DIR", prev_runtime);
    } else {
        crate::env::remove_var("JCODE_RUNTIME_DIR");
    }
}

#[tokio::test]
async fn is_server_ready_returns_false_immediately_for_missing_socket() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("missing.sock");

    let ready = tokio::time::timeout(Duration::from_millis(50), is_server_ready(&socket_path))
        .await
        .expect("missing socket probe should return quickly");

    assert!(!ready, "missing socket should not report ready");
}

#[tokio::test]
async fn wait_for_existing_server_tolerates_delayed_listener() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("jcode.sock");
    let bind_path = socket_path.clone();

    let bind_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let listener = Listener::bind(&bind_path).expect("bind delayed listener");
        tokio::time::sleep(Duration::from_millis(200)).await;
        drop(listener);
    });

    let ready = wait_for_existing_server(&socket_path, Duration::from_secs(1)).await;
    assert!(ready, "delayed live listener should be detected");

    bind_task.await.expect("bind task should complete");
}

#[test]
fn server_initializes_schedule_runner_even_when_ambient_disabled() {
    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let server = Server::new(provider);

    assert!(
        server.ambient_runner.is_some(),
        "schedule/session tasks need the runner even when ambient is disabled"
    );
}

#[tokio::test]
async fn debug_accept_loop_responds_to_ping_without_affecting_client_count() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("jcode.sock");
    let debug_socket_path = temp.path().join("jcode-debug.sock");
    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let server = Server::new_with_paths(provider, socket_path, debug_socket_path.clone());
    let runtime = ServerRuntime::from_server(&server);
    let debug_listener = Listener::bind(&debug_socket_path).expect("bind debug socket");
    let debug_handle = runtime.spawn_debug_accept_loop(debug_listener, std::time::Instant::now());

    let mut client = tokio::time::timeout(
        Duration::from_secs(1),
        Client::connect_debug_with_path(debug_socket_path),
    )
    .await
    .expect("debug connect should complete")
    .expect("debug client should connect");

    assert!(client.ping().await.expect("debug ping should succeed"));
    assert_eq!(*server.client_count.read().await, 0);

    tokio::time::timeout(Duration::from_secs(1), runtime.shutdown())
        .await
        .expect("runtime shutdown should join debug connection tasks");
    tokio::time::timeout(Duration::from_secs(1), debug_handle)
        .await
        .expect("debug accept loop should observe runtime cancellation")
        .expect("debug accept loop should exit cleanly");
}

/// Poll rather than sleep a fixed span: the accept loop is concurrent, so a
/// fixed sleep either flakes or wastes time depending on the machine.
async fn settled(runner: &crate::ambient_runner::AmbientRunnerHandle, want: u64) -> bool {
    for _ in 0..100 {
        let seen = serde_json::from_str::<serde_json::Value>(&runner.status_json().await)
            .expect("status_json emits valid JSON")
            .get("active_user_sessions")
            .and_then(|v| v.as_u64())
            .expect("status exposes active_user_sessions");
        if seen == want {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

/// The ambient runner must see a real user connect without anyone telling it.
///
/// This drives the production seam end to end: a `Server` (whose constructor is
/// the only production caller that supplies the counter), its real main accept
/// loop, and an actual `Client::connect`. It then reads the runner back through
/// `status_json`, the surface the debug socket serves. It never writes the
/// runner's own field, because a test that writes the value it later reads
/// would pass with the two sides completely disconnected.
#[tokio::test]
async fn ambient_runner_observes_user_clients_connecting_and_leaving() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = crate::storage::EnvVarGuard::set("JCODE_HOME", temp.path());

    let socket_path = temp.path().join("jcode.sock");
    let provider: Arc<dyn Provider> = Arc::new(TestProvider);
    let server = Server::new_with_paths(
        provider,
        socket_path.clone(),
        temp.path().join("jcode-debug.sock"),
    );
    let runner = server
        .ambient_runner
        .clone()
        .expect("server always builds a runner, even with ambient disabled");

    let runtime = ServerRuntime::from_server(&server);
    let listener = Listener::bind(&socket_path).expect("bind main socket");
    let accept = runtime.spawn_main_accept_loop(listener);

    let active_sessions = |json: String| -> u64 {
        serde_json::from_str::<serde_json::Value>(&json)
            .expect("status_json emits valid JSON")
            .get("active_user_sessions")
            .and_then(|v| v.as_u64())
            .expect("status exposes active_user_sessions")
    };

    assert_eq!(
        active_sessions(runner.status_json().await),
        0,
        "no clients have connected yet"
    );

    let client = tokio::time::timeout(
        Duration::from_secs(5),
        Client::connect_with_path(socket_path.clone()),
    )
    .await
    .expect("connect should not hang")
    .expect("client should connect");

    assert!(
        settled(&runner, 1).await,
        "a connected user client must be visible to the ambient runner"
    );

    drop(client);
    assert!(
        settled(&runner, 0).await,
        "the runner must also observe the user leaving, or it pauses forever"
    );

    accept.abort();
    let _ = accept.await;
}
