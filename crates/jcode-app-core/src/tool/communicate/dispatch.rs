use super::*;

pub(super) async fn execute(input: Value, ctx: ToolContext) -> Result<ToolOutput> {
    let mut params: CommunicateInput = match serde_json::from_value(input.clone()) {
        Ok(params) => params,
        // Harnesses/models frequently double-encode non-string params as
        // JSON strings ("true", "4", "[...]"). Retry once with those
        // coerced back to their JSON values before failing.
        Err(first_err) => {
            serde_json::from_value(coerce_double_encoded_fields(input)).map_err(|_| first_err)?
        }
    };

    // `to_session` and `target_session` both name a single session id. Historically
    // different actions required different field names (e.g. `dm` wanted `to_session`
    // while `assign_role`/`summary`/`status`/`start`/`resume` wanted `target_session`),
    // which models frequently confuse, producing repeated "'to_session' is required" /
    // "'target_session' is required" errors. Treat the two fields as interchangeable
    // aliases so either name works for any action that targets a session.
    match (params.to_session.is_some(), params.target_session.is_some()) {
        (true, false) => params.target_session = params.to_session.clone(),
        (false, true) => params.to_session = params.target_session.clone(),
        _ => {}
    }

    // Normalize common action synonyms that models invent (e.g. `inbox`, `send`,
    // `msg`) so a near-miss verb maps to the real action instead of erroring out.
    params.action = canonical_swarm_action(&params.action).to_string();

    match params.action.as_str() {
        "message" | "broadcast" | "dm" => {
            dispatch_messages::handle_message_action(params, ctx).await
        }
        "propose_plan" | "approve_plan" | "reject_plan" | "task_graph" | "seed_graph"
        | "expand_node" | "complete_node" | "inject_gap" => {
            dispatch_graph::handle_graph_action(params, ctx).await
        }
        "spawn" | "list_models" | "list_swarms" => {
            dispatch_spawn::handle_spawn_action(params, ctx).await
        }
        "list" | "stop" | "cleanup" | "assign_role" | "status" | "report" | "plan_status"
        | "summary" | "read_context" | "resync_plan" => {
            dispatch_manage::handle_manage_action(params, ctx).await
        }
        "assign_task" | "assign_next" | "fill_slots" | "run_plan" => {
            dispatch_assignment::handle_assignment_action(params, ctx).await
        }
        "start" | "start_task" | "wake" | "resume" | "retry" | "reassign" | "replace"
        | "salvage" | "freeze" | "unfreeze" | "await_members" => {
            dispatch_lifecycle::handle_lifecycle_action(params, ctx).await
        }
        _ => Err(anyhow::anyhow!(
            "Unknown action '{}'. Valid actions: message, broadcast, dm, list, \
             propose_plan, approve_plan, reject_plan, spawn, stop, assign_role, status, report, plan_status, summary, read_context, \
             resync_plan, assign_task, assign_next, fill_slots, run_plan, cleanup, start, start_task, wake, resume, retry, reassign, replace, salvage, freeze, unfreeze, await_members, \
             task_graph (seed the task DAG), expand_node, complete_node, inject_gap, list_models, list_swarms.",
            params.action
        )),
    }
}
