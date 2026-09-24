use super::*;
use tokio::sync::oneshot;

struct Request {
    connection: usize,
    body: Value,
    reply: oneshot::Sender<Vec<Value>>,
}

struct Server {
    requests: mpsc::UnboundedReceiver<Request>,
    task: tokio::task::JoinHandle<()>,
    _base: EnvVarGuard,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = EnvVarGuard::set(
            "JCODE_OPENAI_API_BASE",
            &format!("http://{}/v1", listener.local_addr().unwrap()),
        );
        let (tx, requests) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            let mut sockets = tokio::task::JoinSet::new();
            for connection in 0.. {
                let (tcp, _) = listener.accept().await.unwrap();
                let tx = tx.clone();
                sockets.spawn(async move {
                    let mut socket = tokio_tungstenite::accept_async(tcp).await.unwrap();
                    while let Some(frame) = socket.next().await {
                        match frame {
                            Ok(WsMessage::Text(text)) => {
                                let (reply, events) = oneshot::channel();
                                tx.send(Request {
                                    connection,
                                    body: serde_json::from_str(&text).unwrap(),
                                    reply,
                                })
                                .unwrap();
                                for event in events.await.unwrap() {
                                    if socket
                                        .send(WsMessage::Text(event.to_string()))
                                        .await
                                        .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            Ok(WsMessage::Ping(payload)) => {
                                let _ = socket.send(WsMessage::Pong(payload)).await;
                            }
                            Ok(WsMessage::Close(_)) | Err(_) => break,
                            _ => {}
                        }
                    }
                });
            }
        });
        Self {
            requests,
            task,
            _base: base,
        }
    }

    async fn request(&mut self) -> Request {
        tokio::time::timeout(Duration::from_secs(3), self.requests.recv())
            .await
            .expect("provider must send a request")
            .unwrap()
    }
}

async fn provider() -> OpenAIProvider {
    // Keep catalog refresh traffic out of this websocket-only loopback fixture.
    jcode_base::provider::populate_account_models(vec![DEFAULT_MODEL.into(), "gpt-5.4".into()]);
    let credentials = CodexCredentials {
        access_token: "loopback-test".into(),
        refresh_token: String::new(),
        id_token: None,
        account_id: None,
        expires_at: None,
    };
    let provider = OpenAIProvider::new(credentials.clone());
    // Isolate the fixture from machine-local credential-mode configuration.
    *provider.credentials.write().await = credentials;
    provider.set_model(DEFAULT_MODEL).unwrap();
    provider.set_transport("websocket").unwrap();
    provider
}

fn success(id: &str) -> Vec<Value> {
    vec![
        serde_json::json!({"type":"response.created","response":{"id":id}}),
        serde_json::json!({"type":"response.output_text.delta","delta":id}),
        serde_json::json!({"type":"response.completed","response":{"id":id,"status":"completed","output":[]}}),
    ]
}

async fn collect(mut stream: EventStream) -> Vec<StreamEvent> {
    tokio::time::timeout(Duration::from_secs(3), async move {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event.expect("valid stream event"));
        }
        events
    })
    .await
    .expect("terminal response must end the stream without waiting for socket closure")
}

async fn turn(
    server: &mut Server,
    provider: &dyn Provider,
    messages: &[ChatMessage],
    id: &str,
) -> (usize, Value) {
    let stream = provider
        .complete(messages, &[], "continuation fixture", None)
        .await
        .unwrap();
    let request = server.request().await;
    request.reply.send(success(id)).unwrap();
    let events = collect(stream).await;
    let text: String = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::TextDelta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, id, "response output must be delivered exactly once");
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, StreamEvent::MessageEnd { .. }))
            .count(),
        1
    );
    assert!(!events.iter().any(|e| matches!(
        e,
        StreamEvent::Error { .. } | StreamEvent::RetryRollback { .. }
    )));
    (request.connection, request.body)
}

