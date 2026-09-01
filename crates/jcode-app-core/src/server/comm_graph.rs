//! Server handlers for the task-DAG mutation ops (seed/expand/complete/inject).
//! Mutations run through `jcode_plan::dag`, then persist and broadcast.

use super::{
    SwarmEvent, SwarmEventType, SwarmMember, SwarmState, VersionedPlan, broadcast_swarm_plan,
    fanout_session_event, persist_swarm_state_for, record_swarm_event,
};
use crate::protocol::{NotificationType, ServerEvent, TaskGraphNodeSpec};
use jcode_plan::NodeMeta;
use jcode_plan::bridge::{apply_task_graph, parse_kind, to_task_graph};
use jcode_plan::dag::{
    self, BudgetViolation, DagError, GraphBudget, HandoffArtifact, NodeSpec, NodeStatus,
    PlanSafetyLedger, PlanSafetyPolicy, PlanSafetyStatus, TaskGraph,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::sync::{RwLock, broadcast};

enum GraphMutationResult<T> {
    Applied(T),
    Rejected(String),
    Paused(BudgetViolation),
}

pub(super) fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

/// Resolve the budget a new plan runs under from current configuration.
pub(super) fn configured_policy(plan: &VersionedPlan) -> PlanSafetyPolicy {
    let agents = &crate::config::config().agents;
    PlanSafetyPolicy {
        max_nodes: plan.max_nodes.unwrap_or(agents.swarm_max_graph_nodes),
        max_lineage_depth: agents.swarm_max_graph_lineage_depth,
        max_gate_injections_per_gate: agents.swarm_max_gate_injections_per_gate,
        max_wall_clock_ms: agents.swarm_max_graph_wall_clock_secs.saturating_mul(1_000),
    }
}

/// Read the plan's ledger, starting one under current configuration if the plan
/// predates budgets. A missing ledger is an ordinary first run, not an error:
/// refusing to proceed would strand every plan created before this feature.
fn load_safety_ledger(plan: &VersionedPlan, now_unix_ms: u64) -> PlanSafetyLedger {
    plan.safety_ledger
        .clone()
        .unwrap_or_else(|| PlanSafetyLedger::started(configured_policy(plan), now_unix_ms))
}

pub(super) fn store_safety_ledger(plan: &mut VersionedPlan, ledger: &PlanSafetyLedger) {
    plan.safety_ledger = Some(ledger.clone());
    // Mirror the wall-clock window into node metadata as well: that is the
    // surface `run_plan` reads to decide whether a graph still has time left.
    plan.node_meta.insert(
        dag::PLAN_SAFETY_STATUS_META_ID.to_string(),
        NodeMeta {
            kind: Some(format!(
                "{}:{}",
                ledger.started_at_unix_ms, ledger.policy.max_wall_clock_ms
            )),
            ..NodeMeta::default()
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

fn seed_replacement_with_budget(
    plan: &mut VersionedPlan,
    mut replacement: VersionedPlan,
    specs: Vec<NodeSpec>,
    ledger: PlanSafetyLedger,
) -> GraphMutationResult<u64> {
    let mut graph = to_task_graph(&replacement);
    ledger.policy.apply_to(&mut graph);
    match dag::seed(&mut graph, specs) {
        Ok(()) => {
            apply_task_graph(&mut replacement, &graph);
            store_safety_ledger(&mut replacement, &ledger);
            replacement.version = replacement.version.saturating_add(1);
            *plan = replacement;
            GraphMutationResult::Applied(ledger.started_at_unix_ms)
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

/// Give a plan a budget clock if it does not have one yet.
///
/// `run_plan` refuses to drive a plan with no persisted ledger, and deriving
/// one at read time would restart the clock on every call. So every path that
/// creates or updates a plan persists one exactly once, here.
pub(super) fn ensure_safety_ledger(plan: &mut VersionedPlan, now_unix_ms: u64) {
    if plan.safety_ledger.is_some() {
        return;
    }
    let ledger = PlanSafetyLedger::started(configured_policy(plan), now_unix_ms);
    store_safety_ledger(plan, &ledger);
}

/// Refusal for the coordinator's direct `plan.items` replacement.
///
/// That path rewrites the item list wholesale instead of growing a graph, so it
/// never reaches `mutate_plan_with_budget` and inherited none of its stops. That
/// made the hard pause bypassable: a paused coordinator could re-propose an
/// unbounded list and keep dispatching, because assignment reads `plan.items`
/// without consulting the ledger. Apply the same order of stops here.
pub(super) fn direct_plan_update_refusal(
    plan: &mut VersionedPlan,
    incoming_items: usize,
    now_unix_ms: u64,
) -> Option<String> {
    ensure_safety_ledger(plan, now_unix_ms);
    let ledger = load_safety_ledger(plan, now_unix_ms);
    if ledger.status == PlanSafetyStatus::PausedBudgetExceeded {
        return Some(
            ledger
                .pause
                .as_ref()
                .map(budget_pause_message)
                .unwrap_or_else(|| {
                    "task graph is paused because a hard budget was exceeded".into()
                }),
        );
    }
    if plan.frozen {
        return Some(
            "Plan is frozen. Existing assigned work may complete, but the scheduler rejects new plan items until the coordinator unfreezes it."
                .to_string(),
        );
    }
    if let Some(violation) = wall_clock_violation(&ledger, now_unix_ms) {
        let message = budget_pause_message(&violation);
        pause_for_budget(plan, ledger, violation);
        return Some(message);
    }
    let max_nodes = ledger.policy.max_nodes;
    if incoming_items > max_nodes {
        return Some(format!(
            "plan node budget is {max_nodes}; this update proposes {incoming_items} items. Split the work across graphs or raise `agents.swarm_max_graph_nodes`."
        ));
    }
    None
}

fn initialize_safety_ledger(plan: &mut VersionedPlan, now_unix_ms: u64) -> PlanSafetyLedger {
    let ledger = PlanSafetyLedger::started(configured_policy(plan), now_unix_ms);
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

/// Elect a solo seeder without displacing a live coordinator.
async fn ensure_seeder_can_coordinate(
    swarm_id: &str,
    seeder_session_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
) -> bool {
    let current = swarm_coordinators.read().await.get(swarm_id).cloned();
    match &current {
        Some(coord) if coord == seeder_session_id => return true,
        _ => {}
    }

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

    // Re-check under the write lock so concurrent seeders cannot displace a winner.
    let prior = {
        let mut coordinators = swarm_coordinators.write().await;
        if coordinators.get(swarm_id) != current.as_ref() {
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

/// Claim an available queued node before its actor mutates it.
fn claim_queued_node_for_actor(graph: &mut TaskGraph, node_id: &str, actor: &str) {
    let claimable = graph.get(node_id).is_some_and(|node| {
        node.status == NodeStatus::Queued
            && node.owner.as_deref().is_none_or(|owner| owner == actor)
    });
    if claimable {
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

    let coordinator_id = swarm_coordinators
        .read()
        .await
        .get(swarm_id)
        .cloned()
        .unwrap_or_else(|| req_session_id.to_string());
    let delivered = fanout_session_event(
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
    if delivered == 0 {
        // The pause is already applied; this alert is how the coordinator finds
        // out. Zero recipients means the plan is stopped and nobody has been
        // told, which looks exactly like a plan that simply went quiet. The bus
        // event below is the remaining path, so record the gap instead of
        // treating an undelivered alert as a successful one.
        eprintln!(
            "swarm budget: task graph paused for swarm {swarm_id}, but the alert reached no \
             live receiver for coordinator {coordinator_id}"
        );
    }
    crate::bus::Bus::global().publish(crate::bus::BusEvent::SwarmAwaitCompleted(
        crate::bus::SwarmAwaitCompleted {
            session_id: coordinator_id,
            completed: false,
            summary: "Task graph paused after a hard budget was exceeded".to_string(),
            notification: message.clone(),
            notify: false,
            wake: true,
        },
    ));
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

    // Infer deep mode from effort when omitted; an explicit mode still wins.
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

        // Replace the workflow without leaking old graph state; keep version monotonic.
        let mut replacement = VersionedPlan::new();
        replacement.version = plan.version;
        replacement.max_nodes = Some(crate::config::config().agents.swarm_max_graph_nodes);
        if let Some(mode) = resolved_mode {
            replacement.mode = mode;
        }
        replacement.participants = replacement_participants;
        let ledger = initialize_safety_ledger(&mut replacement, unix_now_ms());
        seed_replacement_with_budget(plan, replacement, specs, ledger)
    };

    match result {
        GraphMutationResult::Applied(_) => {
            // Elect only after success so rejected seeds cannot mutate roles.
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
        GraphMutationResult::Rejected(message) => {
            err(client_event_tx, id, format!("Seed rejected: {message}"));
        }
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
            claim_queued_node_for_actor(graph, &node_id, &req_session_id);
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
    // Capture evidence before the engine consumes the artifact.
    let artifact_confidence = artifact.confidence.clone();

    // A coordinator may salvage a node whose owner is no longer live.
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
            // The node is terminal: every session's authority derived from it
            // ends now, including a headed assignee that never runs through
            // the server-side turn loop and its run-end cleanup.
            crate::tool::grant::clear_assignment_grant(&swarm_id, &node_id);
            // File evidence before finalize publishes the derived done state.
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
            claim_queued_node_for_actor(graph, &gate_id, &req_session_id);
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

/// Freeze or unfreeze graph growth at the coordinator-controlled server boundary.
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
    use crate::protocol::SwarmMemberRuntime;
    use jcode_plan::bridge::apply_task_graph;
    use jcode_plan::dag::{Mode, NodeKind, NodeOrigin, TaskNode};
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicU64;
    use std::time::Instant;

    fn member(session_id: &str, swarm_id: &str, role: &str) -> SwarmMember {
        let (event_tx, _) = mpsc::unbounded_channel();
        SwarmMember {
            session_id: session_id.to_string(),
            event_tx,
            event_txs: HashMap::new(),
            working_dir: None,
            swarm_id: Some(swarm_id.to_string()),
            swarm_enabled: true,
            status: "ready".to_string(),
            lifecycle: Default::default(),
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
        }
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

    async fn budget_wake_for(
        events: &mut broadcast::Receiver<crate::bus::BusEvent>,
        target: &str,
    ) -> bool {
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if let Ok(crate::bus::BusEvent::SwarmAwaitCompleted(event)) = events.recv().await
                    && event.session_id == target
                    && event.wake
                    && event.notification.contains("budget")
                {
                    return true;
                }
            }
        })
        .await
        .unwrap_or(false)
    }

    #[test]
    fn direct_plan_update_is_refused_while_paused() {
        let mut plan = VersionedPlan::new();
        let mut ledger = PlanSafetyLedger::started(configured_policy(&plan), 100);
        ledger.status = PlanSafetyStatus::PausedBudgetExceeded;
        ledger.pause = Some(BudgetViolation {
            budget: GraphBudget::Nodes,
            limit: 1,
            observed: 2,
            operation: "seeding".to_string(),
        });
        store_safety_ledger(&mut plan, &ledger);

        let refusal = direct_plan_update_refusal(&mut plan, 1, 200);

        assert!(
            refusal.is_some_and(|message| message.contains("node count")),
            "a paused plan must refuse a direct item replacement"
        );
    }

    #[test]
    fn direct_plan_update_is_refused_while_frozen() {
        let mut plan = VersionedPlan::new();
        ensure_safety_ledger(&mut plan, 100);
        plan.frozen = true;

        let refusal = direct_plan_update_refusal(&mut plan, 1, 200);

        assert!(
            refusal.is_some_and(|message| message.contains("frozen")),
            "a frozen plan must refuse a direct item replacement"
        );
    }

    #[test]
    fn direct_plan_update_respects_the_node_budget() {
        let mut plan = VersionedPlan::new();
        plan.max_nodes = Some(2);
        ensure_safety_ledger(&mut plan, 100);

        assert!(
            direct_plan_update_refusal(&mut plan, 2, 200).is_none(),
            "a plan at its budget is still proposable"
        );
        assert!(
            direct_plan_update_refusal(&mut plan, 3, 200)
                .is_some_and(|message| message.contains("node budget is 2")),
            "an over-budget item list must be refused"
        );
    }

    #[test]
    fn direct_plan_update_mints_a_ledger_for_a_budgetless_plan() {
        let mut plan = VersionedPlan::new();
        assert!(plan.safety_ledger.is_none());

        assert!(direct_plan_update_refusal(&mut plan, 1, 100).is_none());

        assert!(
            plan.safety_ledger.is_some(),
            "a plan proposed directly still needs a budget clock for run_plan"
        );
    }

    #[test]
    fn seed_budget_overrun_pauses_without_replacing_the_graph() {
        let mut plan = VersionedPlan::new();
        let mut replacement = VersionedPlan::new();
        replacement.max_nodes = Some(1);
        let ledger = PlanSafetyLedger::started(configured_policy(&replacement), 100);
        let specs = vec![
            NodeSpec::new("one", "one", NodeKind::Explore),
            NodeSpec::new("two", "two", NodeKind::Explore),
        ];
        let result = seed_replacement_with_budget(&mut plan, replacement, specs, ledger);
        assert!(matches!(result, GraphMutationResult::Paused(_)));
        assert!(plan.frozen);
        assert!(plan.items.is_empty(), "rejected seed must not land");
        assert_eq!(
            load_safety_ledger(&plan, 999).status,
            PlanSafetyStatus::PausedBudgetExceeded
        );
        let status =
            crate::protocol::PlanGraphStatus::from_versioned_plan("seed", &plan, None, vec![]);
        assert!(status.phases_by_id[dag::PLAN_SAFETY_STATUS_META_ID].starts_with("100:"));
    }

    #[tokio::test]
    async fn node_budget_overrun_pauses_plan_and_wakes_coordinator_without_growth() {
        let swarm_id = "swarm-budget".to_string();
        let coordinator_id = "coord-budget".to_string();
        let worker_id = "worker-budget".to_string();
        let coordinator = member(&coordinator_id, &swarm_id, "coordinator");
        let worker = member(&worker_id, &swarm_id, "agent");
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
        let (client_tx, _) = mpsc::unbounded_channel();
        let mut bus_events = crate::bus::Bus::global().subscribe();

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
        assert!(budget_wake_for(&mut bus_events, &coordinator_id).await);
    }

    #[test]
    fn force_completion_blocks_sibling_gate_replacement_injection() {
        let mut graph = TaskGraph::new(Mode::Light);
        graph.push_node(TaskNode {
            id: "sibling-gate".to_string(),
            content: "inject replacements".to_string(),
            kind: NodeKind::Critique,
            status: NodeStatus::Running,
            owner: Some("gate-worker".to_string()),
            parent: None,
            depends_on: Vec::new(),
            expanded: false,
            is_gate: true,
            planner: None,
            priority: 0,
            output: None,
            origin: Some(NodeOrigin::Gate),
        });
        let mut plan = VersionedPlan::new();
        apply_task_graph(&mut plan, &graph);
        force_completion_closes_graph_growth(&mut plan);
        let replacement = NodeSpec::new("replacement", "replacement", NodeKind::Explore);
        let result = mutate_plan_with_budget(&mut plan, unix_now_ms(), |graph| {
            dag::inject_from_gate(graph, "sibling-gate", "gate-worker", vec![replacement])
        });
        assert!(matches!(
            result,
            GraphMutationResult::Rejected(message) if message.contains("frozen")
        ));
        assert_eq!(
            plan.items.len(),
            1,
            "force completion must block replacement"
        );
        assert!(!plan.items.iter().any(|item| item.id == "replacement"));
    }
}
