use super::*;

pub(super) async fn handle_lifecycle_action(
    params: CommunicateInput,
    ctx: ToolContext,
) -> Result<ToolOutput> {
    match params.action.as_str() {
        "freeze" | "unfreeze" => {
            let action = params.action.clone();
            let request = Request::CommTaskControl {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                action: action.clone(),
                task_id: String::new(),
                target_session: None,
                message: params.message.clone(),
            };
            let response = send_request(request)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to {action} task graph: {e}"))?;
            ensure_success(&response)?;
            Ok(ToolOutput::new(format!(
                "Task graph {}. Existing assigned work may continue.",
                if action == "freeze" {
                    "frozen"
                } else {
                    "unfrozen"
                }
            )))
        }

        "start" | "start_task" | "wake" | "resume" | "retry" | "reassign" | "replace"
        | "salvage" => {
            let task_id = match params.task_id.clone() {
                Some(task_id) => task_id,
                None if params.target_session.is_some() => String::new(),
                None => {
                    return Err(anyhow::anyhow!(
                        "'task_id' is required for {} action unless 'target_session' uniquely identifies the assigned task. Use `swarm list`/`swarm plan_status` to inspect assignments, or pass task_id explicitly.",
                        params.action
                    ));
                }
            };
            if matches!(params.action.as_str(), "reassign" | "replace" | "salvage")
                && params.target_session.is_none()
            {
                return Err(anyhow::anyhow!(
                    "'target_session' is required for {} action",
                    params.action
                ));
            }

            let control_action = if params.action == "start_task" {
                "start".to_string()
            } else {
                params.action.clone()
            };

            let request = Request::CommTaskControl {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                action: control_action.clone(),
                task_id: task_id.clone(),
                target_session: params.target_session.clone(),
                message: params.message.clone(),
            };

            match send_request(request).await {
                Ok(ServerEvent::CommTaskControlResponse {
                    task_id,
                    action,
                    target_session,
                    status,
                    summary,
                    ..
                }) => {
                    let mut output = format!("Task '{}' {}", task_id, action);
                    if let Some(target_session) = target_session {
                        output.push_str(&format!(" -> {}", target_session));
                    }
                    output.push_str(&format!("\nStatus: {}", status));
                    if !summary.next_ready_ids.is_empty() {
                        output.push_str(&format!(
                            "\nNext ready: {}",
                            summary.next_ready_ids.join(", ")
                        ));
                    }
                    if !summary.newly_ready_ids.is_empty() {
                        output.push_str(&format!(
                            "\nNewly ready: {}",
                            summary.newly_ready_ids.join(", ")
                        ));
                    }
                    Ok(ToolOutput::new(output))
                }
                Ok(response) => {
                    ensure_success(&response)?;
                    let target_suffix = params
                        .target_session
                        .as_deref()
                        .map(|target| format!(" -> {}", target))
                        .unwrap_or_default();
                    Ok(ToolOutput::new(format!(
                        "Task '{}' {}{}",
                        task_id, params.action, target_suffix
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to {} task: {}", control_action, e)),
            }
        }

        "await_members" => {
            let target_status = params
                .target_status
                .unwrap_or_else(default_await_target_statuses);
            let mut session_ids = params.session_ids.unwrap_or_default();
            if let Some(target_session) = params.target_session.clone()
                && !session_ids.iter().any(|id| id == &target_session)
            {
                session_ids.push(target_session);
            }
            let timeout_minutes = params.timeout_minutes.unwrap_or(60);
            let timeout_secs = timeout_minutes * 60;
            // Public member waits are always asynchronous. The blocking
            // CommAwaitMembers protocol remains available internally for the
            // run_plan coordination loop, but agents must not park an entire
            // turn waiting on a worker or a long-lived socket.
            let blocking_was_requested = params.background == Some(false);
            let background = true;
            let notify = params.notify.unwrap_or(true);
            let wake = params.wake.unwrap_or(true);

            let request = Request::CommAwaitMembers {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_status,
                session_ids,
                mode: params.mode.clone(),
                timeout_secs: Some(timeout_secs),
                background,
                notify,
                wake,
            };

            // Background waits return promptly with a snapshot; only blocking
            // waits need the long socket timeout that covers the full wait.
            let socket_timeout = if background {
                std::time::Duration::from_secs(30)
            } else {
                std::time::Duration::from_secs(timeout_secs + 30)
            };

            match send_request_with_timeout(request, Some(socket_timeout)).await {
                Ok(ServerEvent::CommAwaitMembersResponse {
                    completed,
                    members,
                    summary,
                    background_started,
                    ..
                }) => {
                    if background_started {
                        let compatibility_note = if blocking_was_requested {
                            "\n\n(Blocking member waits are no longer supported; this wait was started asynchronously.)"
                        } else {
                            "\n\n(You can keep working; this wait runs in the background.)"
                        };
                        return Ok(ToolOutput::new(format!(
                            "{}{}",
                            summary, compatibility_note
                        )));
                    }
                    let reports = fetch_awaited_member_reports(&ctx, &members).await;
                    Ok(format_awaited_members_with_reports(
                        completed, &summary, &members, &reports,
                    ))
                }
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new("Await completed."))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to await members: {}", e)),
            }
        }

        _ => unreachable!("action routed to wrong dispatch family"),
    }
}
