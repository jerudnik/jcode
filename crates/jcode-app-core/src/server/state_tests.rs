//! Regression tests for `SwarmState::assignment_grant_for_session`.
//!
//! These pin the classification order documented on the function: human,
//! coordinator, bootstrap, and ambiguous identities fail open to
//! `Unrestricted`; a live plan assignment binds for its lifetime; only a
//! spawned-worker identity without a live assignment is `Unassigned`.

use super::{SwarmMember, SwarmState};
use crate::tool::grant::GrantLookup;
use jcode_plan::{AssignmentGrant, PlanItem, SwarmTaskProgress, VersionedPlan};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tokio::sync::mpsc;

const SESSION: &str = "session-a";
const SWARM: &str = "swarm-a";

#[allow(deprecated)]
fn member(session_id: &str, swarm_id: Option<&str>) -> SwarmMember {
    let (event_tx, _rx) = mpsc::unbounded_channel();
    let now = Instant::now();
    SwarmMember {
        session_id: session_id.to_string(),
        event_tx,
        event_txs: HashMap::new(),
        working_dir: None,
        swarm_id: swarm_id.map(str::to_string),
        swarm_enabled: true,
        status: "ready".to_string(),
        lifecycle: jcode_swarm_core::SwarmLifecycleStatus::Ready,
        detail: None,
        task_label: None,
        subagent_type: None,
        friendly_name: None,
        report_back_to_session_id: None,
        initial_prompt_delivered: None,
        latest_completion_report: None,
        role: "agent".to_string(),
        joined_at: now,
        last_status_change: now,
        is_headless: false,
        output_tail: None,
        todo_progress: None,
        todo_items: Vec::new(),
        runtime: crate::protocol::SwarmMemberRuntime::default(),
    }
}

/// A plan with a single non-terminal item assigned to `session_id`. When
/// `with_grant` is set, the assignment progress carries a grant and epoch (a
/// live assignment); otherwise it carries only the assignee (a reclaimed one).
fn plan_with_assignment(session_id: &str, with_grant: bool) -> VersionedPlan {
    let mut plan = VersionedPlan::new();
    plan.items.push(PlanItem {
        content: "task".to_string(),
        status: "running".to_string(),
        priority: "normal".to_string(),
        id: "task-a".to_string(),
        subsystem: None,
        file_scope: Vec::new(),
        blocked_by: Vec::new(),
        assigned_to: Some(session_id.to_string()),
    });
    plan.task_progress.insert(
        "task-a".to_string(),
        SwarmTaskProgress {
            assigned_session_id: Some(session_id.to_string()),
            assignment_grant: with_grant.then_some(AssignmentGrant::ReadOnly),
            assignment_epoch: with_grant.then_some(7),
            ..Default::default()
        },
    );
    plan
}

fn state_with(
    member: SwarmMember,
    plan: Option<VersionedPlan>,
    coordinator: Option<&str>,
) -> SwarmState {
    let swarm_id = member.swarm_id.clone();
    let mut swarms_by_id = HashMap::new();
    let mut plans = HashMap::new();
    let mut coordinators = HashMap::new();
    if let Some(swarm_id) = swarm_id {
        swarms_by_id.insert(swarm_id.clone(), HashSet::from([member.session_id.clone()]));
        if let Some(plan) = plan {
            plans.insert(swarm_id.clone(), plan);
        }
        if let Some(coordinator) = coordinator {
            coordinators.insert(swarm_id, coordinator.to_string());
        }
    }
    SwarmState::new(
        HashMap::from([(member.session_id.clone(), member)]),
        swarms_by_id,
        plans,
        coordinators,
    )
}

#[tokio::test]
async fn assignment_grant_plain_client_member_is_unrestricted() {
    // A plain interactive client registers as role "agent" with a swarm id
    // but no spawn-path fields; with a plan that assigns it nothing it must
    // not be treated as an unassigned worker.
    let state = state_with(
        member(SESSION, Some(SWARM)),
        Some(VersionedPlan::new()),
        None,
    );
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Unrestricted
    );
}

#[tokio::test]
async fn assignment_grant_plain_client_no_plan_is_unrestricted() {
    // Same identity, but the swarm has no plan entry at all.
    let state = state_with(member(SESSION, Some(SWARM)), None, None);
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Unrestricted
    );
}

