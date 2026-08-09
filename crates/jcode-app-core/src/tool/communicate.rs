#![cfg_attr(test, allow(clippy::await_holding_lock))]

use super::{Tool, ToolContext, ToolOutput};
use crate::background::TaskResult;
use crate::plan::PlanItem;
use crate::protocol::{
    AgentInfo, AgentStatusSnapshot, AwaitedMemberStatus, CommDeliveryMode, HistoryMessage,
    PlanGraphStatus, Request, ServerEvent, SwarmFleetEntry, ToolCallSummary,
    comm_cleanup_candidate_session_ids, default_comm_await_target_statuses,
    default_comm_cleanup_target_statuses, default_comm_run_await_statuses,
    format_comm_awaited_members_with_reports, format_comm_context_history, format_comm_members,
    format_comm_plan_followup, format_comm_plan_status, format_comm_status_snapshot,
    format_comm_tool_summary, latest_assistant_comm_report, resolve_optional_comm_target_session,
};
use anyhow::Result;
use async_trait::async_trait;
use jcode_swarm_core::validate_swarm_tldr;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

const REQUEST_ID: u64 = 1;

/// Default number of workers `run_plan` keeps active at once for a **light**-mode
/// plan. Light mode is the cheap fan-out preset, so this stays small. Deep mode
/// instead uses `agents.swarm_max_concurrent_agents` (high, configurable).
const LIGHT_MODE_DEFAULT_CONCURRENCY: usize = 4;

mod dispatch;
mod dispatch_assignment;
mod dispatch_graph;
mod dispatch_lifecycle;
mod dispatch_manage;
mod dispatch_messages;
mod dispatch_spawn;
mod input;
mod run_plan;
mod run_plan_errors;
mod schema;
mod seed_graph;
mod transport;
mod workers;

use seed_graph::{
    format_seed_remaps, plan_graph_node_ids, remap_conflicting_seed_nodes, seed_node_id_collision,
    seed_retry_scope,
};
use transport::{send_request, send_request_with_timeout};

use input::{CommunicateInput, canonical_swarm_action, coerce_double_encoded_fields};
use run_plan::*;
use workers::*;

fn fresh_spawn_request_nonce(ctx: &ToolContext) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{}-{}-{}", ctx.session_id, ctx.message_id, now_ms)
}

fn check_error(response: &ServerEvent) -> Option<&str> {
    if let ServerEvent::Error { message, .. } = response {
        Some(message)
    } else {
        None
    }
}

fn ensure_success(response: &ServerEvent) -> Result<()> {
    if let Some(message) = check_error(response) {
        Err(anyhow::anyhow!(message.to_string()))
    } else {
        Ok(())
    }
}

async fn fetch_plan_status(session_id: &str) -> Result<PlanGraphStatus> {
    let request = Request::CommPlanStatus {
        id: REQUEST_ID,
        session_id: session_id.to_string(),
    };
    match send_request(request).await {
        Ok(ServerEvent::CommPlanStatusResponse { summary, .. }) => Ok(summary),
        Ok(response) => {
            ensure_success(&response)?;
            Err(anyhow::anyhow!("No plan status returned."))
        }
        Err(e) => Err(anyhow::anyhow!("Failed to get plan status: {}", e)),
    }
}

fn format_plan_followup(summary: &PlanGraphStatus) -> String {
    format_comm_plan_followup(summary)
}

fn default_cleanup_target_statuses() -> Vec<String> {
    default_comm_cleanup_target_statuses()
}

fn default_run_await_statuses() -> Vec<String> {
    default_comm_run_await_statuses()
}

fn cleanup_candidate_session_ids(
    owner_session_id: &str,
    members: &[AgentInfo],
    target_status: &[String],
    requested_session_ids: &[String],
    force: bool,
) -> Vec<String> {
    comm_cleanup_candidate_session_ids(
        owner_session_id,
        members,
        target_status,
        requested_session_ids,
        force,
    )
}

fn auto_assignment_needs_spawn(response: &ServerEvent) -> bool {
    check_error(response).is_some_and(|message| {
        message.contains(
            "No ready or completed swarm agents are available for automatic task assignment",
        )
    })
}

