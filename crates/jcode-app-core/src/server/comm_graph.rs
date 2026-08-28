//! Server handlers for the task-DAG mutation ops (seed/expand/complete/inject).
//!
//! These are the live counterparts of the validated engine ops in
//! `jcode_plan::dag`. Each handler lifts the swarm's current `VersionedPlan` into
//! a `TaskGraph` (via `jcode_plan::bridge`), applies the engine op (which enforces
//! acyclicity, ownership, gate insertion, and artifact validation), lowers the
//! result back into the plan, then persists and broadcasts using the existing
//! swarm machinery. This keeps a single source of truth and reuses the scheduler,
//! persistence, and TUI broadcast paths.

use super::{
    SwarmEvent, SwarmEventType, SwarmMember, SwarmState, VersionedPlan, broadcast_swarm_plan,
    fanout_session_event, persist_swarm_state_for, record_swarm_event,
};
use crate::protocol::{NotificationType, ServerEvent, TaskGraphNodeSpec};
use jcode_plan::SwarmTaskProgress;
use jcode_plan::bridge::{apply_task_graph, parse_kind, to_task_graph};
use jcode_plan::dag::{
    self, BudgetViolation, DagError, GraphBudget, HandoffArtifact, NodeSpec, NodeStatus, TaskGraph,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::sync::{RwLock, broadcast};

const PLAN_SAFETY_PROGRESS_KEY: &str = "__jcode_plan_safety_v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PlanSafetyPolicy {
    max_nodes: usize,
    max_lineage_depth: usize,
    max_gate_injections_per_gate: usize,
    max_wall_clock_ms: u64,
}

impl PlanSafetyPolicy {
    fn configured(plan: &VersionedPlan) -> Self {
        let agents = &crate::config::config().agents;
        Self {
            max_nodes: plan.max_nodes.unwrap_or(agents.swarm_max_graph_nodes),
            max_lineage_depth: agents.swarm_max_graph_lineage_depth,
            max_gate_injections_per_gate: agents.swarm_max_gate_injections_per_gate,
            max_wall_clock_ms: agents.swarm_max_graph_wall_clock_secs.saturating_mul(1_000),
        }
    }

