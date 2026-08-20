use super::*;
use crate::message::{ContentBlock, Role};
use std::sync::atomic::AtomicUsize;

const SUBSCRIBE_ID: u64 = 1;
const FIRST_MESSAGE_ID: u64 = 2;
const RETRY_MESSAGE_ID: u64 = 3;
const LEGITIMATE_MESSAGE_ID: u64 = 4;

#[derive(Clone, Default)]
struct RateLimitThenSuccessProvider {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Provider for RateLimitThenSuccessProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(Box::pin(stream::iter(vec![Ok(StreamEvent::Error {
                message: "rate limited".to_string(),
                retry_after_secs: Some(1),
            })])));
        }

        Ok(Box::pin(stream::iter(vec![Ok(StreamEvent::MessageEnd {
            stop_reason: Some("end_turn".to_string()),
        })])))
    }

    fn name(&self) -> &str {
        "rate-limit-then-success"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
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

async fn send_message(
    client_writer: &mut crate::transport::WriteHalf,
    client_reader: &mut BufReader<crate::transport::ReadHalf>,
    id: u64,
    content: &str,
) -> ServerEvent {
    let request = Request::Message {
        id,
        content: content.to_string(),
        images: Vec::new(),
        system_reminder: None,
    };
    let payload = serde_json::to_string(&request).expect("serialize message") + "\n";
    client_writer
        .write_all(payload.as_bytes())
        .await
        .expect("write message");
    read_terminal_event(client_reader, id).await
}

fn user_message_count(agent: &Agent, text: &str) -> usize {
    agent
        .messages()
        .iter()
        .filter(|message| {
            message.role == Role::User
                && message.content.iter().any(|block| {
                    matches!(block, ContentBlock::Text { text: value, .. } if value == text)
                })
        })
        .count()
}

#[tokio::test]
async fn rate_limited_resend_reuses_user_turn_then_success_clears_coalescing_state() {
    let (server_stream, client_stream) = crate::transport::Stream::pair().expect("socket pair");
    let provider = RateLimitThenSuccessProvider::default();
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
    let prompt = "identical rate-limited resend";

    let first = send_message(
        &mut client_writer,
        &mut client_reader,
        FIRST_MESSAGE_ID,
        prompt,
    )
    .await;
    assert!(
        matches!(
            first,
            ServerEvent::Error {
                id: FIRST_MESSAGE_ID,
                retry_after_secs: Some(1),
                ..
            }
        ),
        "unexpected first terminal event: {first:?}"
    );
    {
        let agent = agent.lock().await;
        assert_eq!(user_message_count(&agent, prompt), 1);
    }

    // The terminal Error is forwarded before the processing-done branch records
    // the retry payload used by the next request.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let retry = send_message(
        &mut client_writer,
        &mut client_reader,
        RETRY_MESSAGE_ID,
        prompt,
    )
    .await;
    assert!(matches!(
        retry,
        ServerEvent::Done {
            id: RETRY_MESSAGE_ID
        }
    ));
    {
        let agent = agent.lock().await;
        assert_eq!(
            user_message_count(&agent, prompt),
            1,
            "identical retry must not persist a duplicate user turn"
        );
    }

    let legitimate = send_message(
        &mut client_writer,
        &mut client_reader,
        LEGITIMATE_MESSAGE_ID,
        prompt,
    )
    .await;
    assert!(matches!(
        legitimate,
        ServerEvent::Done {
            id: LEGITIMATE_MESSAGE_ID
        }
    ));
    {
        let agent = agent.lock().await;
        assert_eq!(
            user_message_count(&agent, prompt),
            2,
            "a successful turn must clear coalescing state for later legitimate input"
        );
    }

    drop(client_writer);
    server_task.abort();
    match server_task.await {
        Ok(result) => result.expect("server task result"),
        Err(error) => assert!(error.is_cancelled(), "server task join failed: {error}"),
    }
}
