//! Assignment grants for swarm plan workers.
//!
//! A worker's authority comes from what its plan node is for, not from its
//! prompt. The server derives a grant from the node's kind at assignment time
//! and enforces it in the tool dispatch path, so a worker whose prompt says
//! "you may edit files" still cannot edit files when its node is an explore,
//! critique, or synthesize node. Nothing a worker sends over the wire can
//! raise its own grant: installation happens only in the server's assignment
//! seam, and the registry is keyed by session id.
//!
//! Grant meanings:
//! - `ReadOnly`: reading, searching, and swarm reporting verbs only. No file
//!   mutation, no shell, no spawning. Explore, critique, and synthesize nodes.
//! - `Verify`: like `ReadOnly` plus shell, because verification means running
//!   builds and tests. File-mutation tools stay denied: a verify worker that
//!   "fixes" the code it is grading has stopped verifying. The shell itself is
//!   not yet sandboxed; that gap is documented in the tracking issue.
//! - `Write`: no additional restriction beyond existing policy layers.
//!   Implement, fix, and plain (kind-less) tasks.
//!
//! This layer composes with, and never overrides, the other authorization
//! layers in `ToolRegistry::execute`: the session tool policy, the ambient
//! action tier, and the execution directory scope. Ambient tiers rank unattended
//! action risk; assignment grants define worker authority. Each layer can only deny.

use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::{LazyLock, RwLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Grant {
    ReadOnly,
    Verify,
    Write,
}

impl Grant {
    /// Derive the grant from a plan node's kind. `None` is a plain task with
    /// status-quo authority. An unrecognized kind is a contract violation
    /// somewhere upstream, so it gets the most restricted grant rather than
    /// silently full authority.
    pub(crate) fn from_node_kind(kind: Option<&str>) -> Self {
        match kind {
            None => Self::Write,
            Some(kind) => match kind.trim().to_ascii_lowercase().as_str() {
                "implement" | "fix" => Self::Write,
                "verify" => Self::Verify,
                "explore" | "critique" | "synthesize" => Self::ReadOnly,
                "" => Self::Write,
                _ => Self::ReadOnly,
            },
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::Verify => "verify",
            Self::Write => "write",
        }
    }
}

impl fmt::Display for Grant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Debug)]
struct SessionGrant {
    grant: Grant,
    swarm_id: String,
    task_id: String,
}

static SESSION_GRANTS: LazyLock<RwLock<HashMap<String, SessionGrant>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Install the grant for an assigned worker session. Returns the previous
/// assignment's `(swarm_id, task_id)` when a stale entry was replaced, so the
/// caller can log it: a stale entry means some earlier run ended without
/// clearing, which is worth knowing about but must not wedge the worker.
pub(crate) fn install_assignment_grant(
    session_id: &str,
    grant: Grant,
    swarm_id: &str,
    task_id: &str,
) -> Option<(String, String)> {
    let mut grants = SESSION_GRANTS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = grants.insert(
        session_id.to_string(),
        SessionGrant {
            grant,
            swarm_id: swarm_id.to_string(),
            task_id: task_id.to_string(),
        },
    );
    previous
        .filter(|prior| prior.task_id != task_id || prior.swarm_id != swarm_id)
        .map(|prior| (prior.swarm_id, prior.task_id))
}

pub(crate) fn clear_session_grant(session_id: &str) {
    SESSION_GRANTS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(session_id);
}

/// Clear the binding only when it still belongs to the given assignment, so a
/// binding already replaced by a newer assignment survives an older run's
/// cleanup.
pub(crate) fn clear_session_grant_for_task(session_id: &str, swarm_id: &str, task_id: &str) {
    let mut grants = SESSION_GRANTS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if grants
        .get(session_id)
        .is_some_and(|binding| binding.swarm_id == swarm_id && binding.task_id == task_id)
    {
        grants.remove(session_id);
    }
}