fn full_request(body: &Value, messages: &[ChatMessage]) {
    assert!(
        body.get("previous_response_id").is_none(),
        "divergence must replay the full request"
    );
    assert_eq!(
        body["input"],
        serde_json::json!(build_responses_input(messages))
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn append_then_edited_history_replays() {
    let _env = jcode_base::storage::lock_test_env();
    let mut server = Server::new().await;
    let provider = provider().await;
    let mut messages = vec![ChatMessage::user("first")];
    let (connection, first) = turn(&mut server, &provider, &messages, "resp_first").await;
    full_request(&first, &messages);
    let prefix_len = build_responses_input(&messages).len();
    messages.push(ChatMessage::user("second"));
    let (next, append) = turn(&mut server, &provider, &messages, "resp_append").await;
    assert_eq!(next, connection);
    assert_eq!(append["previous_response_id"], "resp_first");
    assert_eq!(
        append["input"],
        serde_json::json!(&build_responses_input(&messages)[prefix_len..])
    );
    // Edit an item introduced by the continuation, not just the initial request.
    messages[1] = ChatMessage::user("edited second");
    messages.push(ChatMessage::user("third"));
    let (next, edited) = turn(&mut server, &provider, &messages, "resp_edit").await;
    full_request(&edited, &messages);
    assert_ne!(next, connection);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn shortened_history_replays() {
    let _env = jcode_base::storage::lock_test_env();
    let mut server = Server::new().await;
    let provider = provider().await;
    let mut messages = vec![ChatMessage::user("first"), ChatMessage::user("second")];
    let (connection, _) = turn(&mut server, &provider, &messages, "resp_first").await;
    messages.pop();
    let (next, body) = turn(&mut server, &provider, &messages, "resp_short").await;
    full_request(&body, &messages);
    assert_ne!(next, connection);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn late_tool_output_before_cursor_replays() {
    let _env = jcode_base::storage::lock_test_env();
    let mut server = Server::new().await;
    let provider = provider().await;
    let mut messages = vec![
        ChatMessage::user("first"),
        assistant_tool_use("call_1", "bash", serde_json::json!({"command":"ls"})),
        ChatMessage::user("tail"),
    ];
    let (connection, _) = turn(&mut server, &provider, &messages, "resp_first").await;
    messages.push(ChatMessage::tool_result("call_1", "real output", false));
    messages.push(ChatMessage::user("new tail"));
    let (next, body) = turn(&mut server, &provider, &messages, "resp_late").await;
    full_request(&body, &messages);
    assert_ne!(next, connection);
    assert_eq!(
        function_call_outputs(body["input"].as_array().unwrap(), "call_1"),
        vec!["real output"]
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn model_and_forked_session_replay() {
    let _env = jcode_base::storage::lock_test_env();
    let mut server = Server::new().await;
    let provider = provider().await;
    let mut messages = vec![ChatMessage::user("first")];
    let stream = provider
        .complete(&messages, &[], "continuation fixture", None)
        .await
        .unwrap();
    let initial = server.request().await;
    let first = initial.connection;
    // A model switch can happen before an in-flight response saves its socket.
    provider.set_model("gpt-5.4").unwrap();
    initial.reply.send(success("resp_first")).unwrap();
    collect(stream).await;
    messages.push(ChatMessage::user("second"));
    let (second, body) = turn(&mut server, &provider, &messages, "resp_model").await;
    full_request(&body, &messages);
    assert_eq!(body["model"], "gpt-5.4");
    assert_ne!(second, first);
    let fork = provider.fork();
    messages.push(ChatMessage::user("third"));
    let (third, body) = turn(&mut server, fork.as_ref(), &messages, "resp_session").await;
    full_request(&body, &messages);
    assert_ne!(third, second);
    let (parent, body) = turn(&mut server, &provider, &messages, "resp_parent").await;
    assert_eq!(parent, second);
    assert_eq!(body["previous_response_id"], "resp_model");
}

fn partial_failure(kind: &str, message: &str) -> Vec<Value> {
    vec![
        serde_json::json!({"type":"response.created","response":{"id":"resp_poisoned"}}),
        serde_json::json!({"type":"response.output_text.delta","delta":"partial"}),
        serde_json::json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"bash","arguments":"{}"}}),
        serde_json::json!({"type":kind,"response":{"id":"resp_poisoned","status":"failed","error":{"code":"invalid_request_error","message":message}}}),
    ]
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn terminal_failure_stops_once_and_next_turn_replays() {
    let _env = jcode_base::storage::lock_test_env();
    for kind in ["response.failed", "response.error", "error"] {
        let mut server = Server::new().await;
        let provider = provider().await;
        let mut messages = vec![ChatMessage::user("first")];
        let (first, _) = turn(&mut server, &provider, &messages, "resp_first").await;
        messages.push(ChatMessage::user("second"));
        let stream = provider
            .complete(&messages, &[], "continuation fixture", None)
            .await
            .unwrap();
        let request = server.request().await;
        assert_eq!(request.connection, first);
        request
            .reply
            .send(partial_failure(kind, "invalid request"))
            .unwrap();
        let events = collect(stream).await;
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, StreamEvent::Error { .. }))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, StreamEvent::TextDelta(_)))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, StreamEvent::ToolUseStart { .. }))
                .count(),
            1
        );
        assert!(!events.iter().any(|e| matches!(
            e,
            StreamEvent::MessageEnd { .. } | StreamEvent::RetryRollback { .. }
        )));
        messages.push(ChatMessage::user("recover"));
        let (next, body) = turn(&mut server, &provider, &messages, "resp_recovered").await;
        full_request(&body, &messages);
        assert_ne!(next, first);
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn retryable_failure_invalidates_before_queued_continuation() {
    let _env = jcode_base::storage::lock_test_env();
    let mut server = Server::new().await;
    let provider = provider().await;
    let messages = vec![ChatMessage::user("first")];
    turn(&mut server, &provider, &messages, "resp_first").await;
    let input = build_responses_input(&[messages[0].clone(), ChatMessage::user("next")]);
    let request = serde_json::json!({"model":DEFAULT_MODEL});
    let (tx, _rx) = mpsc::channel(100);
    let first =
        try_persistent_ws_continuation(&provider.persistent_ws, &request, &input, input.len(), &tx);
    let queued = async {
        let wire = server.request().await;
        // Poll the second continuation while the first owns the socket lock.
        let second = try_persistent_ws_continuation(
            &provider.persistent_ws,
            &request,
            &input,
            input.len(),
            &tx,
        );
        tokio::pin!(second);
        assert!(futures::poll!(&mut second).is_pending());
        wire.reply
            .send(partial_failure("response.failed", "server_error"))
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), second)
            .await
            .expect("queued continuation must not reuse failed socket")
    };
    let (first, second) = tokio::join!(first, queued);
    assert!(matches!(first, PersistentWsResult::Failed(_)));
    assert!(matches!(second, PersistentWsResult::NotAvailable));
    assert!(
        server.requests.try_recv().is_err(),
        "failed socket must receive no second request"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn fresh_terminal_failure_stops_without_caching() {
    let _env = jcode_base::storage::lock_test_env();
    let mut server = Server::new().await;
    let provider = provider().await;
    let mut messages = vec![ChatMessage::user("first")];
    let stream = provider
        .complete(&messages, &[], "continuation fixture", None)
        .await
        .unwrap();
    let request = server.request().await;
    let first = request.connection;
    request
        .reply
        .send(partial_failure("response.failed", "invalid request"))
        .unwrap();
    let events = collect(stream).await;
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, StreamEvent::Error { .. }))
            .count(),
        1
    );
    assert!(!events.iter().any(|e| matches!(
        e,
        StreamEvent::MessageEnd { .. } | StreamEvent::RetryRollback { .. }
    )));
    messages.push(ChatMessage::user("recover"));
    let (next, body) = turn(&mut server, &provider, &messages, "resp_recovered").await;
    full_request(&body, &messages);
    assert_ne!(next, first);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn partial_retry_rolls_back_text_and_tools_before_full_replay() {
    let _env = jcode_base::storage::lock_test_env();
    let mut server = Server::new().await;
    let provider = provider().await;
    let mut messages = vec![ChatMessage::user("first")];
    let (first, _) = turn(&mut server, &provider, &messages, "resp_first").await;
    messages.push(ChatMessage::user("next"));
    let stream = provider
        .complete(&messages, &[], "continuation fixture", None)
        .await
        .unwrap();
    let request = server.request().await;
    request
        .reply
        .send(partial_failure("response.failed", "server_error"))
        .unwrap();
    let replay = server.request().await;
    full_request(&replay.body, &messages);
    assert_ne!(replay.connection, first);
    let mut events = partial_failure("response.failed", "unused");
    events[0] = serde_json::json!({"type":"response.created","response":{"id":"resp_replay"}});
    *events.last_mut().unwrap() = serde_json::json!({"type":"response.completed","response":{"id":"resp_replay","status":"completed","output":[]}});
    replay.reply.send(events).unwrap();
    let events = collect(stream).await;
    let (mut text, mut tools, mut rollbacks, mut ends) = (String::new(), Vec::new(), 0, 0);
    for event in events {
        match event {
            StreamEvent::RetryRollback { .. } => {
                text.clear();
                tools.clear();
                rollbacks += 1;
            }
            StreamEvent::TextDelta(delta) => text.push_str(&delta),
            StreamEvent::ToolUseStart { id, .. } => tools.push(id),
            StreamEvent::MessageEnd { .. } => ends += 1,
            StreamEvent::Error { message, .. } => panic!("unexpected terminal error: {message}"),
            _ => {}
        }
    }
    assert_eq!(
        (text.as_str(), tools, rollbacks, ends),
        ("partial", vec!["call_1".to_string()], 1, 1)
    );
    messages.push(ChatMessage::user("after recovery"));
    let (next, body) = turn(&mut server, &provider, &messages, "resp_next").await;
    assert_eq!(next, replay.connection);
    assert_eq!(body["previous_response_id"], "resp_replay");
}