    fn apply_to(&self, graph: &mut TaskGraph) {
        graph.max_nodes = Some(self.max_nodes);
        graph.max_lineage_depth = self.max_lineage_depth;
        graph.max_gate_injections_per_gate = self.max_gate_injections_per_gate;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PlanSafetyStatus {
    Running,
    PausedBudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PlanSafetyLedger {
    policy: PlanSafetyPolicy,
    started_at_unix_ms: u64,
    status: PlanSafetyStatus,
    pause: Option<BudgetViolation>,
}

impl PlanSafetyLedger {
    fn new(plan: &VersionedPlan, now_unix_ms: u64) -> Self {
        Self {
            policy: PlanSafetyPolicy::configured(plan),
            started_at_unix_ms: now_unix_ms,
            status: PlanSafetyStatus::Running,
            pause: None,
        }
    }
}

enum GraphMutationResult<T> {
    Applied(T),
    Rejected(String),
    Paused(BudgetViolation),
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn load_safety_ledger(plan: &VersionedPlan, now_unix_ms: u64) -> PlanSafetyLedger {
    plan.task_progress
        .get(PLAN_SAFETY_PROGRESS_KEY)
        .and_then(|progress| progress.checkpoint_summary.as_deref())
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_else(|| PlanSafetyLedger::new(plan, now_unix_ms))
}

fn store_safety_ledger(plan: &mut VersionedPlan, ledger: &PlanSafetyLedger) {
    let json = serde_json::to_string(ledger).expect("plan safety ledger must serialize");
    plan.task_progress.insert(
        PLAN_SAFETY_PROGRESS_KEY.to_string(),
        SwarmTaskProgress {
            started_at_unix_ms: Some(ledger.started_at_unix_ms),
            checkpoint_summary: Some(json),
            ..SwarmTaskProgress::default()
        },
    );
}

fn wall_clock_violation(ledger: &PlanSafetyLedger, now_unix_ms: u64) -> Option<BudgetViolation> {
    let elapsed_ms = now_unix_ms.saturating_sub(ledger.started_at_unix_ms);
    (elapsed_ms > ledger.policy.max_wall_clock_ms).then(|| BudgetViolation {
        budget: GraphBudget::WallClock,
        limit: ledger.policy.max_wall_clock_ms,
        observed: elapsed_ms,
        operation: "admitting task-graph growth".to_string(),
    })
}

fn pause_for_budget(
    plan: &mut VersionedPlan,
    mut ledger: PlanSafetyLedger,
    violation: BudgetViolation,
) -> GraphMutationResult<()> {
    ledger.status = PlanSafetyStatus::PausedBudgetExceeded;
    ledger.pause = Some(violation.clone());
    store_safety_ledger(plan, &ledger);
    plan.frozen = true;
    plan.version = plan.version.saturating_add(1);
    GraphMutationResult::Paused(violation)
}

fn mutate_plan_with_budget<T>(
    plan: &mut VersionedPlan,
    now_unix_ms: u64,
    mutation: impl FnOnce(&mut TaskGraph) -> Result<T, DagError>,
) -> GraphMutationResult<T> {
    let ledger = load_safety_ledger(plan, now_unix_ms);
    if ledger.status == PlanSafetyStatus::PausedBudgetExceeded {
        let reason = ledger
            .pause
            .as_ref()
            .map(budget_pause_message)
            .unwrap_or_else(|| "task graph is paused because a hard budget was exceeded".into());
        return GraphMutationResult::Rejected(reason);
    }
    if plan.frozen {
        return GraphMutationResult::Rejected(
            "Task graph is frozen. Existing assigned work may complete, but the scheduler rejects new graph growth until the coordinator unfreezes it."
                .to_string(),
        );
    }
    if let Some(violation) = wall_clock_violation(&ledger, now_unix_ms) {
        return match pause_for_budget(plan, ledger, violation) {
            GraphMutationResult::Paused(violation) => GraphMutationResult::Paused(violation),
            _ => unreachable!("pause_for_budget always pauses"),
        };
    }

    let mut graph = to_task_graph(plan);
    ledger.policy.apply_to(&mut graph);
    match mutation(&mut graph) {
        Ok(value) => {
            apply_task_graph(plan, &graph);
            store_safety_ledger(plan, &ledger);
            plan.version = plan.version.saturating_add(1);
            GraphMutationResult::Applied(value)
        }
        Err(DagError::BudgetExceeded(violation)) => {
            match pause_for_budget(plan, ledger, violation) {
                GraphMutationResult::Paused(violation) => GraphMutationResult::Paused(violation),
                _ => unreachable!("pause_for_budget always pauses"),
            }
        }
        Err(error) => GraphMutationResult::Rejected(error.to_string()),
    }
}

fn budget_pause_message(violation: &BudgetViolation) -> String {
    let budget = match violation.budget {
        GraphBudget::Nodes => "node count",
        GraphBudget::LineageDepth => "lineage depth",
        GraphBudget::GateInjections => "gate injection quota",
        GraphBudget::WallClock => "wall clock",
    };
    format!(
        "Task graph paused: hard {budget} budget exceeded while {} (limit {}, observed {}). The scheduler rejected the mutation. Inspect the graph and start a smaller replacement plan; ordinary unfreeze cannot bypass an exhausted budget.",
        violation.operation, violation.limit, violation.observed
    )
}

fn initialize_safety_ledger(plan: &mut VersionedPlan, now_unix_ms: u64) -> PlanSafetyLedger {
    let ledger = PlanSafetyLedger::new(plan, now_unix_ms);
    plan.max_nodes = Some(ledger.policy.max_nodes);
    store_safety_ledger(plan, &ledger);
    ledger
}

fn force_completion_closes_graph_growth(plan: &mut VersionedPlan) {
    plan.frozen = true;
}

fn spec_from_wire(spec: TaskGraphNodeSpec) -> NodeSpec {
    NodeSpec {
        id: Some(spec.id),
        content: spec.content,
        kind: parse_kind(spec.kind.as_deref()),
        depends_on: spec.depends_on,
        priority: spec.priority,
    }
}

async fn swarm_id_for(
    session_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
) -> Option<String> {
    swarm_members
        .read()
        .await
        .get(session_id)
        .and_then(|member| member.swarm_id.clone())
}

/// Ensure the seeding session can actually drive the graph it just created.
///
/// Deep-mode sessions are frequently solo `agent`s with no coordinator elected,
/// yet `assign_task` / `assign_next` / `run_plan` are coordinator-gated. Without
/// this, a fresh deep-mode agent can seed a task graph but then cannot dispatch
/// any of it. We elect the seeder as coordinator when the swarm has no *live*
/// coordinator, mirroring the self-promote rule used by `assign_role`. A live,
/// non-headless coordinator is left untouched so a real coordinator is never
/// displaced by a worker that happens to seed.
///
/// Returns true when the seeder was (or already is) the coordinator afterwards.
async fn ensure_seeder_can_coordinate(
    swarm_id: &str,
    seeder_session_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
) -> bool {
    // 1. Read the current coordinator id without holding the lock across the
    //    liveness check (matches the non-nested lock pattern used elsewhere).
    let current = swarm_coordinators.read().await.get(swarm_id).cloned();
    match &current {
        Some(coord) if coord == seeder_session_id => return true,
        _ => {}
    }

    // 2. Decide whether the existing coordinator is still a live driver.
    let coordinator_is_live = match &current {
        Some(coord) => {
            let members = swarm_members.read().await;
            members
                .get(coord)
                .map(|member| !member.event_tx.is_closed() && !member.is_headless)
                .unwrap_or(false)
        }
        None => false,
    };
    if coordinator_is_live {
        return false;
    }

    // 3. Promote the seeder; demote any prior (stale) coordinator member. Re-check
    //    under the write lock that the coordinator is still the one we inspected
    //    (compare-and-swap): two concurrent seeders race here, and the loser must
    //    not silently displace the winner it never liveness-checked.
    let prior = {
        let mut coordinators = swarm_coordinators.write().await;
        if coordinators.get(swarm_id) != current.as_ref() {
            // Someone else changed the coordinator between our read and write.
            return coordinators.get(swarm_id).map(String::as_str) == Some(seeder_session_id);
        }
        coordinators.insert(swarm_id.to_string(), seeder_session_id.to_string())
    };
    {
        let mut members = swarm_members.write().await;
        if let Some(member) = members.get_mut(seeder_session_id) {
            member.role = "coordinator".to_string();
        }
        if let Some(prior) = prior
            && prior != seeder_session_id
            && let Some(member) = members.get_mut(&prior)
        {
            member.role = "agent".to_string();
        }
    }
    true
}

/// Auto-claim a queued node for the participant that is trying to mutate it.
///
/// Seeded nodes are unowned until dispatch, but the deep-mode contract tells the
/// seeding agent to `expand_node`/`complete_node` its own nodes, and the assign
/// path refuses self-assignment — so without this a solo deep seeder could never
/// legally touch any node it seeded (observed live as "Complete rejected: actor
/// does not own node"). Similarly, assignment to a client-attached worker leaves
/// the item `queued` (the server-run flip to `running` is skipped when a live
/// client owns the turn), so the assignee's own complete/expand would bounce with
/// "invalid state Queued".
///
/// Claiming is safe only when the node is genuinely available to this actor:
/// queued, with every dependency done (enforced by `dispatch`), and either
/// unowned or already assigned to this same actor. A node owned by someone else
/// is never touched — the engine's `NotOwner` check still applies.
fn claim_queued_node_for_actor(graph: &mut TaskGraph, node_id: &str, actor: &str) {
    let claimable = graph.get(node_id).is_some_and(|node| {
        node.status == NodeStatus::Queued
            && node.owner.as_deref().is_none_or(|owner| owner == actor)
    });
    if claimable {
        // `dispatch` re-validates queued status and dependency satisfaction; if
        // deps are not done the claim is skipped and the engine op reports the
        // real error.
        let _ = dag::dispatch(graph, node_id, actor);
    }
}

fn err(client_event_tx: &mpsc::UnboundedSender<ServerEvent>, id: u64, message: String) {
    let _ = client_event_tx.send(ServerEvent::Error {
        id,
        message,
        retry_after_secs: None,
    });
}

/// Shared finalize: persist, broadcast, record a plan-update event, and ack.
#[expect(
    clippy::too_many_arguments,
    reason = "finalize threads through swarm persistence, broadcast, and event-history handles"
)]
async fn finalize(
    id: u64,
    swarm_id: &str,
    req_session_id: &str,
    reason: &str,
    item_count: usize,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    event_history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    event_counter: &Arc<std::sync::atomic::AtomicU64>,
    swarm_event_tx: &broadcast::Sender<SwarmEvent>,
) {
    let from_name = swarm_members
        .read()
        .await
        .get(req_session_id)
        .and_then(|member| member.friendly_name.clone());

    let swarm_state = SwarmState {
        members: Arc::clone(swarm_members),
        swarms_by_id: Arc::clone(swarms_by_id),
        plans: Arc::clone(swarm_plans),
        coordinators: Arc::clone(swarm_coordinators),
    };
    persist_swarm_state_for(swarm_id, &swarm_state).await;
    broadcast_swarm_plan(
        swarm_id,
        Some(reason.to_string()),
        swarm_plans,
        swarm_members,
        swarms_by_id,
    )
    .await;
    record_swarm_event(
        event_history,
        event_counter,
        swarm_event_tx,
        req_session_id.to_string(),
        from_name,
        Some(swarm_id.to_string()),
        SwarmEventType::PlanUpdate {
            swarm_id: swarm_id.to_string(),
            item_count,
        },
    )
    .await;
    let _ = client_event_tx.send(ServerEvent::Done { id });
}

#[expect(
    clippy::too_many_arguments,
    reason = "budget pause publishes through the same persistence and event handles as graph mutations"
)]
async fn publish_budget_pause(
    id: u64,
    swarm_id: &str,
    req_session_id: &str,
    violation: &BudgetViolation,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    event_history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    event_counter: &Arc<std::sync::atomic::AtomicU64>,
    swarm_event_tx: &broadcast::Sender<SwarmEvent>,
) {
    let message = budget_pause_message(violation);
    let swarm_state = SwarmState {
        members: Arc::clone(swarm_members),
        swarms_by_id: Arc::clone(swarms_by_id),
        plans: Arc::clone(swarm_plans),
        coordinators: Arc::clone(swarm_coordinators),
    };
    persist_swarm_state_for(swarm_id, &swarm_state).await;
    broadcast_swarm_plan(
        swarm_id,
        Some("task_graph_budget_pause".to_string()),
        swarm_plans,
        swarm_members,
        swarms_by_id,
    )
    .await;
    let item_count = swarm_plans
        .read()
        .await
        .get(swarm_id)
        .map(|plan| plan.items.len())
        .unwrap_or(0);
    record_swarm_event(
        event_history,
        event_counter,
        swarm_event_tx,
        req_session_id.to_string(),
        None,
        Some(swarm_id.to_string()),
        SwarmEventType::PlanUpdate {
            swarm_id: swarm_id.to_string(),
            item_count,
        },
    )
    .await;

    if let Some(coordinator_id) = swarm_coordinators.read().await.get(swarm_id).cloned() {
        let _ = fanout_session_event(
            swarm_members,
            &coordinator_id,
            ServerEvent::Notification {
                from_session: "task-graph-scheduler".to_string(),
                from_name: Some("Task graph scheduler".to_string()),
                notification_type: NotificationType::Message {
                    scope: Some("dm".to_string()),
                    tldr: Some("task graph paused after a hard budget was exceeded".to_string()),
                },
                message: message.clone(),
            },
        )
        .await;
    }
    err(client_event_tx, id, message);
}