async fn fetch_swarm_members(session_id: &str) -> Result<Vec<AgentInfo>> {
    let request = Request::CommList {
        id: REQUEST_ID,
        session_id: session_id.to_string(),
    };
    match send_request(request).await {
        Ok(ServerEvent::CommMembers { members, .. }) => Ok(members),
        Ok(response) => {
            ensure_success(&response)?;
            Ok(Vec::new())
        }
        Err(e) => Err(anyhow::anyhow!("Failed to list swarm members: {}", e)),
    }
}

fn swarm_member_is_in_flight(member: &AgentInfo) -> bool {
    matches!(
        member.status.as_deref(),
        Some("queued" | "running" | "running_stale")
    )
}

fn coordination_in_flight_count(
    summary: &PlanGraphStatus,
    members: &[AgentInfo],
    current_session_id: &str,
) -> usize {
    summary.active_ids.len().max(
        members
            .iter()
            .filter(|member| member.session_id != current_session_id)
            .filter(|member| swarm_member_is_in_flight(member))
            .filter(|member| swarm_member_is_drivable_worker(member, current_session_id))
            .count(),
    )
}

/// Sessions `run_plan` should await as genuinely in-flight on *this* plan.
///
/// A member counts only when it is both in-flight (`queued`/`running`) **and** a
/// drivable worker for this run: headless, or owned by the coordinator
/// (`report_back_to_session_id == coordinator`). This deliberately excludes
/// independent, client-attached human sessions that merely share the swarm and
/// happen to sit in a `queued` status. Awaiting those would hang `run_plan`
/// forever even though every plan task is already terminal (they are never auto
/// driven), which is exactly the stall this scoping prevents.
///
/// Pure over an already-fetched member list so the coordination loop can reuse
/// one `CommList` snapshot for both in-flight scoping and failure-wave
/// classification.
fn in_flight_swarm_session_ids(members: &[AgentInfo], coordinator_session_id: &str) -> Vec<String> {
    members
        .iter()
        .filter(|member| member.session_id != coordinator_session_id)
        .filter(|member| swarm_member_is_in_flight(member))
        .filter(|member| swarm_member_is_drivable_worker(member, coordinator_session_id))
        .map(|member| member.session_id.clone())
        .collect()
}

/// Fetch-and-filter convenience over [`in_flight_swarm_session_ids`] for call
/// sites that do not otherwise need the member snapshot.
async fn fetch_in_flight_swarm_sessions(session_id: &str) -> Result<Vec<String>> {
    let members = fetch_swarm_members(session_id).await?;
    Ok(in_flight_swarm_session_ids(&members, session_id))
}

/// Whether `member` is a worker `run_plan` can rely on to autonomously execute an
/// assignment (and therefore one it is safe to await): a spawned headless worker,
/// or one owned by the coordinator that issued the run. Foreign client-attached
/// sessions are not drivable and must not gate `run_plan` completion.
fn swarm_member_is_drivable_worker(member: &AgentInfo, coordinator_session_id: &str) -> bool {
    member.is_headless.unwrap_or(false)
        || member.report_back_to_session_id.as_deref() == Some(coordinator_session_id)
}

fn format_members(ctx: &ToolContext, members: &[AgentInfo]) -> ToolOutput {
    ToolOutput::new(format_comm_members(&ctx.session_id, members))
}

fn format_tool_summary(target: &str, calls: &[ToolCallSummary]) -> ToolOutput {
    ToolOutput::new(format_comm_tool_summary(target, calls))
}

fn format_status_snapshot(snapshot: &AgentStatusSnapshot) -> ToolOutput {
    ToolOutput::new(format_comm_status_snapshot(snapshot))
}

fn format_plan_status(summary: &PlanGraphStatus) -> ToolOutput {
    let mut output = format_comm_plan_status(summary);
    if let Some(budget_line) = plan_status_budget_line(
        summary,
        crate::config::config().agents.swarm_max_concurrent_agents,
    ) {
        output.push_str(&budget_line);
    }
    ToolOutput::new(output)
}

