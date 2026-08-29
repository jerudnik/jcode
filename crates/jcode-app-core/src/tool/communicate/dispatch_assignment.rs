use super::*;

pub(super) async fn handle_assignment_action(
    params: CommunicateInput,
    ctx: ToolContext,
) -> Result<ToolOutput> {
    match params.action.as_str() {
        "assign_task" => {
            let target = params
                .target_session
                .clone()
                .unwrap_or_else(|| "next available agent".to_string());
            let spawn_if_needed = params.spawn_if_needed.unwrap_or(false);
            let prefer_spawn = params.prefer_spawn.unwrap_or(false);

            if prefer_spawn && params.target_session.is_none() {
                let spawned_session = spawn_assignment_session(&ctx, &params).await?;
                return assign_task_to_session(
                    &ctx,
                    &params,
                    spawned_session,
                    " (spawned by planner preference)",
                )
                .await;
            }

            let request = Request::CommAssignTask {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_session: params.target_session.clone(),
                task_id: params.task_id.clone(),
                message: params.message.clone(),
            };

            match send_request(request).await {
                Ok(ServerEvent::CommAssignTaskResponse {
                    task_id,
                    target_session,
                    ..
                }) => {
                    let mut output = format!("Task '{}' assigned to {}", task_id, target_session);
                    if let Ok(summary) = fetch_plan_status(&ctx.session_id).await {
                        output.push_str(&format!("\n{}", format_plan_followup(&summary)));
                    }
                    Ok(ToolOutput::new(output))
                }
                Ok(response)
                    if spawn_if_needed
                        && params.target_session.is_none()
                        && auto_assignment_needs_spawn(&response) =>
                {
                    let spawned_session = spawn_assignment_session(&ctx, &params).await?;
                    assign_task_to_session(
                        &ctx,
                        &params,
                        spawned_session,
                        " (spawned automatically)",
                    )
                    .await
                }
                Ok(response) => {
                    ensure_success(&response)?;
                    let msg = params.task_id.as_deref().map_or_else(
                        || format!("Assigned next runnable task to {}", target),
                        |task_id| format!("Task '{}' assigned to {}", task_id, target),
                    );
                    Ok(ToolOutput::new(msg))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to assign task: {}", e)),
            }
        }

        "assign_next" => {
            let target = params
                .target_session
                .clone()
                .unwrap_or_else(|| "next available agent".to_string());

            let request = Request::CommAssignNext {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_session: params.target_session.clone(),
                working_dir: params.working_dir.clone(),
                prefer_spawn: params.prefer_spawn,
                spawn_if_needed: params.spawn_if_needed,
                message: params.message.clone(),
                model: params.model.clone(),
                effort: params.effort.clone(),
            };

            match send_request(request).await {
                Ok(ServerEvent::CommAssignTaskResponse {
                    task_id,
                    target_session,
                    ..
                }) => Ok(ToolOutput::new(format!(
                    "Task '{}' assigned to {}",
                    task_id, target_session
                ))),
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!(
                        "Assigned next runnable task to {}",
                        target
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to assign next task: {}", e)),
            }
        }

        "fill_slots" => {
            let concurrency_limit = params.concurrency_limit.ok_or_else(|| {
                anyhow::anyhow!("'concurrency_limit' is required for fill_slots action")
            })?;

            let summary = fetch_plan_status(&ctx.session_id).await?;
            let members = fetch_swarm_members(&ctx.session_id).await?;

            let active_count = coordination_in_flight_count(&summary, &members, &ctx.session_id);
            if dispatch_window_full(active_count, concurrency_limit) {
                return Ok(ToolOutput::new(format!(
                    "Window already full: {} active/in-flight task(s) >= limit {}",
                    active_count, concurrency_limit
                )));
            }

            let mut assignments = Vec::new();
            let available_slots = concurrency_limit.saturating_sub(active_count);
            for _ in 0..available_slots {
                let request = Request::CommAssignNext {
                    id: REQUEST_ID,
                    session_id: ctx.session_id.clone(),
                    target_session: params.target_session.clone(),
                    working_dir: params.working_dir.clone(),
                    prefer_spawn: params.prefer_spawn,
                    spawn_if_needed: params.spawn_if_needed,
                    message: params.message.clone(),
                    model: params.model.clone(),
                    effort: params.effort.clone(),
                };

                match send_request(request).await {
                    Ok(ServerEvent::CommAssignTaskResponse {
                        task_id,
                        target_session,
                        ..
                    }) => assignments.push(format!("{} -> {}", task_id, target_session)),
                    Ok(ServerEvent::Error { message, .. })
                        if message.contains("No runnable unassigned tasks")
                            || message.contains("No ready or completed swarm agents") =>
                    {
                        break;
                    }
                    Ok(response) => {
                        ensure_success(&response)?;
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!("Failed to fill slots: {}", e));
                    }
                }
            }

            if assignments.is_empty() {
                Ok(ToolOutput::new(format!(
                    "No assignments made. Active: {}, limit: {}",
                    active_count, concurrency_limit
                )))
            } else {
                let mut output = format!(
                    "Filled {} slot(s):\n{}",
                    assignments.len(),
                    assignments
                        .into_iter()
                        .map(|line| format!("- {}", line))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                if let Ok(summary) = fetch_plan_status(&ctx.session_id).await {
                    output.push_str(&format!("\n{}", format_plan_followup(&summary)));
                }
                Ok(ToolOutput::new(output))
            }
        }

        "run_plan" => {
            // Background-by-default: the plan driver runs as a managed
            // background task (progress card, bg tool, notify/wake) so the
            // coordinating agent stays responsive. Pass background=false
            // to block inline until the plan reaches a terminal state.
            if params.background.unwrap_or(true) {
                run_swarm_plan_in_background(&ctx, params.clone()).await
            } else {
                run_swarm_plan_to_terminal(&ctx, &params, &RunPlanReporter::inline()).await
            }
        }

        _ => unreachable!("action routed to wrong dispatch family"),
    }
}

/// Whether the fill_slots dispatch window has no free slot. Pure so the
/// boundary is unit-testable: a mutation probe turned the previous inline
/// `>=` into `>` (an over-dispatch off-by-one) and zero tests went red.
fn dispatch_window_full(active_count: usize, concurrency_limit: usize) -> bool {
    active_count >= concurrency_limit
}

#[cfg(test)]
mod window_tests {
    use super::dispatch_window_full;

    #[test]
    fn dispatch_window_is_full_at_exactly_the_limit() {
        assert!(!dispatch_window_full(0, 2));
        assert!(!dispatch_window_full(1, 2));
        assert!(dispatch_window_full(2, 2));
        assert!(dispatch_window_full(3, 2));
    }
}
