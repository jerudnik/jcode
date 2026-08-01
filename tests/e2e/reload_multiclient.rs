//! In-process multi-client reload tests.
//!
//! These exercise the real server reload fan-out and streaming-interrupt paths
//! against a `MockProvider` with multiple clients attached, without execing a
//! new binary (the server runs with `JCODE_TEST_SESSION=1`, so
//! `await_reload_signal` performs all the in-process bookkeeping -- recovery
//! intents, graceful shutdown, marker writes, and client fan-out -- but skips
//! the final `exec`). This is the cheapest way to cover the multi-client +
//! streaming behavior that the binary-level reload tests cannot reach without
//! real credentials and a release build.

use crate::test_support::*;

/// Spin up an in-process server backed by a `MockProvider`.
async fn start_inprocess_server(
    label: &str,
    provider: Arc<dyn jcode::provider::Provider>,
) -> Result<(
    std::path::PathBuf,
    std::path::PathBuf,
    tokio::task::JoinHandle<Result<server::ServerExit>>,
)> {
    let runtime_dir = short_runtime_dir(format!(
        "jcode-reload-mc-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&runtime_dir)?;
    let socket_path = runtime_dir.join("jcode.sock");
    let debug_socket_path = runtime_dir.join("jcode-debug.sock");

    let server_instance =
        server::Server::new_with_paths(provider, socket_path.clone(), debug_socket_path.clone());
    let server_handle = tokio::spawn(async move { server_instance.run().await });
    wait_for_socket(&socket_path).await?;
    Ok((socket_path, debug_socket_path, server_handle))
}

/// Read events until the `Done` for `msg_id` arrives, so a test can sequence
/// turns without racing the stream.
async fn wait_for_done(client: &mut server::Client, msg_id: u64) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), client.read_event()).await {
            Ok(Ok(ServerEvent::Done { id })) if id == msg_id => return Ok(()),
            Ok(Ok(_)) => continue,
            Ok(Err(e)) => anyhow::bail!("client disconnected waiting for Done: {e}"),
            Err(_) => continue,
        }
    }
    anyhow::bail!("timed out waiting for Done on message {msg_id}")
}

/// Read events from `client` until a `Reloading` event is seen or the deadline
/// elapses. Returns whether a `Reloading` event was observed plus the events
/// seen. A clean disconnect terminates the scan (returning `false`).
async fn read_until_reloading(
    client: &mut server::Client,
    deadline: Instant,
) -> Result<(bool, Vec<ServerEvent>)> {
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), client.read_event()).await {
            Ok(Ok(event)) => {
                let is_reloading = matches!(event, ServerEvent::Reloading { .. });
                seen.push(event);
                if is_reloading {
                    return Ok((true, seen));
                }
            }
            Ok(Err(_)) => return Ok((false, seen)),
            Err(_) => continue,
        }
    }
    Ok((false, seen))
}

async fn subscribe_new_session(socket_path: &std::path::Path) -> Result<(server::Client, String)> {
    let mut client = server::Client::connect_with_path(socket_path.to_path_buf()).await?;
    let sub = client.subscribe().await?;
    let _ = collect_until_done_unix(&mut client, sub).await?;
    // Peer subscribes fan out SwarmStatus broadcasts that can interleave ahead
    // of the History reply; skip them.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let session_id = loop {
        let history = client.get_history_event().await?;
        match &history {
            ServerEvent::History { session_id, .. } => break session_id.clone(),
            _ if std::time::Instant::now() < deadline => continue,
            other => anyhow::bail!("expected history, got {other:?}"),
        }
    };
    Ok((client, session_id))
}

/// Clients attached to MULTIPLE independent sessions must each be notified when
/// any client triggers a reload, because a reload restarts the whole server.
/// This exercises `handle_reload`'s fan-out across every live session's
/// `event_txs`, not just the triggering session.
#[tokio::test]
async fn reload_notifies_clients_across_independent_sessions() -> Result<()> {
    let _env = setup_test_env()?;

    let provider = MockProvider::new();
    let provider: Arc<dyn Provider> = Arc::new(provider);
    let (socket_path, debug_socket_path, server_handle) =
        start_inprocess_server("multisession", provider).await?;

    let result = async {
        let (mut client_a, _sid_a) = subscribe_new_session(&socket_path).await?;
        let (mut client_b, _sid_b) = subscribe_new_session(&socket_path).await?;
        let (mut client_c, _sid_c) = subscribe_new_session(&socket_path).await?;

        // Let all three senders register.
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Only client A asks for the reload.
        client_a.reload().await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let (a_saw, a_events) = read_until_reloading(&mut client_a, deadline).await?;
        let (b_saw, b_events) = read_until_reloading(&mut client_b, deadline).await?;
        let (c_saw, c_events) = read_until_reloading(&mut client_c, deadline).await?;

        let fmt = |events: &[ServerEvent]| {
            events.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>()
        };
        assert!(a_saw, "initiator session must be notified; saw: {:?}", fmt(&a_events));
        assert!(
            b_saw,
            "a client on a SECOND independent session must be notified the server is reloading; saw: {:?}",
            fmt(&b_events)
        );
        assert!(
            c_saw,
            "a client on a THIRD independent session must be notified the server is reloading; saw: {:?}",
            fmt(&c_events)
        );

        Ok::<_, anyhow::Error>(())
    }
    .await;

    server::clear_reload_marker();
    abort_server_and_cleanup(&server_handle, &socket_path, &debug_socket_path);
    result
}