/// Seed (or re-seed) the swarm task DAG from a batch of node specs.
#[expect(
    clippy::too_many_arguments,
    reason = "swarm op threads runtime handles"
)]
pub(super) async fn handle_comm_seed_graph(
    id: u64,
    req_session_id: String,
    mode: Option<String>,
    replace_existing: bool,
    nodes: Vec<TaskGraphNodeSpec>,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    event_history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    event_counter: &Arc<std::sync::atomic::AtomicU64>,
    swarm_event_tx: &broadcast::Sender<SwarmEvent>,
) {
    let Some(swarm_id) = swarm_id_for(&req_session_id, swarm_members).await else {
        err(client_event_tx, id, "Not in a swarm.".to_string());
        return;
    };

    let specs: Vec<NodeSpec> = nodes.into_iter().map(spec_from_wire).collect();
    let count = specs.len();
    let mut replacement_participants = swarms_by_id
        .read()
        .await
        .get(&swarm_id)
        .cloned()
        .unwrap_or_else(HashSet::new);
    replacement_participants.insert(req_session_id.clone());

    // Resolve the plan mode. The model is *asked* to pass `mode:"deep"` when it is
    // running at `swarm-deep` effort, but it frequently forgets. Rather than
    // silently downgrading a deep-effort session to light (which disables the
    // gates + artifact validation that define deep mode), default the mode from
    // the seeder's recorded reasoning effort when the caller did not specify one.
    // An explicit `mode` always wins so a caller can still opt into light.
    let resolved_mode = mode.or_else(|| {
        crate::session_effort::session_effort(&req_session_id)
            .filter(|effort| crate::prompt::is_deep_swarm_effort(effort))
            .map(|_| "deep".to_string())
    });

    let result = {
        let mut plans = swarm_plans.write().await;
        let plan = plans
            .entry(swarm_id.clone())
            .or_insert_with(VersionedPlan::new);
        if plan.frozen {
            err(
                client_event_tx,
                id,
                "Seed rejected: this task graph is frozen. Ask the coordinator to call `swarm` with `action:\"unfreeze\"`, then retry seeding. Existing assigned work may still be completed while growth is frozen."
                    .to_string(),
            );
            return;
        }
        if !plan.items.is_empty() && !replace_existing {
            err(
                client_event_tx,
                id,
                format!(
                    "Seed rejected: this swarm already has {} node(s). Use expand_node/inject_gap to extend it, or retry task_graph with replace_existing=true to start a fresh graph.",
                    plan.items.len()
                ),
            );
            return;
        }
        let in_flight: Vec<&str> = plan
            .items
            .iter()
            .filter(|item| {
                let terminal = matches!(
                    item.status.as_str(),
                    "completed" | "done" | "failed" | "stopped" | "crashed"
                );
                matches!(item.status.as_str(), "running" | "running_stale")
                    || (item.assigned_to.is_some() && !terminal)
            })
            .map(|item| item.id.as_str())
            .collect();
        if replace_existing && !in_flight.is_empty() {
            err(
                client_event_tx,
                id,
                format!(
                    "Seed rejected: cannot replace a graph with in-flight node(s): {}. Complete, fail, stop, or requeue them before starting a fresh graph.",
                    in_flight.join(", ")
                ),
            );
            return;
        }

        // Seed into a fresh plan rather than lifting the persisted graph. This is
        // the lifecycle boundary that prevents old nodes, node_meta, progress, or
        // participants from leaking into a new workflow. Preserve only the
        // monotonic version so clients cannot mistake replacement for stale data.
        let mut replacement = VersionedPlan::new();
        replacement.version = plan.version;
        replacement.max_nodes = Some(crate::config::config().agents.swarm_max_graph_nodes);
        if let Some(mode) = resolved_mode {
            replacement.mode = mode;
        }
        replacement.participants = replacement_participants;
        let ledger = initialize_safety_ledger(&mut replacement, unix_now_ms());
        let mut graph = to_task_graph(&replacement);
        ledger.policy.apply_to(&mut graph);
        match dag::seed(&mut graph, specs) {
            Ok(()) => {
                apply_task_graph(&mut replacement, &graph);
                replacement.version = replacement.version.saturating_add(1);
                *plan = replacement;
                Ok(())
            }
            Err(e) => Err(e),
        }
    };

    match result {
        Ok(()) => {
            // A deep-mode seeder is usually a solo agent. Elect it coordinator
            // only after the seed succeeds so rejected calls cannot mutate roles.
            ensure_seeder_can_coordinate(
                &swarm_id,
                &req_session_id,
                swarm_members,
                swarm_coordinators,
            )
            .await;
            finalize(
                id,
                &swarm_id,
                &req_session_id,
                "task_graph_seed",
                count,
                client_event_tx,
                swarm_members,
                swarms_by_id,
                swarm_plans,
                swarm_coordinators,
                event_history,
                event_counter,
                swarm_event_tx,
            )
            .await;
        }
        Err(e) => err(client_event_tx, id, format!("Seed rejected: {e}")),
    }
}

