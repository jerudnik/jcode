use super::*;

pub(super) async fn handle_manage_action(
    params: CommunicateInput,
    ctx: ToolContext,
) -> Result<ToolOutput> {
    match params.action.as_str() {
        "stop" => {
            let target = params
                .target_session
                .ok_or_else(|| anyhow::anyhow!("'target_session' is required for stop action"))?;

            let request = Request::CommStop {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_session: target.clone(),
                force: params.force,
                cross_swarm: false,
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!("Stopped agent: {}", target)))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to stop agent: {}", e)),
            }
        }

        "cleanup" => cleanup_swarm_workers(&ctx, &params)
            .await
            .map(ToolOutput::new),

        "assign_role" => {
            let target_raw = params.target_session.ok_or_else(|| {
                anyhow::anyhow!("'target_session' is required for assign_role action")
            })?;
            let role = params
                .role
                .ok_or_else(|| anyhow::anyhow!("'role' is required for assign_role action"))?;

            // Resolve "current" to the caller's own session ID
            let target = if target_raw == "current" {
                ctx.session_id.clone()
            } else {
                target_raw
            };

            let request = Request::CommAssignRole {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_session: target.clone(),
                role: role.clone(),
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!(
                        "Assigned role '{}' to {}",
                        role, target
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to assign role: {}", e)),
            }
        }

        "status" => {
            let target = resolve_optional_target_session(params.target_session, &ctx.session_id);

            let request = Request::CommStatus {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_session: target.clone(),
            };

            match send_request(request).await {
                Ok(ServerEvent::CommStatusResponse { snapshot, .. }) => {
                    Ok(format_status_snapshot(&snapshot))
                }
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new("No status snapshot returned."))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to get status snapshot: {}", e)),
            }
        }

        "report" => {
            let message = params
                .message
                .ok_or_else(|| anyhow::anyhow!("'message' is required for report action"))?;
            let tldr = validate_swarm_tldr(params.tldr.as_deref(), &message, "this report")
                .map_err(|e| anyhow::anyhow!(e))?;
            let request = Request::CommReport {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                status: params.status,
                message,
                validation: params.validation,
                follow_up: params.follow_up,
                tldr,
            };
            match send_request(request).await {
                Ok(ServerEvent::CommReportResponse {
                    status, message, ..
                }) => Ok(ToolOutput::new(format!(
                    "Report recorded with status `{status}`. {message}"
                ))),
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new("Report recorded."))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to record report: {}", e)),
            }
        }

        "plan_status" => {
            let summary = fetch_plan_status(&ctx.session_id).await?;
            Ok(format_plan_status(&summary))
        }

        "summary" => {
            let target = params.target_session.ok_or_else(|| {
                anyhow::anyhow!("'target_session' is required for summary action")
            })?;

            let request = Request::CommSummary {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_session: target.clone(),
                limit: params.limit,
            };

            match send_request(request).await {
                Ok(ServerEvent::CommSummaryResponse { tool_calls, .. }) => {
                    Ok(format_tool_summary(&target, &tool_calls))
                }
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new("No tool call data returned."))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to get summary: {}", e)),
            }
        }

        "read_context" => {
            let target = params.target_session.ok_or_else(|| {
                anyhow::anyhow!("'target_session' is required for read_context action")
            })?;

            let request = Request::CommReadContext {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_session: target.clone(),
            };

            match send_request(request).await {
                Ok(ServerEvent::CommContextHistory { messages, .. }) => {
                    Ok(format_context_history(&target, &messages))
                }
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new("No context data returned."))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to read context: {}", e)),
            }
        }

        "resync_plan" => {
            let request = Request::CommResyncPlan {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new("Swarm plan re-synced to your session."))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to resync plan: {}", e)),
            }
        }

        _ => unreachable!("action routed to wrong dispatch family"),
    }
}
