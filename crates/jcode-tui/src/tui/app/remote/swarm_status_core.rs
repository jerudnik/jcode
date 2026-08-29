//! Pure diffing of swarm member snapshots into user-facing status notices.
//!
//! The server streams full `SwarmStatus` snapshots; the strip renders them,
//! but lifecycle transitions (an agent finishing, failing, or blocking) used
//! to pass silently. This module compares the previous snapshot with the next
//! one and produces a compact one-line notice in the same spirit as the
//! "Swarm plan synced" notice from [`super::swarm_plan_core`].

use crate::protocol::SwarmMemberStatus;
use jcode_swarm_core::MemberLifecycleState;
use jcode_tui_render::swarm_gallery::is_active_status;

/// How many member names to list per transition category before collapsing
/// the rest into "+N".
const MAX_NAMES_PER_CATEGORY: usize = 3;

/// Lifecycle buckets worth announcing when a member newly enters them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Transition {
    Succeeded,
    Failed,
    Stopped,
    Lost,
}

impl Transition {
    fn verb(self) -> &'static str {
        match self {
            Transition::Succeeded => "succeeded",
            Transition::Failed => "failed",
            Transition::Stopped => "stopped",
            Transition::Lost => "lost",
        }
    }
}

fn member_label(member: &SwarmMemberStatus) -> String {
    member
        .friendly_name
        .clone()
        .unwrap_or_else(|| member.session_id.chars().take(8).collect())
}

/// Classify a status change into an announceable transition, if any.
///
/// Transitions into active states and startup transitions (`starting` →
/// `ready`) are intentionally silent: spawning is user-initiated and already
/// visible.
///
/// Compare lifecycle states rather than the raw strings. The wire still
/// carries older spellings of the same state, so matching literals here would
/// drop a real completion reported as `completed` and would announce a
/// spurious transition when a member's spelling changed but its state did not.
fn classify(prev_status: &str, next_status: &str) -> Option<Transition> {
    let prev = MemberLifecycleState::from_compatibility_status(prev_status);
    let next = MemberLifecycleState::from_compatibility_status(next_status);
    if prev == next {
        return None;
    }
    match next {
        MemberLifecycleState::Succeeded => Some(Transition::Succeeded),
        MemberLifecycleState::Failed => Some(Transition::Failed),
        MemberLifecycleState::Stopped => Some(Transition::Stopped),
        MemberLifecycleState::Lost => Some(Transition::Lost),
        _ => None,
    }
}

fn format_names(mut names: Vec<String>) -> String {
    names.sort();
    let hidden = names.len().saturating_sub(MAX_NAMES_PER_CATEGORY);
    names.truncate(MAX_NAMES_PER_CATEGORY);
    let mut out = names.join(", ");
    if hidden > 0 {
        out.push_str(&format!(" +{hidden}"));
    }
    out
}

/// Diff two swarm snapshots and build a status notice describing member
/// lifecycle transitions, e.g. `🐝 bat succeeded · 2/7 active` or
/// `🐝 crab failed · hen blocked · all 7 finished`. Returns `None` when
/// nothing announceable changed.
pub(in crate::tui::app) fn swarm_status_transition_notice(
    prev: &[SwarmMemberStatus],
    next: &[SwarmMemberStatus],
) -> Option<String> {
    if prev.is_empty() || next.is_empty() {
        return None;
    }
    let prev_status: std::collections::HashMap<&str, &str> = prev
        .iter()
        .map(|m| (m.session_id.as_str(), m.status.as_str()))
        .collect();

    let mut buckets: Vec<(Transition, Vec<String>)> = Vec::new();
    for member in next {
        let Some(prev_status) = prev_status.get(member.session_id.as_str()) else {
            // New member: spawning is user/agent initiated and already visible.
            continue;
        };
        if let Some(transition) = classify(prev_status, &member.status) {
            match buckets.iter_mut().find(|(t, _)| *t == transition) {
                Some((_, names)) => names.push(member_label(member)),
                None => buckets.push((transition, vec![member_label(member)])),
            }
        }
    }
    if buckets.is_empty() {
        return None;
    }
    buckets.sort_by_key(|(t, _)| *t);

    let mut segments: Vec<String> = buckets
        .into_iter()
        .map(|(transition, names)| format!("{} {}", format_names(names), transition.verb()))
        .collect();

    // Tail: the same "M/N active" tally the strip shows, or a wrap-up line
    // when nothing is working anymore.
    let active = next.iter().filter(|m| is_active_status(&m.status)).count();
    segments.push(if active > 0 {
        format!("{active}/{} active", next.len())
    } else {
        format!("all {} finished", next.len())
    });

    Some(format!("🐝 {}", segments.join(" · ")))
}

#[cfg(test)]
mod tests {
    use super::swarm_status_transition_notice;
    use crate::protocol::SwarmMemberStatus;

