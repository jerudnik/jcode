use super::swarm_mutation_state::{
    PersistedSwarmMutationResponse, SwarmMutationRuntime, begin_or_replay, finish_request,
    request_key,
};
use super::{
    PendingPlanProposal, PlanProposalCache, SessionInterruptQueues, SwarmEventState,
    SwarmEventType, SwarmMember, SwarmState, VersionedPlan, broadcast_swarm_plan,
    persist_swarm_state_for, queue_soft_interrupt_for_session, record_swarm_event,
    summarize_plan_items,
};
use crate::agent::Agent;
use crate::plan::PlanItem;
use crate::protocol::{NotificationType, ServerEvent};
use jcode_agent_runtime::SoftInterruptSource;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock, mpsc};

type SessionAgents = Arc<RwLock<HashMap<String, Arc<Mutex<Agent>>>>>;

/// Reject plans whose dependency graph contains a cycle. Cyclic items can never
/// become runnable (`summarize_plan_graph` parks them in `blocked_ids` forever),
/// so letting one into a live plan silently wedges all dependent work. This
/// mirrors the acyclicity validation `jcode_plan::dag::seed`/`expand` already
/// enforce on the task-graph paths. Returns a user-facing error naming the
/// cyclic item ids, or `None` when the graph is a valid DAG.
fn plan_cycle_error(items: &[PlanItem]) -> Option<String> {
    let cycle_ids = crate::plan::cycle_item_ids(items);
    if cycle_ids.is_empty() {
        return None;
    }
    Some(format!(
        "Plan rejected: dependency cycle among item(s): {}. Fix the blocked_by \
         edges so the plan forms a DAG, then propose again.",
        cycle_ids.join(", ")
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "plan proposal updates sessions, swarm coordination, proposal cache, interrupts, and event history"
)]
pub(super) async fn handle_comm_propose_plan(
    id: u64,
    req_session_id: String,
    items: Vec<PlanItem>,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_state: &SwarmState,
    plan_proposals: &PlanProposalCache,
    sessions: &SessionAgents,
    soft_interrupt_queues: &SessionInterruptQueues,
    swarm_events: &SwarmEventState,
    _swarm_mutation_runtime: &SwarmMutationRuntime,
) {
    let swarm_members = &swarm_state.members;
    let swarms_by_id = &swarm_state.swarms_by_id;
    let swarm_plans = &swarm_state.plans;
    let swarm_coordinators = &swarm_state.coordinators;
    let swarm_id = {
        let members = swarm_members.read().await;
        members
            .get(&req_session_id)
            .and_then(|member| member.swarm_id.clone())
    };

    let swarm_id = match swarm_id.as_ref() {
        Some(swarm_id) => swarm_id.clone(),
        None => {
            let _ = client_event_tx.send(ServerEvent::Error {
                id,
                message: "Not in a swarm.".to_string(),
                retry_after_secs: None,
            });
            return;
        }
    };

    let (from_name, coordinator_id) = {
        let members = swarm_members.read().await;
        let from_name = members
            .get(&req_session_id)
            .and_then(|member| member.friendly_name.clone());
        let coordinators = swarm_coordinators.read().await;
        let coordinator_id = coordinators.get(&swarm_id).cloned();
        (from_name, coordinator_id)
    };
    let from_label = from_name
        .clone()
        .unwrap_or_else(|| req_session_id.chars().take(8).collect());

    let Some(coordinator_id) = coordinator_id else {
        let _ = client_event_tx.send(ServerEvent::Error {
            id,
            message: "No coordinator for this swarm.".to_string(),
            retry_after_secs: None,
        });
        return;
    };

    if coordinator_id == req_session_id {
        // Close the last entry path for dependency cycles into live plans: the
        // dag seed/expand ops validate acyclicity, but this direct-update path
        // used to write `plan.items` verbatim. A cyclic task can never become
        // runnable (it lands in blocked_ids forever), silently wedging all
        // dependent work, so reject for light plans too, matching `dag::seed`
        // which rejects cycles in both modes.
        if let Some(message) = plan_cycle_error(&items) {
            let _ = client_event_tx.send(ServerEvent::Error {
                id,
                message,
                retry_after_secs: None,
            });
            return;
        }
        let (version, participant_ids) = {
            let mut plans = swarm_plans.write().await;
            let plan = plans
                .entry(swarm_id.clone())
                .or_insert_with(VersionedPlan::new);
            plan.participants.insert(req_session_id.clone());
            for item in &items {
                if let Some(owner) = &item.assigned_to {
                    plan.participants.insert(owner.clone());
                }
            }
            plan.items = items.clone();
            plan.version += 1;
            (plan.version, plan.participants.clone())
        };

        let members = swarm_members.read().await;
        let notification_msg = format!(
            "Plan updated by {} ({} items, v{})",
            from_label,
            items.len(),
            version
        );
        for sid in participant_ids {
            if sid == req_session_id {
                continue;
            }
            if let Some(member) = members.get(&sid) {
                let _ = member.event_tx.send(ServerEvent::Notification {
                    from_session: req_session_id.clone(),
                    from_name: from_name.clone(),
                    notification_type: NotificationType::Message {
                        scope: Some("plan".to_string()),
                        tldr: None,
                    },
                    message: notification_msg.clone(),
                });
            }
            let _ = queue_soft_interrupt_for_session(
                &sid,
                notification_msg.clone(),
                false,
                SoftInterruptSource::System,
                soft_interrupt_queues,
                sessions,
            )
            .await;
        }

        persist_swarm_state_for(&swarm_id, swarm_state).await;

        broadcast_swarm_plan(
            &swarm_id,
            Some("coordinator_direct_update".to_string()),
            swarm_plans,
            swarm_members,
            swarms_by_id,
        )
        .await;
        record_swarm_event(
            &swarm_events.history,
            &swarm_events.counter,
            &swarm_events.tx,
            req_session_id.clone(),
            from_name.clone(),
            Some(swarm_id.clone()),
            SwarmEventType::PlanUpdate {
                swarm_id: swarm_id.clone(),
                item_count: items.len(),
            },
        )
        .await;

        let _ = client_event_tx.send(ServerEvent::Done { id });
        return;
    }

    // Non-coordinator proposal path. A cycle among the proposed items alone is
    // a cycle in any merged graph, so reject it here for immediate proposer
    // feedback instead of storing a proposal that can only bounce at approval.
    // (Cycles that only form against the *existing* plan are caught at
    // approve-time, where the merge is authoritative.)
    if let Some(message) = plan_cycle_error(&items) {
        let _ = client_event_tx.send(ServerEvent::Error {
            id,
            message,
            retry_after_secs: None,
        });
        return;
    }

    {
        let mut cache = plan_proposals.write().await;
        cache.entry(swarm_id.clone()).or_default().insert(
            req_session_id.clone(),
            PendingPlanProposal {
                items: items.clone(),
                created_at: Instant::now(),
            },
        );
    }
    record_swarm_event(
        &swarm_events.history,
        &swarm_events.counter,
        &swarm_events.tx,
        req_session_id.clone(),
        from_name.clone(),
        Some(swarm_id.clone()),
        SwarmEventType::PlanProposal {
            swarm_id: swarm_id.clone(),
            proposer_session: req_session_id.clone(),
            item_count: items.len(),
        },
    )
    .await;

    let summary = summarize_plan_items(&items, 3);
    let notification_msg = format!(
        "Plan proposal from {} ({} items). Summary: {}. Review with communicate approve_plan/reject_plan.",
        from_label,
        items.len(),
        summary
    );

    let members = swarm_members.read().await;
    if let Some(member) = members.get(&coordinator_id) {
        let _ = member.event_tx.send(ServerEvent::Notification {
            from_session: req_session_id.clone(),
            from_name: from_name.clone(),
            notification_type: NotificationType::Message {
                scope: Some("plan_proposal".to_string()),
                tldr: None,
            },
            message: notification_msg.clone(),
        });
        let _ = member.event_tx.send(ServerEvent::SwarmPlanProposal {
            swarm_id: swarm_id.clone(),
            proposer_session: req_session_id.clone(),
            proposer_name: from_name.clone(),
            items: items.clone(),
            summary: summary.clone(),
            proposal_key: req_session_id.clone(),
        });
    }
    let _ = queue_soft_interrupt_for_session(
        &coordinator_id,
        notification_msg.clone(),
        false,
        SoftInterruptSource::System,
        soft_interrupt_queues,
        sessions,
    )
    .await;

    let proposer_confirmation = "Plan proposal sent to coordinator (not yet applied).".to_string();
    if let Some(member) = members.get(&req_session_id) {
        let _ = member.event_tx.send(ServerEvent::Notification {
            from_session: req_session_id.clone(),
            from_name: from_name.clone(),
            notification_type: NotificationType::Message {
                scope: Some("plan_proposal".to_string()),
                tldr: None,
            },
            message: proposer_confirmation.clone(),
        });
    }
    let _ = queue_soft_interrupt_for_session(
        &req_session_id,
        proposer_confirmation,
        false,
        SoftInterruptSource::System,
        soft_interrupt_queues,
        sessions,
    )
    .await;

    let _ = client_event_tx.send(ServerEvent::Done { id });
}

#[expect(
    clippy::too_many_arguments,
    reason = "plan approval updates sessions, swarm coordination, interrupts, and event history"
)]
pub(super) async fn handle_comm_approve_plan(
    id: u64,
    req_session_id: String,
    proposer_session: String,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_state: &SwarmState,
    plan_proposals: &PlanProposalCache,
    sessions: &SessionAgents,
    soft_interrupt_queues: &SessionInterruptQueues,
    swarm_events: &SwarmEventState,
    swarm_mutation_runtime: &SwarmMutationRuntime,
) {
    let swarm_members = &swarm_state.members;
    let swarms_by_id = &swarm_state.swarms_by_id;
    let swarm_plans = &swarm_state.plans;
    let swarm_coordinators = &swarm_state.coordinators;
    let swarm_id = match require_coordinator_swarm(
        id,
        &req_session_id,
        "Only the coordinator can approve plan proposals.",
        client_event_tx,
        swarm_members,
        swarm_coordinators,
    )
    .await
    {
        Some(swarm_id) => swarm_id,
        None => return,
    };

    let mutation_key = request_key(
        &req_session_id,
        "approve_plan",
        &[swarm_id.clone(), proposer_session.clone()],
    );
    let Some(mutation_state) = begin_or_replay(
        swarm_mutation_runtime,
        &mutation_key,
        "approve_plan",
        &req_session_id,
        id,
        client_event_tx,
    )
    .await
    else {
        return;
    };

    let proposal_items = {
        let cache = plan_proposals.read().await;
        cache
            .get(&swarm_id)
            .and_then(|proposals| proposals.get(&proposer_session))
            .map(|proposal| proposal.items.clone())
    };

    let items = match proposal_items {
        Some(items) => items,
        None => {
            finish_request(
                swarm_mutation_runtime,
                &mutation_state,
                PersistedSwarmMutationResponse::Error {
                    message: format!("No pending plan proposal from session '{proposer_session}'"),
                    retry_after_secs: None,
                },
            )
            .await;
            return;
        }
    };

    // Validate the merged graph (existing plan + proposed items) for
    // dependency cycles before committing. A cycle here permanently wedges
    // every task that depends on it, so reject the approval and keep the
    // proposal pending for the proposer to fix and re-propose.
    let merged_cycle_error = {
        let plans = swarm_plans.read().await;
        let mut merged: Vec<PlanItem> = plans
            .get(&swarm_id)
            .map(|plan| plan.items.clone())
            .unwrap_or_default();
        merged.extend(items.iter().cloned());
        plan_cycle_error(&merged)
    };
    if let Some(message) = merged_cycle_error {
        finish_request(
            swarm_mutation_runtime,
            &mutation_state,
            PersistedSwarmMutationResponse::Error {
                message,
                retry_after_secs: None,
            },
        )
        .await;
        return;
    }

    let participant_ids = {
        let mut plans = swarm_plans.write().await;
        let plan = plans
            .entry(swarm_id.clone())
            .or_insert_with(VersionedPlan::new);
        plan.items.extend(items.clone());
        plan.version += 1;
        plan.participants.insert(req_session_id.clone());
        plan.participants.insert(proposer_session.clone());
        for item in &items {
            if let Some(owner) = &item.assigned_to {
                plan.participants.insert(owner.clone());
            }
        }
        plan.participants.clone()
    };

    {
        let mut cache = plan_proposals.write().await;
        if let Some(proposals) = cache.get_mut(&swarm_id) {
            proposals.remove(&proposer_session);
        }
    }

    broadcast_swarm_plan(
        &swarm_id,
        Some("proposal_approved".to_string()),
        swarm_plans,
        swarm_members,
        swarms_by_id,
    )
    .await;
    record_swarm_event(
        &swarm_events.history,
        &swarm_events.counter,
        &swarm_events.tx,
        req_session_id.clone(),
        None,
        Some(swarm_id.clone()),
        SwarmEventType::PlanUpdate {
            swarm_id: swarm_id.clone(),
            item_count: items.len(),
        },
    )
    .await;

    let coordinator_name = {
        let members = swarm_members.read().await;
        members
            .get(&req_session_id)
            .and_then(|member| member.friendly_name.clone())
    };

    let members = swarm_members.read().await;
    for sid in participant_ids {
        if let Some(member) = members.get(&sid) {
            let message = format!(
                "Plan approved by coordinator: {} items added from {}",
                items.len(),
                proposer_session
            );
            let _ = member.event_tx.send(ServerEvent::Notification {
                from_session: req_session_id.clone(),
                from_name: coordinator_name.clone(),
                notification_type: NotificationType::Message {
                    scope: Some("plan".to_string()),
                    tldr: None,
                },
                message: message.clone(),
            });

            let _ = queue_soft_interrupt_for_session(
                &sid,
                message.clone(),
                false,
                SoftInterruptSource::System,
                soft_interrupt_queues,
                sessions,
            )
            .await;
        }
    }

    persist_swarm_state_for(&swarm_id, swarm_state).await;

    finish_request(
        swarm_mutation_runtime,
        &mutation_state,
        PersistedSwarmMutationResponse::Done,
    )
    .await;
}

