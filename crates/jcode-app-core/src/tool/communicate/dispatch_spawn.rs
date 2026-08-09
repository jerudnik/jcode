use super::*;

pub(super) async fn handle_spawn_action(
    params: CommunicateInput,
    ctx: ToolContext,
) -> Result<ToolOutput> {
    match params.action.as_str() {
        "spawn" => {
            let label = params.required_spawn_label()?;
            let request = Request::CommSpawn {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                working_dir: params.working_dir.clone(),
                initial_message: params.spawn_initial_message(),
                request_nonce: None,
                spawn_mode: params.spawn_mode.clone(),
                model: params.model.clone(),
                effort: params.effort.clone(),
                label: Some(label),
                subagent_type: params.subagent_type.clone(),
            };

            match send_request(request).await {
                Ok(ServerEvent::CommSpawnResponse { new_session_id, .. })
                    if !new_session_id.is_empty() =>
                {
                    Ok(ToolOutput::new(format!(
                        "Spawned new agent: {}",
                        new_session_id
                    )))
                }
                Ok(response) => {
                    ensure_success(&response)?;
                    Err(anyhow::anyhow!(
                        "Spawn succeeded but new session ID was not returned."
                    ))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to spawn agent: {}", e)),
            }
        }

        "list_models" => {
            let request = Request::CommListModels {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
            };
            match send_request(request).await {
                Ok(ServerEvent::CommListModelsResponse {
                    current_model,
                    configured_swarm_model,
                    model_routes,
                    ..
                }) => Ok(ToolOutput::new(format_swarm_model_list(
                    current_model.as_deref(),
                    configured_swarm_model.as_deref(),
                    &model_routes,
                ))),
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new("No model catalog returned."))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to list models: {}", e)),
            }
        }

        "list_swarms" => {
            let request = Request::CommListSwarms {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
            };
            match send_request(request).await {
                Ok(ServerEvent::CommListSwarmsResponse { swarms, .. }) => {
                    Ok(format_swarm_fleet(&swarms))
                }
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new("No live swarms found."))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to list swarms: {}", e)),
            }
        }

        _ => unreachable!("action routed to wrong dispatch family"),
    }
}