/// Decompose a node the caller owns into a child sub-DAG.
#[expect(
    clippy::too_many_arguments,
    reason = "swarm op threads runtime handles"
)]
pub(super) async fn handle_comm_expand_node(
    id: u64,
    req_session_id: String,
    node_id: String,
    children: Vec<TaskGraphNodeSpec>,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    event_history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    event_counter: &Arc<std::sync::atomic::AtomicU64>,
    swarm_event_tx: &broadcast::Sender<SwarmEvent>,
) {
    let Some(swarm_id) = swarm_id_for(&req_session_id, swarm_members).await else {
        err(client_event_tx, id, "Not in a swarm.".to_string());
        return;
    };
    let specs: Vec<NodeSpec> = children.into_iter().map(spec_from_wire).collect();
    let count = specs.len();

    let result = {
        let mut plans = swarm_plans.write().await;
        let Some(plan) = plans.get_mut(&swarm_id) else {
            err(client_event_tx, id, "No plan for this swarm.".to_string());
            return;
        };
        mutate_plan_with_budget(plan, unix_now_ms(), |graph| {
            claim_queued_node_for_actor(&mut graph, &node_id, &req_session_id);
            dag::expand_node(graph, &node_id, &req_session_id, specs)
        })
    };

    match result {
        GraphMutationResult::Applied(_) => {
            finalize(
                id,
                &swarm_id,
                &req_session_id,
                "task_graph_expand",
                count,
                client_event_tx,
                swarm_members,
                swarms_by_id,
                swarm_plans,
                swarm_coordinators,
                event_history,
                event_counter,
                swarm_event_tx,
            )
            .await;
        }
        GraphMutationResult::Rejected(error) => {
            err(client_event_tx, id, format!("Expand rejected: {error}"));
        }
        GraphMutationResult::Paused(violation) => {
            publish_budget_pause(
                id,
                &swarm_id,
                &req_session_id,
                &violation,
                client_event_tx,
                swarm_members,
                swarms_by_id,
                swarm_plans,
                swarm_coordinators,
                event_history,
                event_counter,
                swarm_event_tx,
            )
            .await;
        }
    }
}

