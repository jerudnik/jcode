//! Shared swarm control-verb intelligence for operator clients.
//!
//! The wire protocol exposes several overlapping drive verbs
//! (`comm_assign_task`, `comm_task_control` with start/retry/...), and picking
//! the right one for a runnable-but-stuck plan node requires knowing the
//! node's assignment and lifecycle state. Operators should not have to guess:
//! the 2026-07-07 run_plan stall took three human attempts
//! (assign_task -> retry -> start_task) to resolve.
//!
//! This module holds that decision in one place so every client (TUI slash
//! commands today, web cockpit later) resolves a stuck instance with one
//! action. It also implements the three-tier member type resolution decided
//! for the swarm control plane (phase of assigned instance, then Agent-tool
//! preset, then free-form swarm tag, then untyped).
//!
//! Vocabulary: a plan node is an "instance"; its `kind` displays as "phase".

use crate::plan::PlanItem;

/// The single verb a client should send to move one plan instance forward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JumpstartVerb {
    /// The instance is unassigned and runnable: send `comm_assign_task`.
    AssignTask,
    /// The instance is assigned but has not started: send `comm_task_control`
    /// with action `start`.
    Start,
    /// The instance failed or went stale: send `comm_task_control` with
    /// action `retry`.
    Retry,
    /// The instance is actively running; nothing to do.
    AlreadyActive,
    /// The instance already completed; nothing to do.
    AlreadyDone,
    /// The instance is blocked on incomplete dependencies (listed).
    Blocked(Vec<String>),
}

impl JumpstartVerb {
    /// The `comm_task_control` action string, when this verb maps to one.
    pub fn task_control_action(&self) -> Option<&'static str> {
        match self {
            JumpstartVerb::Start => Some("start"),
            JumpstartVerb::Retry => Some("retry"),
            _ => None,
        }
    }
}

fn is_completed(status: &str) -> bool {
    matches!(status, "completed" | "done")
}

fn is_failed(status: &str) -> bool {
    matches!(status, "failed" | "stopped" | "crashed")
}

/// Dependencies of `item` that have not completed, given the full plan.
/// Unknown dependency ids count as incomplete: the scheduler will not run
/// the node either way.
fn incomplete_deps(item: &PlanItem, items: &[PlanItem]) -> Vec<String> {
    item.blocked_by
        .iter()
        .filter(|dep| {
            !items
                .iter()
                .any(|other| &other.id == *dep && is_completed(&other.status))
        })
        .cloned()
        .collect()
}

/// Decide the one verb that moves a plan instance forward.
///
/// Decision order (first match wins):
/// 1. completed -> [`JumpstartVerb::AlreadyDone`]
/// 2. failed / stopped / crashed / stale -> [`JumpstartVerb::Retry`]
/// 3. actively running -> [`JumpstartVerb::AlreadyActive`]
/// 4. blocked on incomplete dependencies -> [`JumpstartVerb::Blocked`]
/// 5. assigned but not started -> [`JumpstartVerb::Start`]
/// 6. unassigned and runnable -> [`JumpstartVerb::AssignTask`]
pub fn decide_jumpstart(item: &PlanItem, items: &[PlanItem]) -> JumpstartVerb {
    if is_completed(&item.status) {
        return JumpstartVerb::AlreadyDone;
    }
    if is_failed(&item.status) || item.status == "running_stale" {
        return JumpstartVerb::Retry;
    }
    if item.status == "running" {
        return JumpstartVerb::AlreadyActive;
    }
    let deps = incomplete_deps(item, items);
    if !deps.is_empty() {
        return JumpstartVerb::Blocked(deps);
    }
    if item.assigned_to.is_some() {
        return JumpstartVerb::Start;
    }
    JumpstartVerb::AssignTask
}

/// Pick the instance most in need of a jumpstart when the operator did not
/// name one: failed/stale first (retry), then assigned-but-unstarted (start),
/// then unassigned runnable (assign).
pub fn pick_jumpstart_node(items: &[PlanItem]) -> Option<&PlanItem> {
    let actionable = |verbs: &[fn(&JumpstartVerb) -> bool]| {
        items.iter().find(|item| {
            let verb = decide_jumpstart(item, items);
            verbs.iter().any(|matches| matches(&verb))
        })
    };
    actionable(&[|v| matches!(v, JumpstartVerb::Retry)])
        .or_else(|| actionable(&[|v| matches!(v, JumpstartVerb::Start)]))
        .or_else(|| actionable(&[|v| matches!(v, JumpstartVerb::AssignTask)]))
}

/// A roster member's displayed type, resolved per the three-tier rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedMemberType {
    /// Working a plan instance: display the instance's phase.
    Phase(String),
    /// Tool-spawned child with an upstream Agent-tool preset.
    Preset(String),
    /// Off-plan swarm member with a free-form swarm tag.
    Tag(String),
    Untyped,
}

impl ResolvedMemberType {
    /// Short user-facing label, using phase/instance vocabulary.
    pub fn display(&self) -> String {
        match self {
            ResolvedMemberType::Phase(phase) => format!("{phase} phase"),
            ResolvedMemberType::Preset(preset) => format!("preset {preset}"),
            ResolvedMemberType::Tag(tag) => tag.clone(),
            ResolvedMemberType::Untyped => "untyped".to_string(),
        }
    }
}

