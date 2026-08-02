//! R05-FIX-1 regression: a cancel must not be reported as a server reload.
//!
//! Split out of `agent_tests.rs` to keep that file under the test-size budget.

use super::*;

/// R05-FIX-1: `Request::Cancel` carries no cause, and the server registers the
/// agent's graceful-shutdown signal as `SessionControlHandle`'s
/// `stop_current_turn_signal` (client_lifecycle.rs:582-595). So a user or
/// stall-guard cancel sets the very bit a server reload sets. Before the fix,
/// every downstream reader could only ask "is it set?", and answered
/// "server reload", telling the model its work would resume across a restart
/// that was never going to happen.
#[tokio::test]
async fn r05_user_cancel_is_not_labelled_a_server_reload() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("temp JCODE_HOME");
    let _home = ScopedEnvVar::set("JCODE_HOME", temp.path());
    let _telemetry = ScopedEnvVar::set("JCODE_NO_TELEMETRY", "1");

    let (polled_tx, _polled_rx) = oneshot::channel();
    let agent = mid_stream_cancel_agent(polled_tx).await;

    // Built exactly as the server builds it, so this exercises the real wiring
    // rather than a hand-rolled signal.
    let control = crate::server::SessionControlHandle::new(
        agent.session_id().to_string(),
        agent.soft_interrupt_queue(),
        agent.background_tool_signal(),
        agent.graceful_shutdown_signal(),
    );

    // What Request::Cancel does: cancel_processing_message -> request_cancel.
    control.request_cancel();

    assert!(
        agent.graceful_shutdown_signal().is_set(),
        "a cancel must still interrupt the turn"
    );
    assert!(
        !agent.graceful_shutdown_signal().is_server_reload(),
        "R05: a plain cancel was reported as a server reload"
    );

    // A real reload must still be labelled a reload.
    agent.request_graceful_shutdown();
    assert!(
        agent.graceful_shutdown_signal().is_server_reload(),
        "a genuine reload must keep its reload label"
    );
}
