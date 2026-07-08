//! `/swarm` verb commands: bind the wire `comm_*` drive verbs to slash
//! commands so an operator can steer a running swarm from inside the chat.
//!
//! Parsing and rendering live here as pure functions (unit-testable without
//! an `App`); dispatch is wired in `remote/key_handling.rs`. The wedge-
//! jumpstart decision (assign vs start vs retry) is shared logic in
//! `jcode_app_core::swarm_verbs` so other clients reuse it.
//!
//! Vocabulary: a plan node is an "instance"; its kind displays as "phase".

use crate::plan::PlanItem;
use crate::protocol::{PlanGraphStatus, SwarmMemberStatus};
use crate::swarm_verbs::resolve_member_type;

/// A parsed `/swarm` verb. `/swarm`, `/swarm on`, and `/swarm off` are not
/// verbs; they keep their existing feature-toggle handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::tui::app) enum SwarmVerb {
    /// Render the member roster with three-tier typing.
    Status,
    /// Request and render the plan/DAG status summary.
    Plan,
    /// Wedge-jumpstart: move one instance forward with the right verb.
    Start {
        node_id: Option<String>,
        session: Option<String>,
    },
    /// Stop a swarm member.
    Stop { member: String },
    /// Spawn a new swarm member with a label and optional initial prompt.
    Spawn {
        label: String,
        prompt: Option<String>,
    },
}

pub(in crate::tui::app) const SWARM_VERB_USAGE: &str = "Usage: /swarm [status|plan|start [instance] [session]|stop <member>|spawn <label> [prompt]|on|off]";

/// Parse a `/swarm <verb> ...` command. Returns `None` when the input is not
/// a swarm verb (so feature-toggle handling can run), `Some(Err)` on a
/// malformed verb.
pub(in crate::tui::app) fn parse_swarm_verb(trimmed: &str) -> Option<Result<SwarmVerb, String>> {
    let rest = trimmed.strip_prefix("/swarm")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let mut tokens = rest.split_whitespace();
    let verb = tokens.next()?;
    match verb {
        "status" | "roster" | "members" => Some(Ok(SwarmVerb::Status)),
        "plan" | "dag" => Some(Ok(SwarmVerb::Plan)),
        "start" | "jumpstart" => {
            let node_id = tokens.next().map(str::to_string);
            let session = tokens.next().map(str::to_string);
            if tokens.next().is_some() {
                return Some(Err(
                    "Too many arguments. Usage: /swarm start [instance] [session]".to_string(),
                ));
            }
            Some(Ok(SwarmVerb::Start { node_id, session }))
        }
        "stop" => match tokens.next() {
            Some(member) if tokens.next().is_none() => Some(Ok(SwarmVerb::Stop {
                member: member.to_string(),
            })),
            _ => Some(Err("Usage: /swarm stop <member>".to_string())),
        },
        "spawn" => match tokens.next() {
            Some(label) => {
                let prompt = tokens.collect::<Vec<_>>().join(" ");
                Some(Ok(SwarmVerb::Spawn {
                    label: label.to_string(),
                    prompt: if prompt.is_empty() {
                        None
                    } else {
                        Some(prompt)
                    },
                }))
            }
            None => Some(Err(
                "Usage: /swarm spawn <label> [initial prompt]".to_string()
            )),
        },
        // Feature toggles and bare status keep their existing handlers.
        "on" | "off" => None,
        other => Some(Err(format!(
            "Unknown swarm verb '{other}'. {SWARM_VERB_USAGE}"
        ))),
    }
}

/// The plan instance a member is currently assigned to, if any.
fn assigned_instance<'a>(
    member: &SwarmMemberStatus,
    items: &'a [PlanItem],
) -> Option<&'a PlanItem> {
    items
        .iter()
        .find(|item| item.assigned_to.as_deref() == Some(member.session_id.as_str()))
}

