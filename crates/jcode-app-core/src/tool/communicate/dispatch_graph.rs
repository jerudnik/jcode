use super::*;

pub(super) async fn handle_graph_action(
    params: CommunicateInput,
    ctx: ToolContext,
) -> Result<ToolOutput> {
    match params.action.as_str() {
        "propose_plan" => {
            let items = params.plan_items.ok_or_else(|| {
                anyhow::anyhow!("'plan_items' is required for propose_plan action")
            })?;
            if items.is_empty() {
                return Err(anyhow::anyhow!(
                    "'plan_items' must include at least one item"
                ));
            }
            let item_count = items.len() as u64;

            let request = Request::CommProposePlan {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                items,
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!(
                        "Plan proposal submitted ({} items).",
                        item_count
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to propose plan: {}", e)),
            }
        }

        "approve_plan" => {
            let proposer = params.proposer_session.ok_or_else(|| {
                anyhow::anyhow!("'proposer_session' is required for approve_plan action")
            })?;

            let request = Request::CommApprovePlan {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                proposer_session: proposer.clone(),
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!(
                        "Approved plan proposal from {}",
                        proposer
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to approve plan: {}", e)),
            }
        }

        "reject_plan" => {
            let proposer = params.proposer_session.ok_or_else(|| {
                anyhow::anyhow!("'proposer_session' is required for reject_plan action")
            })?;
            let reason = params.reason.clone();

            let request = Request::CommRejectPlan {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                proposer_session: proposer.clone(),
                reason: reason.clone(),
            };

            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    let reason_msg = reason
                        .as_ref()
                        .map(|r| format!(" (reason: {})", r))
                        .unwrap_or_default();
                    Ok(ToolOutput::new(format!(
                        "Rejected plan proposal from {}{}",
                        proposer, reason_msg
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to reject plan: {}", e)),
            }
        }

        "task_graph" | "seed_graph" => {
            let nodes = params
                .nodes
                .clone()
                .ok_or_else(|| anyhow::anyhow!("'nodes' is required for task_graph action"))?;
            if nodes.is_empty() {
                return Err(anyhow::anyhow!("'nodes' must include at least one node"));
            }
            let count = nodes.len();
            let mut seed_nodes = nodes.clone();
            let replace_existing = params.replace_existing_graph();
            let request = Request::CommSeedGraph {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                mode: params.mode.clone(),
                replace_existing,
                nodes: seed_nodes.clone(),
            };
            let mut response = send_request(request)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to seed task graph: {}", e))?;
            if replace_existing {
                ensure_success(&response)?;
                return Ok(ToolOutput::new(format!(
                    "Seeded task graph ({} nodes).",
                    count
                )));
            }
            let mut changes = Vec::new();
            let mut occupied = None;
            // At most one durable collision can be resolved per request. The
            // extra iteration consumes the final success response after all
            // colliding ids have been remapped.
            for _ in 0..=nodes.len() {
                let Some(conflicting_id) = seed_node_id_collision(&response) else {
                    ensure_success(&response)?;
                    let suffix = if changes.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " Renamed conflicting ids: {}.",
                            format_seed_remaps(&changes)
                        )
                    };
                    return Ok(ToolOutput::new(format!(
                        "Seeded task graph ({} nodes).{}",
                        count, suffix
                    )));
                };
                let occupied = match occupied.as_ref() {
                    Some(ids) => ids,
                    None => {
                        let summary = fetch_plan_status(&ctx.session_id).await?;
                        occupied.insert(plan_graph_node_ids(&summary))
                    }
                };
                let (remapped, mut remaps) = remap_conflicting_seed_nodes(
                    &seed_nodes,
                    occupied,
                    conflicting_id,
                    &seed_retry_scope(&ctx),
                );
                if remaps.is_empty() {
                    ensure_success(&response)?;
                    unreachable!("a duplicate seed error should have returned above")
                }
                seed_nodes = remapped;
                changes.append(&mut remaps);
                response = send_request(Request::CommSeedGraph {
                    id: REQUEST_ID,
                    session_id: ctx.session_id.clone(),
                    mode: params.mode.clone(),
                    replace_existing,
                    nodes: seed_nodes.clone(),
                })
                .await
                .map_err(|e| anyhow::anyhow!("Failed to retry task graph seed: {}", e))?;
            }
            ensure_success(&response)?;
            unreachable!("seed retry loop only exhausts while the server returns collisions")
        }

        "expand_node" => {
            let node_id = params
                .node_id
                .clone()
                .ok_or_else(|| anyhow::anyhow!("'node_id' is required for expand_node action"))?;
            let children = params.nodes.clone().ok_or_else(|| {
                anyhow::anyhow!("'nodes' (children) is required for expand_node action")
            })?;
            if children.is_empty() {
                return Err(anyhow::anyhow!("expand_node requires at least one child"));
            }
            let count = children.len();
            let request = Request::CommExpandNode {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                node_id: node_id.clone(),
                children,
            };
            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!(
                        "Decomposed '{}' into {} children.",
                        node_id, count
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to expand node: {}", e)),
            }
        }

        "complete_node" => {
            let node_id = params
                .node_id
                .clone()
                .ok_or_else(|| anyhow::anyhow!("'node_id' is required for complete_node action"))?;
            let artifact_json = match params.artifact.clone() {
                Some(value) => serde_json::to_string(&value)
                    .map_err(|e| anyhow::anyhow!("invalid artifact: {}", e))?,
                None => {
                    return Err(anyhow::anyhow!(
                        "'artifact' object is required for complete_node action"
                    ));
                }
            };
            let request = Request::CommCompleteNode {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                node_id: node_id.clone(),
                artifact_json,
            };
            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!("Completed node '{}'.", node_id)))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to complete node: {}", e)),
            }
        }

        "inject_gap" => {
            let gate_id = params
                .gate_id
                .clone()
                .or_else(|| params.node_id.clone())
                .ok_or_else(|| anyhow::anyhow!("'gate_id' is required for inject_gap action"))?;
            let nodes = params
                .nodes
                .clone()
                .ok_or_else(|| anyhow::anyhow!("'nodes' is required for inject_gap action"))?;
            if nodes.is_empty() {
                return Err(anyhow::anyhow!("inject_gap requires at least one node"));
            }
            let count = nodes.len();
            let request = Request::CommInjectGap {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                gate_id: gate_id.clone(),
                nodes,
            };
            match send_request(request).await {
                Ok(response) => {
                    ensure_success(&response)?;
                    Ok(ToolOutput::new(format!(
                        "Injected {} gap node(s) from gate '{}'.",
                        count, gate_id
                    )))
                }
                Err(e) => Err(anyhow::anyhow!("Failed to inject gap nodes: {}", e)),
            }
        }

        _ => unreachable!("action routed to wrong dispatch family"),
    }
}