/// Deep-mode budget line for `plan_status`: how wide the ready frontier is
/// versus the concurrency budget, with a widen-the-graph nudge when the ready
/// set cannot fill the slots. This makes under-utilization visible at plan
/// time, before `run_plan` even starts, so the coordinator can restructure the
/// graph instead of discovering the waste after the run. Pure over its inputs
/// for unit testing; returns `None` for light plans.
fn plan_status_budget_line(summary: &PlanGraphStatus, deep_cap: usize) -> Option<String> {
    if !summary.mode.eq_ignore_ascii_case("deep") {
        return None;
    }
    let budget = resolve_run_plan_concurrency(None, true, deep_cap);
    let budget_label = if budget == usize::MAX {
        format!("{} (member cap)", jcode_swarm_core::MAX_SWARM_MEMBERS)
    } else {
        budget.to_string()
    };
    let ready_width = summary.ready_ids.len();
    let active_width = summary.active_ids.len();
    let mut line = format!(
        "  Parallel budget: {} concurrent worker slot(s); ready set is {} wide ({} active).\n",
        budget_label, ready_width, active_width
    );
    let effective_budget = if budget == usize::MAX {
        jcode_swarm_core::MAX_SWARM_MEMBERS
    } else {
        budget
    };
    // Nudge only when narrowness is structural: the frontier cannot fill the
    // budget while other non-terminal work exists but is serialized behind
    // depends_on edges. A small plan that is simply almost done gets no nudge.
    let frontier = ready_width + active_width;
    let terminal = summary.completed_ids.len() + summary.cycle_ids.len();
    let serialized_remaining = summary.item_count > terminal + frontier;
    if frontier < effective_budget && serialized_remaining {
        line.push_str(
            "  The ready frontier is narrower than the budget while more work waits behind \
             depends_on edges: prefer expand_node with MANY independent siblings (depends_on \
             only for real data dependencies) to widen it.\n",
        );
    }
    Some(line)
}

fn format_context_history(target: &str, messages: &[HistoryMessage]) -> ToolOutput {
    ToolOutput::new(format_comm_context_history(target, messages))
}

#[cfg(test)]
fn format_awaited_members(
    completed: bool,
    summary: &str,
    members: &[AwaitedMemberStatus],
) -> ToolOutput {
    format_awaited_members_with_reports(completed, summary, members, &HashMap::new())
}

fn latest_assistant_report(messages: &[HistoryMessage]) -> Option<String> {
    latest_assistant_comm_report(messages)
}

fn resolve_optional_target_session(target: Option<String>, current_session: &str) -> String {
    resolve_optional_comm_target_session(target, current_session)
}

fn format_awaited_members_with_reports(
    completed: bool,
    summary: &str,
    members: &[AwaitedMemberStatus],
    reports: &HashMap<String, String>,
) -> ToolOutput {
    ToolOutput::new(format_comm_awaited_members_with_reports(
        completed, summary, members, reports,
    ))
}

async fn fetch_awaited_member_reports(
    ctx: &ToolContext,
    members: &[AwaitedMemberStatus],
) -> HashMap<String, String> {
    let mut reports = HashMap::new();
    for member in members.iter().filter(|member| member.done) {
        let request = Request::CommReadContext {
            id: REQUEST_ID,
            session_id: ctx.session_id.clone(),
            target_session: member.session_id.clone(),
        };
        match send_request(request).await {
            Ok(ServerEvent::CommContextHistory { messages, .. }) => {
                if let Some(report) = latest_assistant_report(&messages) {
                    reports.insert(member.session_id.clone(), report);
                }
            }
            Ok(response) => {
                if check_error(&response).is_some() {
                    continue;
                }
            }
            Err(_) => continue,
        }
    }
    reports
}

fn default_await_target_statuses() -> Vec<String> {
    default_comm_await_target_statuses()
}

/// Render the swarm model catalog for the `list_models` action: the current
/// (spawn-default) model, any config pin, and one line per route with
/// availability, auth method, and a relative cost estimate.
fn format_swarm_model_list(
    current_model: Option<&str>,
    configured_swarm_model: Option<&str>,
    model_routes: &[jcode_provider_core::ModelRoute],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Current model (spawn default when no override): {}\n",
        current_model.unwrap_or("unknown")
    ));
    match configured_swarm_model {
        Some(pin) if !pin.trim().is_empty() => {
            out.push_str(&format!("Configured agents.swarm_model pin: {pin}\n"));
        }
        _ => out.push_str("No agents.swarm_model pin configured (workers inherit the coordinator's model unless a per-spawn model is passed).\n"),
    }
    if model_routes.is_empty() {
        out.push_str(
            "\nNo model routes reported. Spawn with a bare model name or omit model to inherit.",
        );
        return out;
    }
    out.push_str("\nAvailable model routes (pass as spawn model, e.g. 'gpt-5.5' or route-pinned 'openai-api:gpt-5.5'):\n");
    for route in model_routes {
        let availability = if route.available {
            ""
        } else {
            " [unavailable]"
        };
        let cost = route
            .estimated_reference_cost_micros()
            .map(|micros| format!(" ~${:.2}/ref-task", micros as f64 / 1_000_000.0))
            .unwrap_or_default();
        let detail = if route.detail.is_empty() {
            String::new()
        } else {
            format!(" ({})", route.detail)
        };
        out.push_str(&format!(
            "- {} via {} [{}]{}{}{}\n",
            route.model, route.provider, route.api_method, availability, cost, detail
        ));
    }
    out.push_str("\nAlso pass effort (none|low|medium|high|xhigh|max) to set the spawned agent's reasoning effort.");
    out
}

