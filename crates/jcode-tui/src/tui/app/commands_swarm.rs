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
use crate::protocol::{PlanGraphStatus, SwarmFleetEntry, SwarmMemberStatus};
use crate::swarm_verbs::resolve_member_type;
use std::collections::BTreeMap;

/// A parsed `/swarm` verb. `/swarm`, `/swarm on`, and `/swarm off` are not
/// verbs; they keep their existing feature-toggle handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::tui::app) enum SwarmVerb {
    /// Render the member roster with three-tier typing.
    Status,
    /// Request and render the plan/DAG status summary.
    Plan,
    /// Request and render the live fleet dashboard summary.
    Fleet,
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

pub(in crate::tui::app) const SWARM_VERB_USAGE: &str = "Usage: /swarm [status|plan|fleet|start [instance] [session]|stop <member>|spawn <label> [prompt]|on|off]";

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
        "fleet" | "swarms" | "list_swarms" | "fleet_status" | "list_fleet" => {
            Some(Ok(SwarmVerb::Fleet))
        }
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

/// One actionable plan instance surfaced from a fleet row, tagged with the
/// plan state that makes it worth acting on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::tui::app) struct FleetTarget {
    /// Plan instance id (the `/swarm start <id>` argument).
    pub instance_id: String,
    /// Plan state: `"failed"`, `"ready"`, or `"active"`.
    pub state: &'static str,
}

/// A structured, actionable view of one live swarm, derived purely from a
/// `SwarmFleetEntry`. This is the model behind the actionable fleet output:
/// it names the swarm, its coordinator, whether it needs operator input, and
/// the concrete plan instances worth driving next (failed → ready → active).
///
/// Wire reality: `SwarmFleetEntry` carries no per-member session ids, only the
/// coordinator and the plan's instance ids. The `/swarm` drive verbs
/// (`start`/`stop`/`plan`/`status`) act on the caller's own swarm, so we
/// surface the instance ids to target rather than pretend to dispatch into a
/// different swarm's session. Selecting a foreign swarm's work still requires
/// being attached to that swarm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::tui::app) struct FleetSelection {
    pub swarm_id: String,
    pub coordinator_session_id: Option<String>,
    /// Human-facing coordinator label (name → session id → "unknown").
    pub coordinator_display: String,
    pub needs_operator_input: bool,
    /// Actionable instance targets, ordered failed → ready → active.
    pub targets: Vec<FleetTarget>,
}

impl FleetSelection {
    /// The single best next instance to jumpstart, if any: a failed instance
    /// first (needs a retry), else a ready one.
    pub(in crate::tui::app) fn primary_target(&self) -> Option<&FleetTarget> {
        self.targets
            .iter()
            .find(|t| t.state == "failed")
            .or_else(|| self.targets.iter().find(|t| t.state == "ready"))
    }
}

/// Build the structured, attention-sorted selection model for a live fleet.
/// Swarms needing operator input sort first; ties break by `swarm_id` so the
/// order is stable. Each swarm's actionable instance targets are pulled from
/// its plan (failed → ready → active).
pub(in crate::tui::app) fn fleet_selection_rows(swarms: &[SwarmFleetEntry]) -> Vec<FleetSelection> {
    let mut rows: Vec<FleetSelection> = swarms
        .iter()
        .map(|swarm| {
            let coordinator_display = swarm
                .coordinator_name
                .as_deref()
                .or(swarm.coordinator_session_id.as_deref())
                .unwrap_or("unknown")
                .to_string();
            let mut targets = Vec::new();
            for id in &swarm.plan.failed_ids {
                targets.push(FleetTarget {
                    instance_id: id.clone(),
                    state: "failed",
                });
            }
            for id in &swarm.plan.ready_ids {
                targets.push(FleetTarget {
                    instance_id: id.clone(),
                    state: "ready",
                });
            }
            for id in &swarm.plan.active_ids {
                targets.push(FleetTarget {
                    instance_id: id.clone(),
                    state: "active",
                });
            }
            FleetSelection {
                swarm_id: swarm.swarm_id.clone(),
                coordinator_session_id: swarm.coordinator_session_id.clone(),
                coordinator_display,
                needs_operator_input: swarm.needs_operator_input,
                targets,
            }
        })
        .collect();
    // Attention first, then stable by swarm_id.
    rows.sort_by(|a, b| {
        b.needs_operator_input
            .cmp(&a.needs_operator_input)
            .then_with(|| a.swarm_id.cmp(&b.swarm_id))
    });
    rows
}