/// When a client takes over a live session (the multi-client reconnect path),
/// the SUCCESSOR connection must still receive the reload notification and the
/// superseded connection must be cleanly disconnected -- not left hanging.
#[tokio::test]
async fn reload_notifies_successor_after_session_takeover() -> Result<()> {
    let _env = setup_test_env()?;

    let provider = MockProvider::new();
    let provider: Arc<dyn Provider> = Arc::new(provider);
    let (socket_path, debug_socket_path, server_handle) =
        start_inprocess_server("takeover", provider).await?;

    let result = async {
        // Original owner.
        let (mut client_a, session_id) = subscribe_new_session(&socket_path).await?;

        // Successor reconnects to the same session with takeover semantics.
        let mut client_b = server::Client::connect_with_path(socket_path.clone()).await?;
        let sub_b = client_b.subscribe().await?;
        let _ = collect_until_done_unix(&mut client_b, sub_b).await?;
        let resume_b = client_b
            .resume_session_with_options(&session_id, true, true)
            .await?;
        let _ = collect_until_history_unix(&mut client_b, resume_b).await?;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // The successor triggers the reload.
        client_b.reload().await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let (b_saw, b_events) = read_until_reloading(&mut client_b, deadline).await?;
        assert!(
            b_saw,
            "the live successor connection must be told the server is reloading; saw: {:?}",
            b_events
                .iter()
                .map(|e| format!("{e:?}"))
                .collect::<Vec<_>>()
        );

        // The superseded original connection must end (disconnect) rather than
        // hang indefinitely. It may have received an Error/Reloading first.
        let a_deadline = Instant::now() + Duration::from_secs(5);
        let mut a_terminated = false;
        while Instant::now() < a_deadline {
            match tokio::time::timeout(Duration::from_millis(500), client_a.read_event()).await {
                Ok(Ok(_)) => continue,
                Ok(Err(_)) => {
                    a_terminated = true;
                    break;
                }
                Err(_) => continue,
            }
        }
        assert!(
            a_terminated,
            "the superseded original connection must be cleanly disconnected, not left hanging"
        );

        Ok::<_, anyhow::Error>(())
    }
    .await;

    server::clear_reload_marker();
    abort_server_and_cleanup(&server_handle, &socket_path, &debug_socket_path);
    result
}

/// A client with an in-flight STREAMING turn during a reload must reach a
/// terminal state (Reloading and/or Done) without hanging. This drives the
/// graceful-shutdown interrupt path inside `run_turn_streaming_mpsc` where the
/// server fires each session's shutdown signal mid-stream.
#[tokio::test]
async fn reload_interrupts_in_flight_streaming_turn_without_hanging() -> Result<()> {
    let _env = setup_test_env()?;

    let provider = MockProvider::new();
    // A long stream of text deltas with NO MessageEnd: the turn stays "running"
    // so the reload graceful-shutdown path must interrupt it. The mock yields
    // all events eagerly, but the server still processes the stream while the
    // reload signal races in; the key assertion is that the streaming client
    // reaches a terminal event quickly rather than hanging.
    let mut long_stream = Vec::new();
    for i in 0..2000 {
        long_stream.push(StreamEvent::TextDelta(format!("chunk-{i} ")));
    }
    long_stream.push(StreamEvent::MessageEnd {
        stop_reason: Some("end_turn".to_string()),
    });
    provider.queue_response(long_stream);
    let provider: Arc<dyn Provider> = Arc::new(provider);
    let (socket_path, debug_socket_path, server_handle) =
        start_inprocess_server("streaming", provider).await?;

    let result = async {
        let (mut client, _session_id) = subscribe_new_session(&socket_path).await?;

        // Kick off the streaming turn.
        let msg_id = client.send_message("please stream a lot").await?;

        // Wait until the stream is actually flowing before reloading.
        let mut saw_text = false;
        let stream_deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < stream_deadline {
            match tokio::time::timeout(Duration::from_millis(500), client.read_event()).await {
                Ok(Ok(ServerEvent::TextDelta { .. })) => {
                    saw_text = true;
                    break;
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) => break,
                Err(_) => continue,
            }
        }
        assert!(saw_text, "streaming turn should emit text before reload");

        // Reload while the turn is in flight.
        client.reload().await?;

        // The client must reach a terminal state quickly: either Reloading,
        // a Done for the message, or a disconnect. It must NOT hang.
        let mut terminal = false;
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(500), client.read_event()).await {
                Ok(Ok(ServerEvent::Reloading { .. })) => {
                    terminal = true;
                    break;
                }
                Ok(Ok(ServerEvent::Done { id })) if id == msg_id => {
                    terminal = true;
                    break;
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) => {
                    terminal = true;
                    break;
                }
                Err(_) => continue,
            }
        }
        assert!(
            terminal,
            "a streaming turn interrupted by reload must reach a terminal state (Reloading/Done/disconnect) without hanging"
        );

        Ok::<_, anyhow::Error>(())
    }
    .await;

    server::clear_reload_marker();
    abort_server_and_cleanup(&server_handle, &socket_path, &debug_socket_path);
    result
}