/// Resolve a member's displayed type: assigned instance's phase, else the
/// Agent-tool preset, else the free-form swarm tag, else untyped.
pub fn resolve_member_type(
    assigned_instance_phase: Option<&str>,
    agent_tool_preset: Option<&str>,
    swarm_tag: Option<&str>,
) -> ResolvedMemberType {
    let non_empty = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    if let Some(phase) = non_empty(assigned_instance_phase) {
        return ResolvedMemberType::Phase(phase);
    }
    if let Some(preset) = non_empty(agent_tool_preset) {
        return ResolvedMemberType::Preset(preset);
    }
    if let Some(tag) = non_empty(swarm_tag) {
        return ResolvedMemberType::Tag(tag);
    }
    ResolvedMemberType::Untyped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, status: &str) -> PlanItem {
        PlanItem {
            content: format!("work on {id}"),
            status: status.to_string(),
            priority: "medium".to_string(),
            id: id.to_string(),
            subsystem: None,
            file_scope: Vec::new(),
            blocked_by: Vec::new(),
            assigned_to: None,
        }
    }

    #[test]
    fn completed_instance_is_already_done() {
        let items = vec![item("a", "completed")];
        assert_eq!(
            decide_jumpstart(&items[0], &items),
            JumpstartVerb::AlreadyDone
        );
        let items = vec![item("a", "done")];
        assert_eq!(
            decide_jumpstart(&items[0], &items),
            JumpstartVerb::AlreadyDone
        );
    }

    #[test]
    fn failed_instance_needs_retry() {
        for status in ["failed", "stopped", "crashed", "running_stale"] {
            let items = vec![item("a", status)];
            assert_eq!(
                decide_jumpstart(&items[0], &items),
                JumpstartVerb::Retry,
                "status {status}"
            );
        }
    }

    #[test]
    fn stale_assigned_instance_still_retries_not_starts() {
        // The motivating incident: an assigned node that went stale must map
        // to retry, not start or assign.
        let mut stale = item("a", "running_stale");
        stale.assigned_to = Some("worker-1".to_string());
        let items = vec![stale];
        assert_eq!(decide_jumpstart(&items[0], &items), JumpstartVerb::Retry);
    }

    #[test]
    fn running_instance_is_already_active() {
        let items = vec![item("a", "running")];
        assert_eq!(
            decide_jumpstart(&items[0], &items),
            JumpstartVerb::AlreadyActive
        );
    }

    #[test]
    fn blocked_instance_lists_incomplete_deps() {
        let mut blocked = item("b", "queued");
        blocked.blocked_by = vec!["a".to_string(), "missing".to_string()];
        let items = vec![item("a", "queued"), blocked];
        assert_eq!(
            decide_jumpstart(&items[1], &items),
            JumpstartVerb::Blocked(vec!["a".to_string(), "missing".to_string()])
        );
    }

    #[test]
    fn completed_deps_do_not_block() {
        let mut node = item("b", "queued");
        node.blocked_by = vec!["a".to_string()];
        let items = vec![item("a", "completed"), node];
        assert_eq!(
            decide_jumpstart(&items[1], &items),
            JumpstartVerb::AssignTask
        );
    }

    #[test]
    fn assigned_unstarted_instance_needs_start() {
        let mut assigned = item("a", "queued");
        assigned.assigned_to = Some("worker-1".to_string());
        let items = vec![assigned];
        assert_eq!(decide_jumpstart(&items[0], &items), JumpstartVerb::Start);
    }

    #[test]
    fn unassigned_runnable_instance_needs_assign() {
        let items = vec![item("a", "queued")];
        assert_eq!(
            decide_jumpstart(&items[0], &items),
            JumpstartVerb::AssignTask
        );
    }

    #[test]
    fn task_control_action_mapping() {
        assert_eq!(JumpstartVerb::Start.task_control_action(), Some("start"));
        assert_eq!(JumpstartVerb::Retry.task_control_action(), Some("retry"));
        assert_eq!(JumpstartVerb::AssignTask.task_control_action(), None);
        assert_eq!(JumpstartVerb::AlreadyDone.task_control_action(), None);
    }

    #[test]
    fn pick_prefers_retry_then_start_then_assign() {
        let mut assigned = item("started-not-running", "queued");
        assigned.assigned_to = Some("w".to_string());
        let items = vec![
            item("free", "queued"),
            assigned.clone(),
            item("broken", "failed"),
        ];
        assert_eq!(
            pick_jumpstart_node(&items).map(|i| i.id.as_str()),
            Some("broken")
        );

        let items = vec![item("free", "queued"), assigned];
        assert_eq!(
            pick_jumpstart_node(&items).map(|i| i.id.as_str()),
            Some("started-not-running")
        );

        let items = vec![item("done", "completed"), item("free", "queued")];
        assert_eq!(
            pick_jumpstart_node(&items).map(|i| i.id.as_str()),
            Some("free")
        );

        let items = vec![item("done", "completed")];
        assert_eq!(pick_jumpstart_node(&items), None);
    }

    #[test]
    fn member_type_resolution_order() {
        assert_eq!(
            resolve_member_type(Some("verify"), Some("reviewer"), Some("manager")),
            ResolvedMemberType::Phase("verify".to_string())
        );
        assert_eq!(
            resolve_member_type(None, Some("reviewer"), Some("manager")),
            ResolvedMemberType::Preset("reviewer".to_string())
        );
        assert_eq!(
            resolve_member_type(None, None, Some("manager")),
            ResolvedMemberType::Tag("manager".to_string())
        );
        assert_eq!(
            resolve_member_type(None, None, None),
            ResolvedMemberType::Untyped
        );
        // Blank strings do not count as typing.
        assert_eq!(
            resolve_member_type(Some("  "), None, Some("")),
            ResolvedMemberType::Untyped
        );
    }

    #[test]
    fn member_type_display_uses_phase_vocabulary() {
        assert_eq!(
            resolve_member_type(Some("implement"), None, None).display(),
            "implement phase"
        );
        assert_eq!(ResolvedMemberType::Untyped.display(), "untyped");
    }
}