/// Clear every session bound to the given task. Called when the task reaches
/// a terminal state, so no worker (headed or headless, current or stale
/// assignee) retains authority derived from a finished node.
pub(crate) fn clear_assignment_grant(swarm_id: &str, task_id: &str) {
    SESSION_GRANTS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|_, binding| binding.swarm_id != swarm_id || binding.task_id != task_id);
}

/// Move a binding across a session rename, so a reattached worker keeps the
/// grant of its still-assigned node instead of silently regaining full
/// authority under the new session id.
pub(crate) fn rename_session_grant(old_session_id: &str, new_session_id: &str) {
    let mut grants = SESSION_GRANTS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(binding) = grants.remove(old_session_id) {
        grants.insert(new_session_id.to_string(), binding);
    }
}

/// Test-visible read of a session's current binding.
#[cfg(test)]
pub(crate) fn session_grant_binding(session_id: &str) -> Option<(Grant, String, String)> {
    SESSION_GRANTS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(session_id)
        .map(|binding| {
            (
                binding.grant,
                binding.swarm_id.clone(),
                binding.task_id.clone(),
            )
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GrantError {
    session_id: String,
    grant: Grant,
    tool: String,
    swarm_id: String,
    task_id: String,
    detail: String,
}

impl fmt::Display for GrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tool '{}' was refused by the {} assignment grant for task '{}' in swarm '{}': {}. \
             The grant comes from the assigned node's kind and cannot be raised from inside \
             the session; report findings with the swarm tool instead",
            self.tool, self.grant, self.task_id, self.swarm_id, self.detail
        )
    }
}

impl std::error::Error for GrantError {}

/// Swarm actions that observe, report, or complete assigned graph work.
/// Everything else on the swarm tool either creates agents, reassigns work,
/// or mutates the graph beyond the caller's own node, and is denied under a
/// restricted grant.
fn swarm_action_allowed(action: &str) -> bool {
    matches!(
        action,
        "message"
            | "broadcast"
            | "dm"
            | "list"
            | "status"
            | "report"
            | "plan_status"
            | "summary"
            | "read_context"
            | "complete_node"
            | "expand_node"
            | "inject_gap"
            | "await_members"
            | "list_models"
            | "list_swarms"
    )
}

/// Tools with no filesystem or agent-topology effects, safe under any grant.
/// Mirrors the neutral set in `execution_scope`, plus the read tools.
fn tool_read_safe(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read"
            | "ls"
            | "agentgrep"
            | "batch"
            | "bg"
            | "conversation_search"
            | "initiative"
            | "invalid"
            | "memory"
            | "session_search"
            | "skill_manage"
            | "todo"
            | "webfetch"
            | "websearch"
    )
}

