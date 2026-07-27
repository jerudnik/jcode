use super::{ToolContext, check_error};
use crate::protocol::{PlanGraphStatus, ServerEvent, TaskGraphNodeSpec};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

pub(super) fn seed_node_id_collision(response: &ServerEvent) -> Option<&str> {
    let message = check_error(response)?;
    let (_, tail) = message.split_once("duplicate node id '")?;
    let (id, _) = tail.split_once('\'')?;
    (!id.is_empty()).then_some(id)
}

pub(super) fn plan_graph_node_ids(summary: &PlanGraphStatus) -> HashSet<String> {
    summary
        .ready_ids
        .iter()
        .chain(&summary.blocked_ids)
        .chain(&summary.active_ids)
        .chain(&summary.completed_ids)
        .chain(&summary.failed_ids)
        .chain(&summary.cycle_ids)
        .chain(&summary.unresolved_dependency_ids)
        .cloned()
        .collect()
}

pub(super) fn seed_retry_scope(ctx: &ToolContext) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ctx.session_id.hash(&mut hasher);
    ctx.message_id.hash(&mut hasher);
    format!("seed-{:08x}", hasher.finish() as u32)
}

/// Rename only seed ids that collide with the existing durable plan, then rewrite
/// intra-batch dependency edges to follow them. The scope is stable for a tool
/// turn, so retrying the same call produces the same ids and is itself idempotent.
pub(super) fn remap_conflicting_seed_nodes(
    nodes: &[TaskGraphNodeSpec],
    occupied: &HashSet<String>,
    conflicting_id: &str,
    scope: &str,
) -> (Vec<TaskGraphNodeSpec>, Vec<(String, String)>) {
    let original_ids: HashSet<&str> = nodes.iter().map(|node| node.id.as_str()).collect();
    let mut reserved = occupied.clone();
    reserved.extend(original_ids.iter().map(|id| (*id).to_string()));
    let mut mapping = HashMap::<String, String>::new();

    if occupied.contains(conflicting_id) && nodes.iter().any(|node| node.id == conflicting_id) {
        let node_id = conflicting_id.to_string();
        let base = format!("{conflicting_id}::{scope}");
        let mut candidate = base.clone();
        let mut discriminator = 2usize;
        while reserved.contains(&candidate) {
            candidate = format!("{base}-{discriminator}");
            discriminator += 1;
        }
        reserved.insert(candidate.clone());
        mapping.insert(node_id, candidate);
    }

    let remapped = nodes
        .iter()
        .cloned()
        .map(|mut node| {
            if let Some(id) = mapping.get(&node.id) {
                node.id = id.clone();
            }
            for dependency in &mut node.depends_on {
                if let Some(id) = mapping.get(dependency) {
                    *dependency = id.clone();
                }
            }
            node
        })
        .collect();
    let changes = nodes
        .iter()
        .filter_map(|node| {
            mapping
                .get(&node.id)
                .map(|mapped| (node.id.clone(), mapped.clone()))
        })
        .collect();
    (remapped, changes)
}

pub(super) fn format_seed_remaps(changes: &[(String, String)]) -> String {
    changes
        .iter()
        .map(|(from, to)| format!("{from} -> {to}"))
        .collect::<Vec<_>>()
        .join(", ")
}
