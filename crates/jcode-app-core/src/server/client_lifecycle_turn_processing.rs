use super::{
    PendingRateLimitedMessage, ProcessingState, SwarmStatusRefs, server_reload_starting,
    truncate_detail, update_member_status,
};
use crate::agent::Agent;
use crate::protocol::ServerEvent;
use crate::server::{reload_recovery, shutdown, state};
use anyhow::Result;
use futures::FutureExt;
use jcode_agent_runtime::StreamError;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use super::message_intake::ProcessingMessage;

pub(super) async fn start_processing_message_with_rate_limit_state(
    message: ProcessingMessage,
    client_session_id: &str,
    state: &mut ProcessingState<'_>,
    agent: &Arc<Mutex<Agent>>,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    processing_done_tx: &mpsc::UnboundedSender<(u64, Result<()>, Option<String>)>,
    processing_message: &mut Option<PendingRateLimitedMessage>,
    reuse_existing_user_message: bool,
    swarm: &SwarmStatusRefs<'_>,
) {
    let ProcessingMessage {
        id,
        content,
        images,
        system_reminder,
    } = message;
    if server_reload_starting() {
        crate::logging::info(&format!(
            "Rejecting new message for session {} because server reload is starting",
            client_session_id
        ));
        if let Err(error) = client_event_tx.send(ServerEvent::Reloading { new_socket: None }) {
            crate::logging::warn(&format!(
                "Failed to send reload event for session {}: {}",
                client_session_id, error
            ));
        }
        return;
    }

    if *state.client_is_processing {
        if let Err(error) = client_event_tx.send(ServerEvent::Error {
            id,
            message: "Already processing a message".to_string(),
            retry_after_secs: None,
        }) {
            crate::logging::warn(&format!(
                "Failed to send busy error for message id={}: {}",
                id, error
            ));
        }
        return;
    }

    *state.client_is_processing = true;
    *state.message_id = Some(id);
    *state.session_id = Some(client_session_id.to_string());
    *processing_message = Some(PendingRateLimitedMessage {
        content: content.clone(),
        images: images.clone(),
        system_reminder: system_reminder.clone(),
    });

    if let Some(reminder) = system_reminder.as_deref()
        && let Err(error) = reload_recovery::mark_delivered_if_matching_continuation(
            client_session_id,
            reminder,
            "client_message_accepted",
        )
    {
        crate::logging::warn(&format!(
            "Failed to mark reload recovery intent delivered for accepted message session={} id={}: {}",
            client_session_id, id, error
        ));
    }

    update_member_status(
        client_session_id,
        "running",
        Some(truncate_detail(&content, 120)),
        swarm.members,
        swarm.swarms_by_id,
        Some(swarm.event_history),
        Some(swarm.event_counter),
        Some(swarm.event_tx),
    )
    .await;

    let start_message_index = {
        let agent_guard = agent.lock().await;
        agent_guard.message_count()
    };
    let agent = Arc::clone(agent);
    let report_agent = Arc::clone(&agent);
    let tx = state::session_event_fanout_sender_with_fallback(
        client_session_id.to_string(),
        Arc::clone(swarm.members),
        client_event_tx.clone(),
    );
    let done_tx = processing_done_tx.clone();
    crate::logging::info(&format!("Processing message id={} spawning task", id));
    *state.task = Some(tokio::spawn(async move {
        let event_tx = tx.clone();
        let result = match std::panic::AssertUnwindSafe(
            process_message_streaming_mpsc_with_existing_user_message(
                agent,
                &content,
                images,
                system_reminder,
                event_tx,
                reuse_existing_user_message,
            ),
        )
        .catch_unwind()
        .await
        {
            Ok(result) => result,
            Err(panic_payload) => {
                let msg = if let Some(text) = panic_payload.downcast_ref::<&str>() {
                    text.to_string()
                } else if let Some(text) = panic_payload.downcast_ref::<String>() {
                    text.clone()
                } else {
                    "unknown panic".to_string()
                };
                crate::logging::error(&format!(
                    "Processing task PANICKED for message id={}: {}",
                    id, msg
                ));
                Err(anyhow::anyhow!("Processing task panicked: {}", msg))
            }
        };
        match &result {
            Ok(()) => crate::logging::info(&format!(
                "Processing task completed OK for message id={}",
                id
            )),
            Err(error) => crate::logging::warn(&format!(
                "Processing task completed with error for message id={}: {}",
                id, error
            )),
        }
        let completion_report = if result.is_ok() {
            let agent = report_agent.lock().await;
            agent.latest_assistant_text_after(start_message_index)
        } else {
            None
        };
        // Keep the terminal event on the same ordered fanout channel as the
        // stream. Sending it later from the owning client's event loop could
        // race ahead of the final MessageEnd for newly attached clients.
        let terminal_event = match &result {
            Ok(()) => ServerEvent::Done { id },
            Err(error) => ServerEvent::Error {
                id,
                message: crate::util::format_error_chain(error),
                retry_after_secs: error
                    .downcast_ref::<StreamError>()
                    .and_then(|stream_error| stream_error.retry_after_secs),
            },
        };
        if let Err(error) = tx.send(terminal_event) {
            crate::logging::warn(&format!(
                "Failed to send terminal event for message id={}: {}",
                id, error
            ));
        }
        if let Err(error) = done_tx.send((id, result, completion_report)) {
            crate::logging::warn(&format!(
                "Failed to send processing completion for message id={}: {}",
                id, error
            ));
        }
    }));
}

/// Process a message and stream events (mpsc channel - per-client)
pub(crate) async fn process_message_streaming_mpsc(
    agent: Arc<Mutex<Agent>>,
    content: &str,
    images: Vec<(String, String)>,
    system_reminder: Option<String>,
    event_tx: mpsc::UnboundedSender<ServerEvent>,
) -> Result<()> {
    process_message_streaming_mpsc_with_existing_user_message(
        agent,
        content,
        images,
        system_reminder,
        event_tx,
        false,
    )
    .await
}

async fn process_message_streaming_mpsc_with_existing_user_message(
    agent: Arc<Mutex<Agent>>,
    content: &str,
    images: Vec<(String, String)>,
    system_reminder: Option<String>,
    event_tx: mpsc::UnboundedSender<ServerEvent>,
    reuse_existing_user_message: bool,
) -> Result<()> {
    // Activity lease (F01 design 3.3): this is the common provider-turn
    // boundary for every caller family (client message tasks, client
    // actions, swarm assignment, spawned/headless initial turns, Jade relay,
    // live wake turns, startup reload-recovery continuations). Acquiring at
    // the top of the future covers all of them by construction, including
    // the wait for the per-session agent mutex. A ShuttingDown refusal means
    // the daemon is draining: no new turn may start.
    let _lease = shutdown::acquire_lease(
        jcode_core::activity::ActivityClass::ProviderTurn,
        "streaming-turn",
    )
    .map_err(|refused| anyhow::anyhow!("turn refused: {refused}"))?;
    let mut agent = agent.lock().await;
    let session_id = agent.session_id().to_string();
    let result = agent
        .run_once_streaming_mpsc_with_existing_user_message(
            content,
            images,
            system_reminder,
            event_tx,
            reuse_existing_user_message,
        )
        .await;
    if result.is_ok() {
        crate::runtime_memory_log::emit_event(
            crate::runtime_memory_log::RuntimeMemoryLogEvent::new(
                "turn_completed",
                "message_turn_finished",
            )
            .with_session_id(session_id)
            .force_attribution(),
        );
        crate::process_memory::release_retained_heap_debounced(
            "server_turn_completed",
            std::time::Duration::from_secs(30),
        );
    }
    result
}