#[expect(
    clippy::too_many_arguments,
    reason = "plan rejection updates sessions, swarm coordination, interrupts, and event history"
)]
pub(super) async fn handle_comm_reject_plan(
    id: u64,
    req_session_id: String,
    proposer_session: String,
    reason: Option<String>,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_state: &SwarmState,
    plan_proposals: &PlanProposalCache,
    sessions: &SessionAgents,
    soft_interrupt_queues: &SessionInterruptQueues,
    swarm_events: &SwarmEventState,
    swarm_mutation_runtime: &SwarmMutationRuntime,
) {
    let swarm_members = &swarm_state.members;
    let swarm_coordinators = &swarm_state.coordinators;
    let swarm_id = match require_coordinator_swarm(
        id,
        &req_session_id,
        "Only the coordinator can reject plan proposals.",
        client_event_tx,
        swarm_members,
        swarm_coordinators,
    )
    .await
    {
        Some(swarm_id) => swarm_id,
        None => return,
    };

    let mutation_key = request_key(
        &req_session_id,
        "reject_plan",
        &[
            swarm_id.clone(),
            proposer_session.clone(),
            reason.clone().unwrap_or_default(),
        ],
    );
    let Some(mutation_state) = begin_or_replay(
        swarm_mutation_runtime,
        &mutation_key,
        "reject_plan",
        &req_session_id,
        id,
        client_event_tx,
    )
    .await
    else {
        return;
    };

    let proposal_exists = {
        let cache = plan_proposals.read().await;
        cache
            .get(&swarm_id)
            .and_then(|proposals| proposals.get(&proposer_session))
            .is_some()
    };

    if !proposal_exists {
        finish_request(
            swarm_mutation_runtime,
            &mutation_state,
            PersistedSwarmMutationResponse::Error {
                message: format!("No pending plan proposal from session '{proposer_session}'"),
                retry_after_secs: None,
            },
        )
        .await;
        return;
    }

    {
        let mut cache = plan_proposals.write().await;
        if let Some(proposals) = cache.get_mut(&swarm_id) {
            proposals.remove(&proposer_session);
        }
    }

    let coordinator_name = {
        let members = swarm_members.read().await;
        members
            .get(&req_session_id)
            .and_then(|member| member.friendly_name.clone())
    };

    let members = swarm_members.read().await;
    if let Some(member) = members.get(&proposer_session) {
        let reason_msg = reason
            .as_ref()
            .map(|reason| format!(": {reason}"))
            .unwrap_or_default();
        let message = format!("Your plan proposal was rejected by the coordinator{reason_msg}");
        let _ = member.event_tx.send(ServerEvent::Notification {
            from_session: req_session_id.clone(),
            from_name: coordinator_name.clone(),
            notification_type: NotificationType::Message {
                scope: Some("dm".to_string()),
                tldr: None,
            },
            message: message.clone(),
        });

        let _ = queue_soft_interrupt_for_session(
            &proposer_session,
            message,
            false,
            SoftInterruptSource::System,
            soft_interrupt_queues,
            sessions,
        )
        .await;
    }
    record_swarm_event(
        &swarm_events.history,
        &swarm_events.counter,
        &swarm_events.tx,
        req_session_id.clone(),
        coordinator_name,
        Some(swarm_id.clone()),
        SwarmEventType::Notification {
            notification_type: "plan_rejected".to_string(),
            message: proposer_session.clone(),
        },
    )
    .await;

    finish_request(
        swarm_mutation_runtime,
        &mutation_state,
        PersistedSwarmMutationResponse::Done,
    )
    .await;
}

