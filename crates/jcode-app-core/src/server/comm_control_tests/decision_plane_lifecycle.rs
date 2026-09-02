use super::member_is_assignment_eligible;
use crate::server::client_comm_context::comm_context_member_status;
use crate::server::comm_await::{awaited_member_statuses, pending_member_label};
use crate::server::comm_session::swarm_member_is_stale_for_coordination;
use crate::server::control_log_sync::target_member_view;
use jcode_swarm_core::{MemberLifecycleState, SwarmLifecycleStatus};

fn poison_mirror(member: &mut SwarmMember, canonical: MemberLifecycleState, mirror: &str) {
    member.status = mirror.to_string();
    member.lifecycle = SwarmLifecycleStatus {
        state: canonical,
        assignment_epoch: 1,
        revision: 1,
        reason: None,
        updated_at_unix_ms: 1,
    };
}

#[tokio::test]
async fn poisoned_mirror_cannot_split_decision_plane_surfaces() {
    let swarm_id = "decision-plane";
    let worker_id = "worker";
    let mut worker = member(worker_id, swarm_id, "ready");
    worker.is_headless = true;
    poison_mirror(&mut worker, MemberLifecycleState::Failed, "ready");

    assert!(swarm_member_is_stale_for_coordination(&worker));
    assert!(!member_is_assignment_eligible(&worker));
    let candidates = HashMap::from([
        (worker_id.to_string(), worker.clone()),
        ("coord".to_string(), {
            let mut coordinator = member("coord", swarm_id, "ready");
            coordinator.role = "coordinator".to_string();
            coordinator
        }),
    ]);
    assert!(
        filter_swarm_agent_candidates(&candidates, "coord", swarm_id).is_empty(),
        "canonically terminal worker must not be assignment-eligible"
    );
    assert_eq!(comm_context_member_status(&worker), "failed");
    assert_eq!(target_member_view(&[worker.clone()])[worker_id].status, "failed");

    let swarm_members = Arc::new(RwLock::new(HashMap::from([(
        worker_id.to_string(),
        worker,
    )])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        swarm_id.to_string(),
        HashSet::from([worker_id.to_string()]),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::new()));
    let statuses = awaited_member_statuses(
        "coord",
        swarm_id,
        &[worker_id.to_string()],
        &["failed".to_string()],
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
    )
    .await;

    assert_eq!(statuses[0].status, "failed");
    assert!(statuses[0].done);
    let label = pending_member_label(&statuses[0]);
    assert!(label.contains("failed"), "await label was {label}");
    assert!(!label.contains("ready"), "stale mirror leaked into {label}");
}

#[test]
fn assignment_eligibility_preserves_ready_and_succeeded_semantics() {
    for state in [MemberLifecycleState::Ready, MemberLifecycleState::Succeeded] {
        let mut worker = member("worker", "swarm", "failed");
        poison_mirror(&mut worker, state, "failed");
        assert!(member_is_assignment_eligible(&worker), "state {state:?}");
    }

    for state in [
        MemberLifecycleState::Starting,
        MemberLifecycleState::Assigned,
        MemberLifecycleState::Running,
        MemberLifecycleState::Failed,
        MemberLifecycleState::Stopped,
        MemberLifecycleState::Lost,
    ] {
        let mut worker = member("worker", "swarm", "ready");
        poison_mirror(&mut worker, state, "ready");
        assert!(!member_is_assignment_eligible(&worker), "state {state:?}");
    }
}