fn format_swarm_fleet(swarms: &[SwarmFleetEntry]) -> ToolOutput {
    if swarms.is_empty() {
        return ToolOutput::new("No live swarms found.");
    }

    let mut out = String::from("Live swarms:\n\n");
    for swarm in swarms {
        let coordinator = swarm
            .coordinator_name
            .as_deref()
            .or(swarm.coordinator_session_id.as_deref())
            .unwrap_or("unknown");
        let coordinator_status = swarm.coordinator_status.as_deref().unwrap_or("unknown");
        let attention = if swarm.needs_operator_input {
            " attention"
        } else {
            ""
        };
        out.push_str(&format!(
            "- `{}`: {} member(s), coordinator `{}` ({}){}\n",
            swarm.swarm_id, swarm.member_count, coordinator, coordinator_status, attention
        ));
        if !swarm.members_by_status.is_empty() {
            let statuses = swarm
                .members_by_status
                .iter()
                .map(|(status, count)| format!("{status}:{count}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  - status: {statuses}\n"));
        }
        if !swarm.members_by_type.is_empty() {
            let types = swarm
                .members_by_type
                .iter()
                .map(|(kind, count)| format!("{kind}:{count}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  - type: {types}\n"));
        }
        out.push_str(&format!(
            "  - plan: {} item(s), {} active, {} ready, {} failed, mode {}\n",
            swarm.plan.item_count,
            swarm.plan.active_ids.len(),
            swarm.plan.ready_ids.len(),
            swarm.plan.failed_ids.len(),
            swarm.plan.mode
        ));
        if let Some(tokens) = &swarm.tokens {
            out.push_str(&format!(
                "  - tokens: in {}, out {}, messages {}\n",
                tokens.input_tokens, tokens.output_tokens, tokens.messages_with_token_usage
            ));
        }
        if let Some(age) = swarm.last_activity_age_secs {
            out.push_str(&format!("  - last activity: {age}s ago\n"));
        }
        if let Some(offset) = swarm.control_log_offset {
            out.push_str(&format!("  - control log offset: {offset}\n"));
        }
    }
    ToolOutput::new(out)
}

pub struct CommunicateTool {
    /// Full tool description including the user-tunable swarm prompt
    /// (model-routing guidance loaded from `swarm-prompt.md`). Computed once at
    /// registry construction so `description()` can hand out a borrowed str.
    description: String,
}

impl CommunicateTool {
    pub fn new() -> Self {
        const BASE_DESCRIPTION: &str = "Coordinate agents. Any agent can spawn child agents, and those children can spawn their own, forming a recursive spawn tree with no depth limit (growth is bounded only by the total swarm member cap). For spawn, prefer providing a prompt so the new agent starts with a concrete task instead of idling. Spawned/assigned agents automatically report their final response back to the agent that spawned them; you can stop any agent in the subtree you spawned.\n\nCommunication: prefer structural dataflow (task-graph artifacts via complete_node) over chat, and DMs for point-to-point coordination. broadcast reaches only your spawned subtree (whole swarm for the coordinator) and should be rare.";
        let swarm_prompt = crate::prompt::load_swarm_prompt(None);
        let description = if swarm_prompt.is_empty() {
            BASE_DESCRIPTION.to_string()
        } else {
            format!(
                "{BASE_DESCRIPTION}\n\nSwarm prompt (user-tunable via ~/.jcode/swarm-prompt.md):\n{swarm_prompt}"
            )
        };
        Self { description }
    }
}

#[async_trait]
impl Tool for CommunicateTool {
    fn name(&self) -> &str {
        "swarm"
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        schema::parameters_schema()
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput> {
        dispatch::execute(input, ctx).await
    }
}

#[cfg(test)]
#[path = "communicate_tests.rs"]
mod tests;