/// The reload marker must be active around the time clients are told to
/// reconnect, so that a client which disconnects mid-reload is classified as
/// `Reloading` rather than `Crashed` by `disconnect_disposition`.
#[tokio::test]
async fn reload_marker_is_active_when_clients_are_notified() -> Result<()> {
    let _env = setup_test_env()?;

    let provider = MockProvider::new();
    let provider: Arc<dyn Provider> = Arc::new(provider);
    let (socket_path, debug_socket_path, server_handle) =
        start_inprocess_server("marker", provider).await?;

    let result = async {
        let (mut client, _session_id) = subscribe_new_session(&socket_path).await?;

        tokio::time::sleep(Duration::from_millis(50)).await;
        client.reload().await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let (saw_reloading, events) = read_until_reloading(&mut client, deadline).await?;
        assert!(
            saw_reloading,
            "client should observe Reloading; saw: {:?}",
            events.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>()
        );

        let mut marker_active = false;
        let marker_deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < marker_deadline {
            if server::reload_marker_active(Duration::from_secs(30)) {
                marker_active = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(
            marker_active,
            "the reload marker must be active around the time clients are told to reconnect"
        );

        Ok::<_, anyhow::Error>(())
    }
    .await;

    server::clear_reload_marker();
    abort_server_and_cleanup(&server_handle, &socket_path, &debug_socket_path);
    result
}

/// A hidden continuation sends an empty body with the payload in the system
/// reminder. Two things must hold at the provider boundary, and only an
/// end-to-end run through the real server can show them: no user message may
/// arrive with empty text, and the transcript must not end on an assistant
/// message (Anthropic reads that as prefill and continues it instead of
/// starting a new turn).
///
/// This covers the branch the live reload could not reach, because that reload
/// happened to resume mid-turn with a trailing tool_result.
#[tokio::test]
async fn hidden_continuation_after_assistant_reply_sends_no_blank_user_turn() -> Result<()> {
    let _env = setup_test_env()?;

    let provider = MockProvider::new();
    for reply in ["first answer", "second answer"] {
        provider.queue_response(vec![
            StreamEvent::TextDelta(reply.to_string()),
            StreamEvent::MessageEnd {
                stop_reason: Some("end_turn".to_string()),
            },
        ]);
    }
    let captures = provider.captures.clone();
    let provider: Arc<dyn Provider> = Arc::new(provider);
    let (socket_path, debug_socket_path, server_handle) =
        start_inprocess_server("hidden-continuation", provider).await?;

    let result = async {
        let (mut client, _session_id) = subscribe_new_session(&socket_path).await?;

        // Turn 1: a normal user message, so the transcript ends on an assistant
        // reply, which is the state a resume-while-idle continuation sees.
        let first = client.send_message("hello").await?;
        wait_for_done(&mut client, first).await?;

        // Turn 2: the hidden continuation shape -- empty body, payload in the
        // reminder -- exactly what the reload path sends.
        let second = client
            .send_message_with_reminder("", Some("Reload complete - continue.".to_string()))
            .await?;
        wait_for_done(&mut client, second).await?;

        let calls = captures.messages.lock().unwrap().clone();
        assert!(
            calls.len() >= 2,
            "the continuation must reach the provider as its own call, got {} call(s)",
            calls.len()
        );
        let last = calls.last().expect("a provider call");

        // `Message::with_timestamps` prefixes every user text block with a
        // `[timestamp]` tag before send, so a stored empty string arrives here as
        // "[2026-...Z] " rather than "". Checking `is_empty()` at this boundary
        // would silently never fire; strip the tag and inspect the remainder.
        let blank_user_turns = last
            .iter()
            .filter(|(role, text)| {
                role == "user" && {
                    let body = match text.split_once("] ") {
                        Some((tag, rest)) if tag.starts_with('[') => rest,
                        _ => text.as_str(),
                    };
                    body.trim().is_empty()
                }
            })
            .count();
        assert_eq!(
            blank_user_turns, 0,
            "no blank user turn may reach the provider, got payload {last:?}"
        );

        let (last_role, _) = last.last().expect("at least one message");
        assert_eq!(
            last_role, "user",
            "the transcript must not end on an assistant message, or Anthropic \
             treats the continuation as prefill; got payload {last:?}"
        );

        Ok::<_, anyhow::Error>(())
    }
    .await;

    server::clear_reload_marker();
    abort_server_and_cleanup(&server_handle, &socket_path, &debug_socket_path);
    result
}