/// Complete a node the caller owns with a typed handoff artifact.
#[expect(
    clippy::too_many_arguments,
    reason = "swarm op threads runtime handles"
)]
pub(super) async fn handle_comm_complete_node(
    id: u64,
    req_session_id: String,
    node_id: String,
    artifact_json: String,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    event_history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    event_counter: &Arc<std::sync::atomic::AtomicU64>,
    swarm_event_tx: &broadcast::Sender<SwarmEvent>,
) {
    let Some(swarm_id) = swarm_id_for(&req_session_id, swarm_members).await else {
        err(client_event_tx, id, "Not in a swarm.".to_string());
        return;
    };

    let artifact: HandoffArtifact = match serde_json::from_str(&artifact_json) {
        Ok(artifact) => artifact,
        Err(e) => {
            err(client_event_tx, id, format!("Invalid artifact JSON: {e}"));
            return;
        }
    };
    // W2: capture the evidence summary for the control log before the
    // artifact is consumed by the engine op.
    let artifact_confidence = artifact.confidence.clone();

    // F3 salvage policy: a coordinator may complete a node whose recorded
    // owner is no longer a live swarm member. Without this, a crashed or
    // evicted worker wedges its running node forever (complete/fail are
    // owner-only, requeue requires Failed). The engine op (take_over_node)
    // enforces only mechanics; being the coordinator + the owner being gone is
    // the policy, decided here where membership is visible.
    let salvage_takeover_allowed = {
        let coordinators = swarm_coordinators.read().await;
        let is_coordinator = coordinators
            .get(&swarm_id)
            .map(|coordinator| coordinator == &req_session_id)
            .unwrap_or(false);
        if is_coordinator {
            let members = swarm_members.read().await;
            let plans = swarm_plans.read().await;
            plans
                .get(&swarm_id)
                .and_then(|plan| plan.items.iter().find(|item| item.id == node_id))
                .and_then(|item| item.assigned_to.as_ref())
                .map(|owner| owner != &req_session_id && !members.contains_key(owner))
                .unwrap_or(false)
        } else {
            false
        }
    };

    let result = {
        let mut plans = swarm_plans.write().await;
        let Some(plan) = plans.get_mut(&swarm_id) else {
            err(client_event_tx, id, "No plan for this swarm.".to_string());
            return;
        };
        let mut graph = to_task_graph(plan);
        claim_queued_node_for_actor(&mut graph, &node_id, &req_session_id);
        if salvage_takeover_allowed {
            let _ = dag::take_over_node(&mut graph, &node_id, &req_session_id);
        }
        match dag::complete_node(&mut graph, &node_id, &req_session_id, artifact) {
            Ok(()) => {
                apply_task_graph(plan, &graph);
                if salvage_takeover_allowed {
                    force_completion_closes_graph_growth(plan);
                }
                plan.version += 1;
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    };

    match result {
        Ok(()) => {
            // W2: file the completion evidence in the control log BEFORE the
            // finalize sync, so awaiters see ArtifactFiled ordered ahead of
            // the derived TaskStatusChanged("done") for the same completion.
            super::control_log_sync::append_control_event(
                &swarm_id,
                jcode_swarm_core::control_log::SwarmControlEvent::ArtifactFiled {
                    task_id: node_id.clone(),
                    session_id: req_session_id.clone(),
                    confidence: artifact_confidence,
                },
            );
            finalize(
                id,
                &swarm_id,
                &req_session_id,
                "task_graph_complete",
                1,
                client_event_tx,
                swarm_members,
                swarms_by_id,
                swarm_plans,
                swarm_coordinators,
                event_history,
                event_counter,
                swarm_event_tx,
            )
            .await;
        }
        Err(e) => err(client_event_tx, id, format!("Complete rejected: {e}")),
    }
}

/// Inject gap/fix nodes from a gate the caller owns.
#[expect(
    clippy::too_many_arguments,
    reason = "swarm op threads runtime handles"
)]
pub(super) async fn handle_comm_inject_gap(
    id: u64,
    req_session_id: String,
    gate_id: String,
    nodes: Vec<TaskGraphNodeSpec>,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    event_history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    event_counter: &Arc<std::sync::atomic::AtomicU64>,
    swarm_event_tx: &broadcast::Sender<SwarmEvent>,
) {
    let Some(swarm_id) = swarm_id_for(&req_session_id, swarm_members).await else {
        err(client_event_tx, id, "Not in a swarm.".to_string());
        return;
    };
    let specs: Vec<NodeSpec> = nodes.into_iter().map(spec_from_wire).collect();
    let count = specs.len();

    let result = {
        let mut plans = swarm_plans.write().await;
        let Some(plan) = plans.get_mut(&swarm_id) else {
            err(client_event_tx, id, "No plan for this swarm.".to_string());
            return;
        };
        mutate_plan_with_budget(plan, unix_now_ms(), |graph| {
            claim_queued_node_for_actor(&mut graph, &gate_id, &req_session_id);
            dag::inject_from_gate(graph, &gate_id, &req_session_id, specs)
        })
    };

    match result {
        GraphMutationResult::Applied(_) => {
            finalize(
                id,
                &swarm_id,
                &req_session_id,
                "task_graph_inject_gap",
                count,
                client_event_tx,
                swarm_members,
                swarms_by_id,
                swarm_plans,
                swarm_coordinators,
                event_history,
                event_counter,
                swarm_event_tx,
            )
            .await;
        }
        GraphMutationResult::Rejected(error) => {
            err(client_event_tx, id, format!("Inject rejected: {error}"));
        }
        GraphMutationResult::Paused(violation) => {
            publish_budget_pause(
                id,
                &swarm_id,
                &req_session_id,
                &violation,
                client_event_tx,
                swarm_members,
                swarms_by_id,
                swarm_plans,
                swarm_coordinators,
                event_history,
                event_counter,
                swarm_event_tx,
            )
            .await;
        }
    }
}

