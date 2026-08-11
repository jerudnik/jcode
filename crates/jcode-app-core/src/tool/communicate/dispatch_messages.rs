use super::*;

pub(super) async fn handle_message_action(
    params: CommunicateInput,
    ctx: ToolContext,
) -> Result<ToolOutput> {
    match params.action.as_str() {
        "message" => {
            // `message` is the general-purpose send: it routes by the fields
            // provided. With `to_session` it acts as a DM, and with neither it
            // broadcasts to the sender's spawned subtree (whole swarm only for the coordinator).
            let message = params
                .message
                .ok_or_else(|| anyhow::anyhow!("'message' is required for message action"))?;
            let tldr = validate_swarm_tldr(params.tldr.as_deref(), &message, "this message")
                .map_err(|e| anyhow::anyhow!(e))?;
            let to_session = params.to_session.clone();

            let request = Request::CommMessage {
                id: REQUEST_ID,
                from_session: ctx.session_id.clone(),
                message: message.clone(),
                to_session: to_session.clone(),
                wake: params.wake,
                delivery: params.delivery,
                tldr,
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    let confirmation = match to_session {
                        Some(target) => {
                            format!("Direct message sent to {}: {}", target, message)
                        }
                        None => format!("Broadcast sent to your spawned subtree: {}", message),
                    };
                    Ok(ToolOutput::new(confirmation))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to send message: {}", e)),
            }
        }

        "broadcast" => {
            // `broadcast` targets the sender's spawned subtree (the swarm
            // coordinator reaches the whole swarm). Any `to_session` is intentionally
            // ignored so the action stays an unambiguous group send; use `message`/`dm`
            // to target.
            // Prefer DMs or task-graph artifacts; group sends are for rare
            // coordination moments, not routine status updates.
            let message = params
                .message
                .ok_or_else(|| anyhow::anyhow!("'message' is required for broadcast action"))?;
            let tldr = validate_swarm_tldr(params.tldr.as_deref(), &message, "this broadcast")
                .map_err(|e| anyhow::anyhow!(e))?;

            let request = Request::CommMessage {
                id: REQUEST_ID,
                from_session: ctx.session_id.clone(),
                message: message.clone(),
                to_session: None,
                wake: params.wake,
                delivery: params.delivery,
                tldr,
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!(
                        "Broadcast sent to your spawned subtree: {}",
                        message
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to broadcast message: {}", e)),
            }
        }

        "dm" => {
            let message = params
                .message
                .ok_or_else(|| anyhow::anyhow!("'message' is required for dm action"))?;
            let tldr = validate_swarm_tldr(params.tldr.as_deref(), &message, "this DM")
                .map_err(|e| anyhow::anyhow!(e))?;
            let to_session = params.to_session.ok_or_else(|| {
                anyhow::anyhow!("'to_session' (or 'target_session') is required for dm action")
            })?;

            let request = Request::CommMessage {
                id: REQUEST_ID,
                from_session: ctx.session_id.clone(),
                message: message.clone(),
                to_session: Some(to_session.clone()),
                delivery: params.delivery,
                wake: params.wake,
                tldr,
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!(
                        "Direct message sent to {}: {}",
                        to_session, message
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to send DM: {}", e)),
            }
        }

        _ => unreachable!("action routed to wrong dispatch family"),
    }
}
