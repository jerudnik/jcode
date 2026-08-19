use super::*;
use std::sync::atomic::AtomicUsize;

const SUBSCRIBE_ID: u64 = 1;
const MESSAGE_ID: u64 = 2;

#[derive(Clone, Default)]
struct CountingProvider {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Provider for CountingProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::pin(stream::iter(vec![Ok(StreamEvent::MessageEnd {
            stop_reason: None,
        })])))
    }

    fn name(&self) -> &str {
        "counting-provider"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

#[derive(Debug)]
struct MessageIntakeOutcome {
    terminal_event: ServerEvent,
    provider_calls: usize,
    history_before: usize,
    history_after: usize,
}

async fn read_terminal_event(
    client_reader: &mut BufReader<crate::transport::ReadHalf>,
    request_id: u64,
) -> ServerEvent {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = client_reader
                .read_line(&mut line)
                .await
                .expect("read server event");
            assert_ne!(
                bytes, 0,
                "server closed before request {request_id} finished"
            );
            let event = decode_request_or_event(&line);
            match event {
                ServerEvent::Done { id } | ServerEvent::Error { id, .. } if id == request_id => {
                    break event;
                }
                _ => {}
            }
        }
    })
    .await
    .expect("request should reach a terminal event")
}

async fn drive_message_intake(
    content: &str,
    system_reminder: Option<&str>,
    images: Vec<(String, String)>,
) -> MessageIntakeOutcome {
    let (server_stream, client_stream) = crate::transport::Stream::pair().expect("socket pair");
    let provider = CountingProvider::default();
    let provider_calls = Arc::clone(&provider.calls);
    let provider_template: Arc<dyn Provider> = Arc::new(provider);
    let sessions: SessionAgents = Arc::new(RwLock::new(HashMap::new()));
    let (global_event_tx, _) = broadcast::channel(8);
    let (debug_response_tx, _) = broadcast::channel(8);
    let (swarm_event_tx, _) = broadcast::channel(8);

    let server_task = tokio::spawn(handle_client(
        server_stream,
        Arc::clone(&sessions),
        global_event_tx,
        provider_template,
        Arc::new(RwLock::new(false)),
        Arc::new(RwLock::new(String::new())),
        Arc::new(RwLock::new(0)),
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
    let mut client_reader = BufReader::new(client_reader);
    let working_dir = std::env::current_dir()
        .expect("current dir")
        .to_string_lossy()
        .into_owned();
    let subscribe = subscribe_request(Some(&working_dir));
    let payload = serde_json::to_string(&subscribe).expect("serialize subscribe") + "\n";
    client_writer
        .write_all(payload.as_bytes())
        .await
        .expect("write subscribe");
    assert!(matches!(
        read_terminal_event(&mut client_reader, SUBSCRIBE_ID).await,
        ServerEvent::Done { id: SUBSCRIBE_ID }
    ));

    let agent = {
        let sessions = sessions.read().await;
        assert_eq!(sessions.len(), 1, "test should create exactly one session");
        Arc::clone(sessions.values().next().expect("live session agent"))
    };
    let history_before = agent.lock().await.messages().len();

    let message = Request::Message {
        id: MESSAGE_ID,
        content: content.to_string(),
        images,
        system_reminder: system_reminder.map(str::to_string),
    };
    let payload = serde_json::to_string(&message).expect("serialize message") + "\n";
    client_writer
        .write_all(payload.as_bytes())
        .await
        .expect("write message");
    let terminal_event = read_terminal_event(&mut client_reader, MESSAGE_ID).await;
    let history_after = agent.lock().await.messages().len();
    let provider_calls = provider_calls.load(Ordering::SeqCst);

    drop(client_writer);
    server_task.abort();
    match server_task.await {
        Ok(result) => result.expect("server task result"),
        Err(error) => assert!(error.is_cancelled(), "server task join failed: {error}"),
    }

    MessageIntakeOutcome {
        terminal_event,
        provider_calls,
        history_before,
        history_after,
    }
}

fn assert_rejected_without_turn(outcome: MessageIntakeOutcome) {
    let ServerEvent::Error { id, message, .. } = &outcome.terminal_event else {
        panic!("expected structured rejection without a turn, observed {outcome:?}");
    };
    assert_eq!(*id, MESSAGE_ID);
    let message_lower = message.to_ascii_lowercase();
    assert!(
        message_lower.contains("empty") || message_lower.contains("blank"),
        "rejection should explain the invalid message: {message}"
    );
    assert_eq!(outcome.provider_calls, 0, "provider call must not start");
    assert_eq!(
        outcome.history_after, outcome.history_before,
        "history must remain unchanged"
    );
}

#[test]
fn empty_message_without_reminder_or_images_is_rejected_at_intake() {
    let _guard = crate::storage::lock_test_env_read();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let outcome = runtime.block_on(drive_message_intake("", None, vec![]));
    assert_rejected_without_turn(outcome);
}

#[test]
fn whitespace_message_without_reminder_or_images_is_rejected_at_intake() {
    let _guard = crate::storage::lock_test_env_read();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let outcome = runtime.block_on(drive_message_intake("  ", None, vec![]));
    assert_rejected_without_turn(outcome);
}

#[test]
fn empty_message_with_system_reminder_still_runs_hidden_continuation() {
    let _guard = crate::storage::lock_test_env_read();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let outcome = runtime.block_on(drive_message_intake(
        "",
        Some("continue the interrupted turn"),
        vec![],
    ));

    assert!(
        matches!(outcome.terminal_event, ServerEvent::Done { id: MESSAGE_ID }),
        "hidden continuation should complete normally: {outcome:?}"
    );
    assert_eq!(
        outcome.provider_calls, 1,
        "continuation should call provider"
    );
    assert_eq!(
        outcome.history_after, outcome.history_before,
        "continuation should reuse the trailing user turn without appending a blank one"
    );
}

#[test]
fn empty_message_with_image_still_runs() {
    let _guard = crate::storage::lock_test_env_read();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let outcome = runtime.block_on(drive_message_intake(
        "",
        None,
        vec![(
            "image/png".to_string(),
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
                .to_string(),
        )],
    ));

    assert!(
        matches!(outcome.terminal_event, ServerEvent::Done { id: MESSAGE_ID }),
        "image-only message should complete normally: {outcome:?}"
    );
    assert_eq!(outcome.provider_calls, 1, "image should call provider");
}