/// Render a live fleet/dashboard response from the daemon.
///
/// The output is actionable: swarms needing operator input are listed first
/// and flagged, and each swarm surfaces the concrete instance ids worth
/// driving next plus the exact `/swarm start <id>` command to run. The drive
/// verbs act on the caller's own swarm (see `FleetSelection`), so we surface
/// targets rather than dispatch across sessions.
pub(in crate::tui::app) fn render_swarm_fleet(swarms: &[SwarmFleetEntry]) -> String {
    if swarms.is_empty() {
        return "No live swarms found.".to_string();
    }

    let selections = fleet_selection_rows(swarms);
    // Index by swarm_id so we can render swarms attention-first while still
    // reading the rich per-swarm detail from the original entries.
    let by_id: BTreeMap<&str, &SwarmFleetEntry> = swarms
        .iter()
        .map(|swarm| (swarm.swarm_id.as_str(), swarm))
        .collect();

    let attention = selections.iter().filter(|s| s.needs_operator_input).count();
    let mut lines = if attention > 0 {
        vec![format!(
            "Live swarms ({} total, {} need attention):",
            selections.len(),
            attention
        )]
    } else {
        vec![format!("Live swarms ({} total):", selections.len())]
    };

    for selection in &selections {
        let Some(swarm) = by_id.get(selection.swarm_id.as_str()) else {
            continue;
        };
        let coordinator_status = swarm.coordinator_status.as_deref().unwrap_or("unknown");
        let flag = if selection.needs_operator_input {
            " ⚠ attention"
        } else {
            ""
        };
        lines.push(format!(
            "• {}: {} member(s), coordinator {} ({}){}",
            selection.swarm_id,
            swarm.member_count,
            selection.coordinator_display,
            coordinator_status,
            flag
        ));
        if !swarm.members_by_status.is_empty() {
            let statuses = swarm
                .members_by_status
                .iter()
                .map(|(status, count)| format!("{status}:{count}"))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("  status: {statuses}"));
        }
        if !swarm.members_by_type.is_empty() {
            let types = swarm
                .members_by_type
                .iter()
                .map(|(kind, count)| format!("{kind}:{count}"))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("  type: {types}"));
        }
        lines.push(format!(
            "  plan: {} instance(s), {} active, {} ready, {} failed, mode {}",
            swarm.plan.item_count,
            swarm.plan.active_ids.len(),
            swarm.plan.ready_ids.len(),
            swarm.plan.failed_ids.len(),
            swarm.plan.mode
        ));
        if let Some(tokens) = &swarm.tokens {
            lines.push(format!(
                "  tokens: in {}, out {}, messages {}",
                tokens.input_tokens, tokens.output_tokens, tokens.messages_with_token_usage
            ));
        }
        if let Some(age) = swarm.last_activity_age_secs {
            lines.push(format!("  last activity: {age}s ago"));
        }
        if let Some(offset) = swarm.control_log_offset {
            lines.push(format!("  control log offset: {offset}"));
        }
        // Actionable targets: name the runnable instances and the exact verb.
        let runnable = selection
            .targets
            .iter()
            .filter(|t| t.state == "failed" || t.state == "ready")
            .collect::<Vec<_>>();
        if !runnable.is_empty() {
            let listed = runnable
                .iter()
                .take(4)
                .map(|t| format!("{} ({})", t.instance_id, t.state))
                .collect::<Vec<_>>()
                .join(", ");
            let more = runnable.len().saturating_sub(4);
            let suffix = if more > 0 {
                format!(", +{more} more")
            } else {
                String::new()
            };
            lines.push(format!("  runnable: {listed}{suffix}"));
        }
        if let Some(target) = selection.primary_target() {
            lines.push(format!(
                "  → act: /swarm start {} (this swarm's session)",
                target.instance_id
            ));
        }
    }
    lines.join("\n")
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
/// Note: older `swarm_plan` wire events did not carry per-instance phase;
/// those legacy events still fall back to the assigned member tag.
pub(in crate::tui::app) fn render_swarm_roster(
    members: &[SwarmMemberStatus],
    items: &[PlanItem],
    phases_by_id: &BTreeMap<String, String>,
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
        // legacy events without phases fall back to the member tag.
        let assigned_phase = instance
            .and_then(|item| phases_by_id.get(&item.id).map(String::as_str))
            .or_else(|| instance.and(member.subagent_type.as_deref()));
        let resolved = resolve_member_type(assigned_phase, None, member.subagent_type.as_deref());
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
        assert_eq!(parse_swarm_verb("/swarm fleet"), Some(Ok(SwarmVerb::Fleet)));
        assert_eq!(
            parse_swarm_verb("/swarm swarms"),
            Some(Ok(SwarmVerb::Fleet))
        );
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
        let phases_by_id = BTreeMap::from([("impl-1".to_string(), "verify".to_string())]);
        let out = render_swarm_roster(&members, &items, &phases_by_id);
        // Assigned member: instance phase overrides the worker tag.
        assert!(out.contains("bat — running — verify phase — instance impl-1"));
        // Off-plan tagged member: free-form tag.
        assert!(out.contains("hen — ready — manager"));
        // Untyped member.
        assert!(out.contains("owl — ready — untyped"));
    }

    #[test]
    fn roster_empty_message() {
        assert!(render_swarm_roster(&[], &[], &BTreeMap::new()).contains("No swarm members"));
    }

    #[test]
    fn roster_legacy_plan_without_phase_falls_back_to_assigned_tag() {
        let members = vec![member("bat", "running", Some("implement"))];
        let items = vec![item("impl-1", "running", Some("bat"))];
        let out = render_swarm_roster(&members, &items, &BTreeMap::new());
        assert!(out.contains("bat — running — implement phase — instance impl-1"));
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

    #[test]
    fn fleet_empty_message() {
        assert_eq!(render_swarm_fleet(&[]), "No live swarms found.");
    }

    #[test]
    fn fleet_renders_live_rollup() {
        let mut plan = PlanGraphStatus::empty_for_swarm("swarm-a");
        plan.mode = "deep".to_string();
        plan.item_count = 4;
        plan.active_ids = vec!["impl-1".to_string()];
        plan.ready_ids = vec!["verify-1".to_string()];
        plan.failed_ids = vec!["fix-1".to_string()];
        let swarm = SwarmFleetEntry {
            swarm_id: "swarm-a".to_string(),
            coordinator_session_id: Some("session_bat".to_string()),
            coordinator_name: Some("bat".to_string()),
            coordinator_status: Some("running".to_string()),
            member_count: 3,
            members: Vec::new(),
            members_by_status: BTreeMap::from([("running".to_string(), 2)]),
            members_by_type: BTreeMap::from([("implement".to_string(), 1)]),
            plan,
            needs_operator_input: true,
            tokens: None,
            last_activity_age_secs: Some(12),
            control_log_offset: Some(42),
        };
        let out = render_swarm_fleet(&[swarm]);
        assert!(out.contains("Live swarms (1 total, 1 need attention):"));
        assert!(out.contains("swarm-a: 3 member(s), coordinator bat (running) ⚠ attention"));
        assert!(out.contains("status: running:2"));
        assert!(out.contains("type: implement:1"));
        assert!(out.contains("plan: 4 instance(s), 1 active, 1 ready, 1 failed, mode deep"));
        assert!(out.contains("last activity: 12s ago"));
        assert!(out.contains("control log offset: 42"));
        // Actionable surface: runnable instances (failed first) and the verb.
        assert!(out.contains("runnable: fix-1 (failed), verify-1 (ready)"));
        assert!(out.contains("→ act: /swarm start fix-1"));
    }

    fn fleet_entry(swarm_id: &str, attention: bool) -> SwarmFleetEntry {
        SwarmFleetEntry {
            swarm_id: swarm_id.to_string(),
            coordinator_session_id: Some(format!("session_{swarm_id}")),
            coordinator_name: Some(swarm_id.to_string()),
            coordinator_status: Some("running".to_string()),
            member_count: 1,
            members: Vec::new(),
            members_by_status: BTreeMap::new(),
            members_by_type: BTreeMap::new(),
            plan: PlanGraphStatus::empty_for_swarm(swarm_id),
            needs_operator_input: attention,
            tokens: None,
            last_activity_age_secs: None,
            control_log_offset: None,
        }
    }

    #[test]
    fn fleet_selection_sorts_attention_first_then_by_id() {
        let calm_z = fleet_entry("zulu", false);
        let calm_a = fleet_entry("alpha", false);
        let hot = fleet_entry("mike", true);
        let rows = fleet_selection_rows(&[calm_z, calm_a, hot]);
        let order: Vec<&str> = rows.iter().map(|r| r.swarm_id.as_str()).collect();
        // Attention swarm first, then the calm swarms alphabetically.
        assert_eq!(order, vec!["mike", "alpha", "zulu"]);
        assert!(rows[0].needs_operator_input);
    }

    #[test]
    fn fleet_selection_targets_order_failed_then_ready_then_active() {
        let mut entry = fleet_entry("swarm-x", false);
        entry.plan.failed_ids = vec!["fix-1".to_string()];
        entry.plan.ready_ids = vec!["verify-1".to_string()];
        entry.plan.active_ids = vec!["impl-1".to_string()];
        let rows = fleet_selection_rows(&[entry]);
        let selection = &rows[0];
        let states: Vec<(&str, &str)> = selection
            .targets
            .iter()
            .map(|t| (t.instance_id.as_str(), t.state))
            .collect();
        assert_eq!(
            states,
            vec![
                ("fix-1", "failed"),
                ("verify-1", "ready"),
                ("impl-1", "active")
            ]
        );
        // Primary target prefers the failed instance (needs a retry).
        assert_eq!(
            selection.primary_target().map(|t| t.instance_id.as_str()),
            Some("fix-1")
        );
    }

    #[test]
    fn fleet_selection_primary_target_falls_back_to_ready() {
        let mut entry = fleet_entry("swarm-y", false);
        entry.plan.ready_ids = vec!["verify-2".to_string()];
        entry.plan.active_ids = vec!["impl-2".to_string()];
        let rows = fleet_selection_rows(&[entry]);
        // No failed instance: primary target is the ready one, not the active.
        assert_eq!(
            rows[0].primary_target().map(|t| t.instance_id.as_str()),
            Some("verify-2")
        );
    }

    #[test]
    fn fleet_selection_no_runnable_targets_when_only_active() {
        let mut entry = fleet_entry("swarm-z", false);
        entry.plan.active_ids = vec!["impl-3".to_string()];
        let rows = fleet_selection_rows(&[entry]);
        // An active-only swarm has an active target but nothing to jumpstart.
        assert!(rows[0].primary_target().is_none());
        let out = render_swarm_fleet(&[fleet_entry("swarm-z", false)]);
        assert!(!out.contains("→ act:"));
    }
}