/// Render the swarm roster with three-tier member typing: assigned
/// instance's phase (when known), else Agent-tool preset, else free-form
/// swarm tag, else untyped.
///
/// Note: the `swarm_plan` wire event does not currently carry per-instance
/// phase (`kind` lives server-side in `NodeMeta`), so tier 1 resolves from
/// the member's tag when it matches an assigned instance; the shared
/// resolver in `jcode_app_core::swarm_verbs` is ready for the field once the
/// wire carries it.
pub(in crate::tui::app) fn render_swarm_roster(
    members: &[SwarmMemberStatus],
    items: &[PlanItem],
) -> String {
    if members.is_empty() {
        return "No swarm members known for this session yet.".to_string();
    }
    let mut lines = vec![format!("Swarm roster ({} members):", members.len())];
    for member in members {
        let name = member
            .friendly_name
            .clone()
            .unwrap_or_else(|| member.session_id.chars().take(12).collect());
        let instance = assigned_instance(member, items);
        // Tier 1 uses the assigned instance's phase when the wire carries it;
        // today it does not, so an assigned member's tag doubles as its
        // phase and unassigned members fall through to preset/tag.
        let resolved = resolve_member_type(
            instance.and(member.subagent_type.as_deref()),
            None,
            member.subagent_type.as_deref(),
        );
        let mut line = format!("• {} — {} — {}", name, member.status, resolved.display());
        if let Some(role) = member.role.as_deref()
            && role == "coordinator"
        {
            line.push_str(" — coordinator");
        }
        if let Some(item) = instance {
            line.push_str(&format!(" — instance {}", item.id));
        }
        if let Some(label) = member
            .task_label
            .as_deref()
            .or(member.detail.as_deref())
            .filter(|s| !s.is_empty())
        {
            line.push_str(&format!(" — {}", crate::util::truncate_str(label, 48)));
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn list_ids(label: &str, ids: &[String]) -> Option<String> {
    if ids.is_empty() {
        None
    } else {
        Some(format!("{label}: {}", ids.join(", ")))
    }
}

/// Render a plan/DAG status summary from a `PlanGraphStatus`, flagging
/// failed reasons and low-confidence instances.
pub(in crate::tui::app) fn render_plan_status(
    summary: &PlanGraphStatus,
    items: &[PlanItem],
) -> String {
    if summary.item_count == 0 {
        return "The swarm plan is empty. Seed it with the swarm tool (task_graph).".to_string();
    }
    let content_of = |id: &str| -> String {
        items
            .iter()
            .find(|item| item.id == id)
            .map(|item| format!(" ({})", crate::util::truncate_str(&item.content, 40)))
            .unwrap_or_default()
    };
    let mut lines = vec![format!(
        "Plan v{} · {} mode · {} instances: {} ready, {} blocked, {} active, {} completed, {} failed",
        summary.version,
        summary.mode,
        summary.item_count,
        summary.ready_ids.len(),
        summary.blocked_ids.len(),
        summary.active_ids.len(),
        summary.completed_ids.len(),
        summary.failed_ids.len(),
    )];
    for entry in [
        list_ids("Ready", &summary.ready_ids),
        list_ids("Active", &summary.active_ids),
        list_ids("Blocked", &summary.blocked_ids),
    ]
    .into_iter()
    .flatten()
    {
        lines.push(entry);
    }
    for id in &summary.failed_ids {
        let reason = summary
            .failed_reasons
            .get(id)
            .map(|reason| format!(": {reason}"))
            .unwrap_or_default();
        lines.push(format!("✗ failed {}{}{}", id, content_of(id), reason));
    }
    if !summary.low_confidence_ids.is_empty() {
        lines.push(format!(
            "⚠ low-confidence instances: {}",
            summary.low_confidence_ids.join(", ")
        ));
    }
    if !summary.ready_ids.is_empty() || !summary.failed_ids.is_empty() {
        lines.push("Use /swarm start [instance] to move one forward.".to_string());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn member(session_id: &str, status: &str, tag: Option<&str>) -> SwarmMemberStatus {
        SwarmMemberStatus {
            session_id: session_id.to_string(),
            friendly_name: Some(session_id.to_string()),
            status: status.to_string(),
            detail: None,
            task_label: None,
            subagent_type: tag.map(str::to_string),
            role: None,
            is_headless: None,
            live_attachments: None,
            status_age_secs: None,
            output_tail: None,
            report_back_to_session_id: None,
            todo_progress: None,
            todo_items: Vec::new(),
            initial_prompt_delivered: None,
        }
    }

    fn item(id: &str, status: &str, assigned_to: Option<&str>) -> PlanItem {
        PlanItem {
            content: format!("work {id}"),
            status: status.to_string(),
            priority: "medium".to_string(),
            id: id.to_string(),
            subsystem: None,
            file_scope: Vec::new(),
            blocked_by: Vec::new(),
            assigned_to: assigned_to.map(str::to_string),
        }
    }

    #[test]
    fn parse_status_plan_and_aliases() {
        assert_eq!(
            parse_swarm_verb("/swarm status"),
            Some(Ok(SwarmVerb::Status))
        );
        assert_eq!(
            parse_swarm_verb("/swarm roster"),
            Some(Ok(SwarmVerb::Status))
        );
        assert_eq!(parse_swarm_verb("/swarm plan"), Some(Ok(SwarmVerb::Plan)));
        assert_eq!(parse_swarm_verb("/swarm dag"), Some(Ok(SwarmVerb::Plan)));
    }

    #[test]
    fn parse_start_variants() {
        assert_eq!(
            parse_swarm_verb("/swarm start"),
            Some(Ok(SwarmVerb::Start {
                node_id: None,
                session: None
            }))
        );
        assert_eq!(
            parse_swarm_verb("/swarm start impl-1"),
            Some(Ok(SwarmVerb::Start {
                node_id: Some("impl-1".to_string()),
                session: None
            }))
        );
        assert_eq!(
            parse_swarm_verb("/swarm start impl-1 bat"),
            Some(Ok(SwarmVerb::Start {
                node_id: Some("impl-1".to_string()),
                session: Some("bat".to_string())
            }))
        );
        assert!(matches!(
            parse_swarm_verb("/swarm start a b c"),
            Some(Err(_))
        ));
    }

    #[test]
    fn parse_stop_and_spawn() {
        assert_eq!(
            parse_swarm_verb("/swarm stop bat"),
            Some(Ok(SwarmVerb::Stop {
                member: "bat".to_string()
            }))
        );
        assert!(matches!(parse_swarm_verb("/swarm stop"), Some(Err(_))));
        assert_eq!(
            parse_swarm_verb("/swarm spawn reviewer"),
            Some(Ok(SwarmVerb::Spawn {
                label: "reviewer".to_string(),
                prompt: None
            }))
        );
        assert_eq!(
            parse_swarm_verb("/swarm spawn reviewer review the API surface"),
            Some(Ok(SwarmVerb::Spawn {
                label: "reviewer".to_string(),
                prompt: Some("review the API surface".to_string())
            }))
        );
    }

    #[test]
    fn feature_toggles_fall_through() {
        assert_eq!(parse_swarm_verb("/swarm"), None);
        assert_eq!(parse_swarm_verb("/swarm on"), None);
        assert_eq!(parse_swarm_verb("/swarm off"), None);
        assert_eq!(parse_swarm_verb("/swarmish"), None);
    }

    #[test]
    fn unknown_verb_errors_with_usage() {
        let parsed = parse_swarm_verb("/swarm bogus");
        assert!(matches!(parsed, Some(Err(ref e)) if e.contains("bogus")));
    }

    #[test]
    fn roster_renders_three_tier_typing_and_instances() {
        let members = vec![
            member("bat", "running", Some("implement")),
            member("hen", "ready", Some("manager")),
            member("owl", "ready", None),
        ];
        let items = vec![item("impl-1", "running", Some("bat"))];
        let out = render_swarm_roster(&members, &items);
        // Assigned member: tag doubles as phase, instance shown.
        assert!(out.contains("bat — running — implement phase — instance impl-1"));
        // Off-plan tagged member: free-form tag.
        assert!(out.contains("hen — ready — manager"));
        // Untyped member.
        assert!(out.contains("owl — ready — untyped"));
    }

    #[test]
    fn roster_empty_message() {
        assert!(render_swarm_roster(&[], &[]).contains("No swarm members"));
    }

    #[test]
    fn plan_status_lists_failures_and_low_confidence() {
        let mut summary = PlanGraphStatus::empty_for_swarm("s");
        summary.version = 4;
        summary.item_count = 3;
        summary.ready_ids = vec!["b".to_string()];
        summary.completed_ids = vec!["a".to_string()];
        summary.failed_ids = vec!["c".to_string()];
        summary.failed_reasons = BTreeMap::from([("c".to_string(), "API error (401)".to_string())]);
        summary.low_confidence_ids = vec!["a".to_string()];
        let items = vec![item("c", "failed", None)];
        let out = render_plan_status(&summary, &items);
        assert!(out.contains("Plan v4"));
        assert!(out.contains("1 ready"));
        assert!(out.contains("✗ failed c (work c): API error (401)"));
        assert!(out.contains("low-confidence instances: a"));
        assert!(out.contains("/swarm start"));
    }

    #[test]
    fn plan_status_empty_plan() {
        let summary = PlanGraphStatus::empty_for_swarm("s");
        assert!(render_plan_status(&summary, &[]).contains("empty"));
    }
}