/// Enforce the session's assignment grant for one tool call. `Ok(())` when the
/// session has no installed grant (not an assigned plan worker) or the grant
/// permits the call. Fail closed: under a restricted grant, a tool that is not
/// affirmatively known to be safe is denied.
pub(crate) fn authorize_tool_call(
    session_id: &str,
    tool_name: &str,
    input: &Value,
) -> Result<(), GrantError> {
    let binding = {
        let grants = SESSION_GRANTS
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match grants.get(session_id) {
            Some(binding) => binding.clone(),
            None => return Ok(()),
        }
    };

    if binding.grant == Grant::Write {
        return Ok(());
    }

    let deny = |detail: String| {
        Err(GrantError {
            session_id: session_id.to_string(),
            grant: binding.grant,
            tool: tool_name.to_string(),
            swarm_id: binding.swarm_id.clone(),
            task_id: binding.task_id.clone(),
            detail,
        })
    };

    if tool_read_safe(tool_name) {
        return Ok(());
    }

    match tool_name {
        "swarm" => {
            let action = input.get("action").and_then(Value::as_str).unwrap_or("");
            if swarm_action_allowed(action) {
                Ok(())
            } else {
                deny(format!(
                    "swarm action '{action}' creates agents or mutates work outside the \
                     assigned node"
                ))
            }
        }
        "bash" | "nix" => {
            if binding.grant == Grant::Verify {
                Ok(())
            } else {
                deny("shell execution is not part of this node's work".to_string())
            }
        }
        "write" | "edit" | "multiedit" | "patch" | "apply_patch" => {
            deny("file mutation is not part of this node's work".to_string())
        }
        _ => deny("this tool is not classified as safe for a restricted grant".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn unique_session(name: &str) -> String {
        format!("grant-test-{name}-{}", std::process::id())
    }

    #[test]
    fn scoped_clear_only_removes_the_matching_assignment() {
        let session = unique_session("scoped-clear");
        install_assignment_grant(&session, Grant::ReadOnly, "swarm-a", "task-1");
        // A clear for a different task (an older run's cleanup racing a newer
        // assignment) must leave the binding alone.
        clear_session_grant_for_task(&session, "swarm-a", "task-2");
        assert!(session_grant_binding(&session).is_some());
        clear_session_grant_for_task(&session, "swarm-a", "task-1");
        assert!(session_grant_binding(&session).is_none());
    }

    #[test]
    fn task_clear_removes_every_session_bound_to_the_task() {
        let first = unique_session("task-clear-a");
        let second = unique_session("task-clear-b");
        let unrelated = unique_session("task-clear-other");
        install_assignment_grant(&first, Grant::ReadOnly, "swarm-t", "task-x");
        install_assignment_grant(&second, Grant::Verify, "swarm-t", "task-x");
        install_assignment_grant(&unrelated, Grant::ReadOnly, "swarm-t", "task-y");
        clear_assignment_grant("swarm-t", "task-x");
        assert!(session_grant_binding(&first).is_none());
        assert!(session_grant_binding(&second).is_none());
        assert!(
            session_grant_binding(&unrelated).is_some(),
            "a different task's binding must survive"
        );
        clear_session_grant(&unrelated);
    }

    #[test]
    fn rename_moves_the_binding_to_the_new_session_id() {
        let old_id = unique_session("rename-old");
        let new_id = unique_session("rename-new");
        install_assignment_grant(&old_id, Grant::ReadOnly, "swarm-r", "task-r");
        rename_session_grant(&old_id, &new_id);
        assert!(session_grant_binding(&old_id).is_none());
        let (grant, swarm, task) =
            session_grant_binding(&new_id).expect("binding follows the rename");
        assert_eq!(grant, Grant::ReadOnly);
        assert_eq!(swarm, "swarm-r");
        assert_eq!(task, "task-r");
        // The renamed session is still enforced, not just recorded.
        let denied = authorize_tool_call(&new_id, "edit", &json!({}));
        assert!(denied.is_err(), "grant must keep denying under the new id");
        clear_session_grant(&new_id);
    }

    #[test]
    fn derivation_matrix_maps_kinds_to_grants() {
        assert_eq!(Grant::from_node_kind(Some("explore")), Grant::ReadOnly);
        assert_eq!(Grant::from_node_kind(Some("critique")), Grant::ReadOnly);
        assert_eq!(Grant::from_node_kind(Some("synthesize")), Grant::ReadOnly);
        assert_eq!(Grant::from_node_kind(Some("verify")), Grant::Verify);
        assert_eq!(Grant::from_node_kind(Some("implement")), Grant::Write);
        assert_eq!(Grant::from_node_kind(Some("fix")), Grant::Write);
        assert_eq!(Grant::from_node_kind(None), Grant::Write);
    }

    #[test]
    fn unknown_kind_gets_most_restricted_grant() {
        assert_eq!(
            Grant::from_node_kind(Some("deploy-to-prod")),
            Grant::ReadOnly
        );
        // Case and whitespace do not change the mapping.
        assert_eq!(Grant::from_node_kind(Some(" Explore ")), Grant::ReadOnly);
    }

    #[test]
    fn unbound_session_is_unrestricted() {
        let session = unique_session("unbound");
        assert!(authorize_tool_call(&session, "write", &json!({})).is_ok());
        assert!(authorize_tool_call(&session, "bash", &json!({})).is_ok());
    }

    #[test]
    fn read_only_grant_denies_mutation_and_shell() {
        let session = unique_session("readonly");
        install_assignment_grant(&session, Grant::ReadOnly, "swarm-a", "n1");
        for tool in [
            "write",
            "edit",
            "multiedit",
            "patch",
            "apply_patch",
            "bash",
            "nix",
        ] {
            let error = authorize_tool_call(&session, tool, &json!({}))
                .expect_err("mutating tool must be denied under read-only grant");
            assert!(error.to_string().contains("read-only"), "{error}");
        }
        for tool in ["read", "ls", "agentgrep", "todo", "websearch"] {
            assert!(
                authorize_tool_call(&session, tool, &json!({})).is_ok(),
                "read tool '{tool}' must stay allowed"
            );
        }
        clear_session_grant(&session);
    }

    #[test]
    fn verify_grant_allows_shell_but_not_file_mutation() {
        let session = unique_session("verify");
        install_assignment_grant(&session, Grant::Verify, "swarm-a", "gate1");
        assert!(authorize_tool_call(&session, "bash", &json!({"command": "true"})).is_ok());
        assert!(authorize_tool_call(&session, "nix", &json!({})).is_ok());
        for tool in ["write", "edit", "multiedit", "patch", "apply_patch"] {
            assert!(
                authorize_tool_call(&session, tool, &json!({})).is_err(),
                "file mutation tool '{tool}' must be denied under verify grant"
            );
        }
        clear_session_grant(&session);
    }

    #[test]
    fn restricted_grant_gates_swarm_actions() {
        let session = unique_session("swarm-actions");
        install_assignment_grant(&session, Grant::ReadOnly, "swarm-a", "n2");
        for action in ["report", "complete_node", "expand_node", "dm", "status"] {
            assert!(
                authorize_tool_call(&session, "swarm", &json!({"action": action})).is_ok(),
                "swarm action '{action}' must stay allowed"
            );
        }
        for action in [
            "spawn",
            "run_plan",
            "task_graph",
            "assign_task",
            "stop",
            "freeze",
        ] {
            assert!(
                authorize_tool_call(&session, "swarm", &json!({"action": action})).is_err(),
                "swarm action '{action}' must be denied under a restricted grant"
            );
        }
        clear_session_grant(&session);
    }

    #[test]
    fn unclassified_tool_is_denied_under_restricted_grant() {
        let session = unique_session("unknown-tool");
        install_assignment_grant(&session, Grant::ReadOnly, "swarm-a", "n3");
        assert!(authorize_tool_call(&session, "mcp__anything__doit", &json!({})).is_err());
        assert!(authorize_tool_call(&session, "subagent", &json!({})).is_err());
        clear_session_grant(&session);
    }

    #[test]
    fn clear_restores_full_authority() {
        let session = unique_session("lifecycle");
        install_assignment_grant(&session, Grant::ReadOnly, "swarm-a", "n4");
        assert!(authorize_tool_call(&session, "edit", &json!({})).is_err());
        clear_session_grant(&session);
        assert!(authorize_tool_call(&session, "edit", &json!({})).is_ok());
    }

    #[test]
    fn install_reports_replaced_stale_assignment() {
        let session = unique_session("stale");
        assert!(install_assignment_grant(&session, Grant::ReadOnly, "swarm-a", "n5").is_none());
        // Same assignment re-installed: not stale.
        assert!(install_assignment_grant(&session, Grant::ReadOnly, "swarm-a", "n5").is_none());
        let stale = install_assignment_grant(&session, Grant::Write, "swarm-b", "n6");
        assert_eq!(stale, Some(("swarm-a".to_string(), "n5".to_string())));
        clear_session_grant(&session);
    }
}