#[tokio::test]
async fn assignment_grant_coordinator_is_unrestricted() {
    // Coordinator status dominates and must fail open even when a live
    // assignment is still recorded (e.g. after re-election).
    let state = state_with(
        member(SESSION, Some(SWARM)),
        Some(plan_with_assignment(SESSION, true)),
        Some(SESSION),
    );
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Unrestricted
    );
}

#[tokio::test]
async fn assignment_grant_recovered_headless_join_is_unrestricted() {
    // Control-log replay can recreate a joined session as headless with no
    // owner metadata; is_headless alone is not worker identity.
    let mut member = member(SESSION, Some(SWARM));
    member.is_headless = true;
    let state = state_with(member, Some(VersionedPlan::new()), None);
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Unrestricted
    );
}

#[tokio::test]
async fn assignment_grant_swarm_disabled_member_is_unrestricted() {
    // A member with swarm coordination disabled but a stale swarm id must
    // not be restricted.
    let mut member = member(SESSION, Some(SWARM));
    member.swarm_enabled = false;
    let state = state_with(member, Some(VersionedPlan::new()), None);
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Unrestricted
    );
}

#[tokio::test]
async fn assignment_grant_assigned_client_member_still_gets_grant() {
    // Guard: a client-attached session explicitly assigned a plan node stays
    // bound to that grant for the life of the assignment.
    let state = state_with(
        member(SESSION, Some(SWARM)),
        Some(plan_with_assignment(SESSION, true)),
        None,
    );
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Assigned {
            grant: AssignmentGrant::ReadOnly,
            swarm_id: SWARM.to_string(),
            task_id: "task-a".to_string(),
            epoch: 7,
        }
    );
}

#[tokio::test]
async fn assignment_grant_spawned_worker_without_assignment_is_unassigned() {
    // Guard: a real spawned worker holding no live assignment keeps the
    // unassigned-worker restriction.
    let mut member = member(SESSION, Some(SWARM));
    member.report_back_to_session_id = Some("owner".to_string());
    let state = state_with(member, Some(VersionedPlan::new()), None);
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Unassigned { reclaimed: false }
    );
}

#[tokio::test]
async fn assignment_grant_spawned_worker_no_plan_is_unassigned() {
    // A spawned worker whose swarm has no plan entry at all is still an
    // unassigned worker; only the plain-client no-plan case fails open.
    let mut member = member(SESSION, Some(SWARM));
    member.report_back_to_session_id = Some("owner".to_string());
    let state = state_with(member, None, None);
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Unassigned { reclaimed: false }
    );
}

#[tokio::test]
async fn assignment_grant_reclaimed_spawned_worker_reports_reclaimed() {
    // Guard: a spawned worker whose prior assignment record lost its grant
    // reports reclaimed so the denial message can name salvage.
    let mut member = member(SESSION, Some(SWARM));
    member.initial_prompt_delivered = Some(false);
    let state = state_with(member, Some(plan_with_assignment(SESSION, false)), None);
    assert_eq!(
        state.assignment_grant_for_session(SESSION).await,
        GrantLookup::Unassigned { reclaimed: true }
    );
}

#[test]
fn assignment_grant_spawn_identity_predicate() {
    let base = member(SESSION, Some(SWARM));
    assert!(!base.has_spawn_worker_identity());

    let mut by_owner = base.clone();
    by_owner.report_back_to_session_id = Some("owner".to_string());
    assert!(by_owner.has_spawn_worker_identity());

    let mut by_prompt = base.clone();
    by_prompt.initial_prompt_delivered = Some(false);
    assert!(by_prompt.has_spawn_worker_identity());

    let mut by_type = base.clone();
    by_type.subagent_type = Some("explore".to_string());
    assert!(by_type.has_spawn_worker_identity());

    // Fields set on non-spawn paths must not confer worker identity.
    let mut by_label = base.clone();
    by_label.task_label = Some("assigned task".to_string());
    assert!(!by_label.has_spawn_worker_identity());
    let mut by_report = base.clone();
    by_report.latest_completion_report = Some("done".to_string());
    assert!(!by_report.has_spawn_worker_identity());
    let mut by_headless = base.clone();
    by_headless.is_headless = true;
    assert!(!by_headless.has_spawn_worker_identity());
}
