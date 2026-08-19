// Ownership: does each dormant session keep the directory recorded when the
// client later resumes it?
//
// `Request::ResumeSession` carries no working directory, so the server has to
// source one. `client_lifecycle.rs` has two resume call sites and they source
// it differently. The subscribe-time site passes the directory the client
// declared. The in-session site must pass no override because the resumed
// session owns its recorded directory.
//
// This test drives the real server loop — `handle_client` over a socket pair,
// the same seam `client_lifecycle_tests.rs` uses — rather than replicating the
// sourcing line in test code, so removing that line makes the test fail. Three
// sentinels, none of which can coincide with each other or with anything else
// in the harness.

use crate::protocol::ServerEvent;
use crate::server::await_members_state::AwaitMembersRuntime;
use crate::server::client_lifecycle::handle_client;
use crate::server::swarm_mutation_state::SwarmMutationRuntime;
use crate::server::{
    ClientDebugState, FileTouchService, SessionAgents, SwarmEventState, SwarmState,
};
use jcode_protocol::Request;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc};

const PROP_CLIENT_STARTED_IN: &str = "/sentinel/client/started/in";
const PROP_A_ROOTED_AT: &str = "/sentinel/first/resumed/rooted/at";
const PROP_B_CREATED_IN: &str = "/sentinel/second/resumed/created/in";

/// Persist a dormant session with a recorded working directory.
fn prop_seed_dormant(session_id: &str, working_dir: &str) -> Result<()> {
    let mut persisted = crate::session::Session::create_with_id(
        session_id.to_string(),
        None,
        Some("Propagation Fixture".to_string()),
    );
    persisted.working_dir = Some(working_dir.to_string());
    persisted.save()?;
    Ok(())
}

/// The working directory currently recorded on disk for a session.
fn prop_stored_dir(session_id: &str) -> Option<String> {
    crate::session::Session::load(session_id)
        .ok()
        .and_then(|s| s.working_dir)
}

fn prop_subscribe(working_dir: &str) -> Request {
    Request::Subscribe {
        id: 1,
        working_dir: Some(working_dir.to_string()),
        selfdev: None,
        target_session_id: None,
        client_instance_id: None,
        client_has_local_history: false,
        allow_session_takeover: true,
        terminal_env: Vec::new(),
        protocol_version: None,
        build_hash: None,
        runtime_identity: None,
        spawn_swarm_id: None,
        spawn_session_id: None,
        client_pid: None,
    }
}

fn prop_resume(id: u64, session_id: &str) -> Request {
    Request::ResumeSession {
        id,
        session_id: session_id.to_string(),
        client_instance_id: None,
        client_has_local_history: false,
        allow_session_takeover: true,
    }
}

/// Subscribe a client rooted at `PROP_CLIENT_STARTED_IN`, then resume A and
/// then B through the live request loop. Returns the directories recorded on
/// disk for A and B once both resumes have settled.
async fn prop_drive_two_resumes(
    session_a: &str,
    session_b: &str,
) -> Result<(Option<String>, Option<String>)> {
    prop_seed_dormant(session_a, PROP_A_ROOTED_AT)?;
    prop_seed_dormant(session_b, PROP_B_CREATED_IN)?;

    let (server_stream, client_stream) =
        crate::transport::Stream::pair().map_err(|e| anyhow!(e))?;
    let provider_template: Arc<dyn Provider> = Arc::new(MockProvider);

    let sessions: SessionAgents = Arc::new(RwLock::new(HashMap::new()));
    let (global_event_tx, _global_event_rx) = broadcast::channel(64);
    let (debug_response_tx, _debug_response_rx) = broadcast::channel(8);
    let (swarm_event_tx, _swarm_event_rx) = broadcast::channel(8);

    let server_task = tokio::spawn(handle_client(
        server_stream,
        Arc::clone(&sessions),
        global_event_tx,
        provider_template,
        Arc::new(RwLock::new(false)),
        Arc::new(RwLock::new(String::new())),
        Arc::new(RwLock::new(0usize)),
        Arc::new(RwLock::new(HashMap::new())),
        SwarmState {
            members: Arc::new(RwLock::new(HashMap::new())),
            swarms_by_id: Arc::new(RwLock::new(HashMap::new())),
            plans: Arc::new(RwLock::new(HashMap::new())),
            coordinators: Arc::new(RwLock::new(HashMap::new())),
        },
        Arc::new(RwLock::new(HashMap::new())),
        FileTouchService::new(),
        Arc::new(RwLock::new(ClientDebugState::default())),
        debug_response_tx,
        SwarmEventState {
            history: Arc::new(RwLock::new(std::collections::VecDeque::new())),
            counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            tx: swarm_event_tx,
        },
        "jcode-test".to_string(),
        "🧪".to_string(),
        Arc::new(crate::mcp::SharedMcpPool::from_default_config()),
        Arc::new(RwLock::new(HashMap::new())),
        Arc::new(RwLock::new(HashMap::new())),
        AwaitMembersRuntime::default(),
        SwarmMutationRuntime::default(),
    ));

    let (client_reader, mut client_writer) = client_stream.into_split();

    // Drain the server's event stream and report completed request ids so the
    // stored-directory assertions cannot pass merely because a resume stalled.
    let (done_tx, mut done_rx) = mpsc::unbounded_channel();
    let drain = tokio::spawn(async move {
        let mut reader = BufReader::new(client_reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if let Ok(ServerEvent::Done { id }) = serde_json::from_str(line.trim()) {
                        let _ = done_tx.send(id);
                    }
                }
            }
        }
    });

    for request in [
        prop_subscribe(PROP_CLIENT_STARTED_IN),
        prop_resume(2, session_a),
        prop_resume(3, session_b),
    ] {
        let payload = serde_json::to_string(&request).map_err(|e| anyhow!(e))? + "\n";
        client_writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| anyhow!(e))?;
        loop {
            let done_id = tokio::time::timeout(std::time::Duration::from_secs(5), done_rx.recv())
                .await
                .map_err(|_| anyhow!("timed out waiting for request completion"))?
                .ok_or_else(|| anyhow!("server event stream closed before request completion"))?;
            if done_id == request.id() {
                break;
            }
        }
    }

    let a_dir = prop_stored_dir(session_a);
    let b_dir = prop_stored_dir(session_b);

    drop(client_writer);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server_task).await;
    drain.abort();

    Ok((a_dir, b_dir))
}

/// Each dormant session keeps its recorded directory when resumed in-session.
#[tokio::test]
async fn resuming_two_sessions_preserves_each_sessions_recorded_working_dir() -> Result<()> {
    let _guard = crate::storage::lock_test_env();
    let (_runtime, prev_runtime) = setup_runtime_dir()?;

    let result =
        prop_drive_two_resumes("session_prop_first_target", "session_prop_second_target").await;

    restore_runtime_dir(prev_runtime);
    let (a_dir, b_dir) = result?;

    assert_eq!(
        a_dir.as_deref(),
        Some(PROP_A_ROOTED_AT),
        "resuming from a client started in {PROP_CLIENT_STARTED_IN} must keep \
         the session's recorded directory {PROP_A_ROOTED_AT}"
    );

    assert_eq!(
        b_dir.as_deref(),
        Some(PROP_B_CREATED_IN),
        "a second resume must keep {PROP_B_CREATED_IN} instead of inheriting \
         {PROP_CLIENT_STARTED_IN} from the client or the first resumed session"
    );

    Ok(())
}