/// Freeze or unfreeze graph growth without stopping work already in flight.
///
/// The DAG engine owns graph mechanics, but coordinator authority is a live
/// swarm policy: membership and the elected coordinator are only visible here.
/// Keep that policy at the server boundary, matching coordinator salvage.
#[expect(
    clippy::too_many_arguments,
    reason = "graph control threads runtime handles for persistence and broadcast"
)]
pub(super) async fn handle_comm_graph_freeze(
    id: u64,
    req_session_id: String,
    frozen: bool,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
    event_history: &Arc<RwLock<std::collections::VecDeque<SwarmEvent>>>,
    event_counter: &Arc<std::sync::atomic::AtomicU64>,
    swarm_event_tx: &broadcast::Sender<SwarmEvent>,
) {
    let Some(swarm_id) = swarm_id_for(&req_session_id, swarm_members).await else {
        err(client_event_tx, id, "Not in a swarm.".to_string());
        return;
    };
    let is_coordinator = swarm_coordinators
        .read()
        .await
        .get(&swarm_id)
        .is_some_and(|coordinator| coordinator == &req_session_id);
    if !is_coordinator {
        err(
            client_event_tx,
            id,
            format!(
                "Only the coordinator can {} task-graph growth. Ask the coordinator to call `swarm` with `action:\"{}\"`.",
                if frozen { "freeze" } else { "unfreeze" },
                if frozen { "freeze" } else { "unfreeze" }
            ),
        );
        return;
    }

    {
        let mut plans = swarm_plans.write().await;
        let Some(plan) = plans.get_mut(&swarm_id) else {
            err(client_event_tx, id, "No plan for this swarm.".to_string());
            return;
        };
        if !frozen {
            let ledger = load_safety_ledger(plan, unix_now_ms());
            if ledger.status == PlanSafetyStatus::PausedBudgetExceeded {
                err(
                    client_event_tx,
                    id,
                    ledger.pause.as_ref().map(budget_pause_message).unwrap_or_else(|| {
                        "Task graph remains paused because a hard budget was exceeded. Start a smaller replacement plan; ordinary unfreeze cannot bypass the exhausted budget."
                            .to_string()
                    }),
                );
                return;
            }
        }
        if frozen {
            let mut ledger = load_safety_ledger(plan, unix_now_ms());
            if let Some(violation) = wall_clock_violation(&ledger, unix_now_ms()) {
                ledger.status = PlanSafetyStatus::PausedBudgetExceeded;
                ledger.pause = Some(violation);
                store_safety_ledger(plan, &ledger);
            }
        }
        if plan.frozen != frozen {
            plan.frozen = frozen;
            plan.version = plan.version.saturating_add(1);
        }
    }

    finalize(
        id,
        &swarm_id,
        &req_session_id,
        if frozen {
            "task_graph_freeze"
        } else {
            "task_graph_unfreeze"
        },
        swarm_plans
            .read()
            .await
            .get(&swarm_id)
            .map(|plan| plan.items.len())
            .unwrap_or(0),
        client_event_tx,
        swarm_members,
        swarms_by_id,
        swarm_plans,
        swarm_coordinators,
        event_history,
        event_counter,
        swarm_event_tx,
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{NotificationType, SwarmMemberRuntime};
    use jcode_plan::bridge::apply_task_graph;
    use jcode_plan::dag::{Mode, NodeKind};
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicU64;
    use std::time::Instant;

    fn member(
        session_id: &str,
        swarm_id: &str,
        role: &str,
    ) -> (SwarmMember, mpsc::UnboundedReceiver<ServerEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        (
            SwarmMember {
                session_id: session_id.to_string(),
                event_tx,
                event_txs: HashMap::new(),
                working_dir: None,
                swarm_id: Some(swarm_id.to_string()),
                swarm_enabled: true,
                status: "ready".to_string(),
                detail: None,
                task_label: None,
                subagent_type: None,
                friendly_name: Some(session_id.to_string()),
                report_back_to_session_id: None,
                initial_prompt_delivered: None,
                latest_completion_report: None,
                role: role.to_string(),
                joined_at: Instant::now(),
                last_status_change: Instant::now(),
                is_headless: false,
                output_tail: None,
                todo_progress: None,
                todo_items: Vec::new(),
                runtime: SwarmMemberRuntime::default(),
            },
            event_rx,
        )
    }

    fn spec(id: &str) -> TaskGraphNodeSpec {
        TaskGraphNodeSpec {
            id: id.to_string(),
            content: format!("task {id}"),
            kind: Some("explore".to_string()),
            depends_on: Vec::new(),
            priority: 0,
        }
    }

    #[tokio::test]
    async fn node_budget_overrun_pauses_plan_and_wakes_coordinator_without_growth() {
        let swarm_id = "swarm-budget".to_string();
        let coordinator_id = "coord-budget".to_string();
        let worker_id = "worker-budget".to_string();
        let (coordinator, mut coordinator_rx) = member(&coordinator_id, &swarm_id, "coordinator");
        let (worker, _worker_event_rx) = member(&worker_id, &swarm_id, "agent");
        let swarm_members = Arc::new(RwLock::new(HashMap::from([
            (coordinator_id.clone(), coordinator),
            (worker_id.clone(), worker),
        ])));
        let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
            swarm_id.clone(),
            HashSet::from([coordinator_id.clone(), worker_id.clone()]),
        )])));

        let mut graph = TaskGraph::new(Mode::Light);
        graph.max_nodes = Some(2);
        dag::seed(
            &mut graph,
            vec![
                NodeSpec::new("root", "root task", NodeKind::Explore),
                NodeSpec::new("other", "other task", NodeKind::Explore),
            ],
        )
        .unwrap();
        assert!(dag::dispatch(&mut graph, "root", &worker_id));
        let mut plan = VersionedPlan::new();
        apply_task_graph(&mut plan, &graph);
        let swarm_plans = Arc::new(RwLock::new(HashMap::from([(swarm_id.clone(), plan)])));
        let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
            swarm_id.clone(),
            coordinator_id.clone(),
        )])));
        let event_history = Arc::new(RwLock::new(VecDeque::new()));
        let event_counter = Arc::new(AtomicU64::new(1));
        let swarm_event_tx = broadcast::channel(16).0;
        let (client_tx, mut client_rx) = mpsc::unbounded_channel();

        handle_comm_expand_node(
            1,
            worker_id,
            "root".to_string(),
            vec![spec("too-many")],
            &client_tx,
            &swarm_members,
            &swarms_by_id,
            &swarm_plans,
            &swarm_coordinators,
            &event_history,
            &event_counter,
            &swarm_event_tx,
        )
        .await;

        let plan = swarm_plans.read().await;
        assert_eq!(
            plan[&swarm_id].items.len(),
            2,
            "rejected growth must not land"
        );
        assert!(
            plan[&swarm_id].frozen,
            "budget exhaustion must pause the plan"
        );
        drop(plan);

        assert!(matches!(
            client_rx.recv().await,
            Some(ServerEvent::Error { .. })
        ));
        let mut woke_coordinator = false;
        while let Ok(event) = coordinator_rx.try_recv() {
            if matches!(
                event,
                ServerEvent::Notification {
                    notification_type: NotificationType::Message { .. },
                    ..
                }
            ) {
                woke_coordinator = true;
            }
        }
        assert!(
            woke_coordinator,
            "budget exhaustion must wake the coordinator"
        );
    }
}