    fn member(id: &str, status: &str) -> SwarmMemberStatus {
        SwarmMemberStatus {
            session_id: id.to_string(),
            friendly_name: Some(id.to_string()),
            status: status.to_string(),
            detail: None,
            task_label: None,
            subagent_type: None,
            role: None,
            is_headless: Some(true),
            live_attachments: None,
            status_age_secs: Some(1),
            output_tail: None,
            report_back_to_session_id: None,
            initial_prompt_delivered: None,
            todo_progress: None,
            todo_items: Vec::new(),
            runtime: crate::protocol::SwarmMemberRuntime::default(),
        }
    }

    #[test]
    fn agent_completing_is_announced_with_active_tally() {
        let prev = vec![member("ant", "running"), member("bat", "running")];
        let next = vec![member("ant", "succeeded"), member("bat", "running")];
        assert_eq!(
            swarm_status_transition_notice(&prev, &next).as_deref(),
            Some("🐝 ant succeeded · 1/2 active")
        );
    }

    #[test]
    fn older_spellings_of_success_are_still_announced() {
        let prev = vec![member("ant", "running"), member("bat", "running")];
        let next = vec![member("ant", "completed"), member("bat", "running")];
        assert_eq!(
            swarm_status_transition_notice(&prev, &next).as_deref(),
            Some("🐝 ant succeeded · 1/2 active"),
            "the wire still carries `completed` for a succeeded member"
        );
    }

    #[test]
    fn respelling_the_same_state_is_silent() {
        let prev = vec![member("ant", "completed"), member("bat", "running")];
        let next = vec![member("ant", "succeeded"), member("bat", "running")];
        assert_eq!(
            swarm_status_transition_notice(&prev, &next),
            None,
            "the member did not transition, only its spelling did"
        );
    }

    #[test]
    fn ready_after_working_is_not_reinterpreted_as_success() {
        let prev = vec![member("ant", "running"), member("bat", "running")];
        let next = vec![member("ant", "ready"), member("bat", "running")];
        assert_eq!(swarm_status_transition_notice(&prev, &next), None);
    }

    #[test]
    fn ready_on_startup_is_silent() {
        let prev = vec![member("ant", "starting"), member("bat", "running")];
        let next = vec![member("ant", "ready"), member("bat", "running")];
        assert_eq!(swarm_status_transition_notice(&prev, &next), None);
    }

    #[test]
    fn failure_and_loss_are_announced_together() {
        let prev = vec![
            member("ant", "running"),
            member("bat", "running"),
            member("crab", "running"),
        ];
        let next = vec![
            member("ant", "failed"),
            member("bat", "lost"),
            member("crab", "running"),
        ];
        assert_eq!(
            swarm_status_transition_notice(&prev, &next).as_deref(),
            Some("🐝 ant failed · bat lost · 1/3 active")
        );
    }

    #[test]
    fn last_agent_finishing_reports_all_finished() {
        let prev = vec![member("ant", "succeeded"), member("bat", "running")];
        let next = vec![member("ant", "succeeded"), member("bat", "succeeded")];
        assert_eq!(
            swarm_status_transition_notice(&prev, &next).as_deref(),
            Some("🐝 bat succeeded · all 2 finished")
        );
    }

    #[test]
    fn unchanged_snapshot_is_silent() {
        let prev = vec![member("ant", "running"), member("bat", "succeeded")];
        assert_eq!(swarm_status_transition_notice(&prev, &prev.clone()), None);
    }

    #[test]
    fn unchanged_active_member_and_new_member_are_silent() {
        let prev = vec![member("ant", "running")];
        let next = vec![
            member("ant", "running"),  // no lifecycle transition
            member("bat", "starting"), // new member: spawn already visible
        ];
        assert_eq!(swarm_status_transition_notice(&prev, &next), None);
    }

    #[test]
    fn first_snapshot_is_silent() {
        let next = vec![member("ant", "succeeded")];
        assert_eq!(swarm_status_transition_notice(&[], &next), None);
    }

    #[test]
    fn many_names_collapse_into_more_count() {
        let prev: Vec<_> = ["ant", "bat", "crab", "dove", "elk"]
            .iter()
            .map(|id| member(id, "running"))
            .collect();
        let next: Vec<_> = ["ant", "bat", "crab", "dove", "elk"]
            .iter()
            .map(|id| member(id, "succeeded"))
            .collect();
        assert_eq!(
            swarm_status_transition_notice(&prev, &next).as_deref(),
            Some("🐝 ant, bat, crab +2 succeeded · all 5 finished")
        );
    }

    #[test]
    fn unnamed_member_falls_back_to_session_id_prefix() {
        let mut prev_member = member("session-long-identifier", "running");
        prev_member.friendly_name = None;
        let mut next_member = member("session-long-identifier", "succeeded");
        next_member.friendly_name = None;
        assert_eq!(
            swarm_status_transition_notice(&[prev_member], &[next_member]).as_deref(),
            Some("🐝 session- succeeded · all 1 finished")
        );
    }
}