#[cfg(test)]
#[path = "comm_plan_tests.rs"]
mod tests;

async fn require_coordinator_swarm(
    id: u64,
    req_session_id: &str,
    permission_error: &str,
    client_event_tx: &mpsc::UnboundedSender<ServerEvent>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
) -> Option<String> {
    let (swarm_id, is_coordinator) = {
        let members = swarm_members.read().await;
        let swarm_id = members
            .get(req_session_id)
            .and_then(|member| member.swarm_id.clone());
        let is_coordinator = if let Some(ref swarm_id) = swarm_id {
            let coordinators = swarm_coordinators.read().await;
            coordinators
                .get(swarm_id)
                .map(|coordinator| coordinator == req_session_id)
                .unwrap_or(false)
        } else {
            false
        };
        (swarm_id, is_coordinator)
    };

    if !is_coordinator {
        let _ = client_event_tx.send(ServerEvent::Error {
            id,
            message: permission_error.to_string(),
            retry_after_secs: None,
        });
        return None;
    }

    match swarm_id {
        Some(swarm_id) => Some(swarm_id),
        None => {
            let _ = client_event_tx.send(ServerEvent::Error {
                id,
                message: "Not in a swarm.".to_string(),
                retry_after_secs: None,
            });
            None
        }
    }
}
