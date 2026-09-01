//! Assignment grants for swarm plan workers.
//!
//! Authority is stored on canonical plan assignment progress and snapshotted at
//! tool-call time. Ambient action tiers remain a separate risk ranking.

use serde_json::Value;
use std::fmt;

pub(crate) use jcode_plan::AssignmentGrant as Grant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GrantLookup {
    /// Human, coordinator, bootstrap, or otherwise ambiguous identity.
    Unrestricted,
    /// A known worker that currently holds no live assignment.
    Unassigned { reclaimed: bool },
    Assigned {
        grant: Grant,
        swarm_id: String,
        task_id: String,
        epoch: u64,
    },
}

fn grant_label(grant: Grant) -> &'static str {
    match grant {
        Grant::ReadOnly => "read-only",
        Grant::Verify => "verify",
        Grant::Write => "write",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GrantError {
    session_id: String,
    grant: Option<Grant>,
    tool: String,
    swarm_id: Option<String>,
    task_id: Option<String>,
    detail: String,
    reclaimed: bool,
}

impl fmt::Display for GrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(grant), Some(swarm_id), Some(task_id)) =
            (self.grant, self.swarm_id.as_deref(), self.task_id.as_deref())
        {
            return write!(
                f,
                "tool '{}' was refused by the {} assignment grant for task '{}' in swarm '{}': {}. The grant comes from the assigned node's kind and cannot be raised from inside the session; report findings with the swarm tool instead",
                self.tool,
                grant_label(grant),
                task_id,
                swarm_id,
                self.detail
            );
        }

        write!(
            f,
            "tool '{}' was refused because swarm worker session '{}' holds no live assignment: {}. Obtain one with swarm assign_next or start_task{}",
            self.tool,
            self.session_id,
            self.detail,
            if self.reclaimed {
                "; if the prior assignment was reclaimed, inspect its banked artifacts with swarm salvage"
            } else {
                ""
            }
        )
    }
}

impl std::error::Error for GrantError {}

/// Swarm actions that observe, report, or complete assigned graph work.
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

fn unassigned_swarm_action_allowed(action: &str) -> bool {
    matches!(action, "report" | "status" | "salvage" | "assign_next" | "start_task")
}

/// Tools with no filesystem or agent-topology effects, safe under any grant.
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

/// Apply policy to a lock-free assignment snapshot. The caller must acquire and
/// release swarm-state locks before invoking this function.
pub(crate) fn authorize_tool_call(
    session_id: &str,
    lookup: GrantLookup,
    tool_name: &str,
    input: &Value,
) -> Result<(), GrantError> {
    match lookup {
        GrantLookup::Unrestricted => Ok(()),
        GrantLookup::Unassigned { reclaimed } => {
            if tool_read_safe(tool_name) {
                return Ok(());
            }
            if tool_name == "swarm"
                && input
                    .get("action")
                    .and_then(Value::as_str)
                    .is_some_and(unassigned_swarm_action_allowed)
            {
                return Ok(());
            }
            Err(GrantError {
                session_id: session_id.to_string(),
                grant: None,
                tool: tool_name.to_string(),
                swarm_id: None,
                task_id: None,
                detail: "unassigned workers may only read, report, inspect status, salvage, or acquire work"
                    .to_string(),
                reclaimed,
            })
        }
        GrantLookup::Assigned {
            grant,
            swarm_id,
            task_id,
            epoch: _,
        } => {
            if grant == Grant::Write || tool_read_safe(tool_name) {
                return Ok(());
            }
            if tool_name == "swarm" {
                let action = input.get("action").and_then(Value::as_str).unwrap_or("");
                if swarm_action_allowed(action) {
                    return Ok(());
                }
                return Err(GrantError {
                    session_id: session_id.to_string(),
                    grant: Some(grant),
                    tool: tool_name.to_string(),
                    swarm_id: Some(swarm_id),
                    task_id: Some(task_id),
                    detail: format!("swarm action '{action}' mutates authority or work outside the assigned node"),
                    reclaimed: false,
                });
            }
            if grant == Grant::Verify && tool_name == "bash" {
                return Ok(());
            }
            Err(GrantError {
                session_id: session_id.to_string(),
                grant: Some(grant),
                tool: tool_name.to_string(),
                swarm_id: Some(swarm_id),
                task_id: Some(task_id),
                detail: "this tool is not in the grant's allowlist".to_string(),
                reclaimed: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assigned(grant: Grant) -> GrantLookup {
        GrantLookup::Assigned {
            grant,
            swarm_id: "swarm-a".to_string(),
            task_id: "task-a".to_string(),
            epoch: 1,
        }
    }

    #[test]
    fn denial_baseline_preserved() {
        for grant in [Grant::ReadOnly, Grant::Verify] {
            assert!(authorize_tool_call("worker", assigned(grant), "read", &json!({})).is_ok());
            assert!(authorize_tool_call(
                "worker",
                assigned(grant),
                "swarm",
                &json!({"action": "report"})
            )
            .is_ok());
            assert!(authorize_tool_call("worker", assigned(grant), "edit", &json!({})).is_err());
            assert!(authorize_tool_call(
                "worker",
                assigned(grant),
                "swarm",
                &json!({"action": "spawn"})
            )
            .is_err());
        }
        assert!(authorize_tool_call("worker", assigned(Grant::Verify), "bash", &json!({})).is_ok());
        assert!(authorize_tool_call("worker", assigned(Grant::ReadOnly), "bash", &json!({})).is_err());
        assert!(authorize_tool_call("worker", assigned(Grant::Write), "edit", &json!({})).is_ok());
    }

    #[test]
    fn unassigned_worker_denied_non_read_safe() {
        let lookup = GrantLookup::Unassigned { reclaimed: false };
        assert!(authorize_tool_call("worker", lookup.clone(), "read", &json!({})).is_ok());
        assert!(authorize_tool_call("worker", lookup, "edit", &json!({})).is_err());
    }

    #[test]
    fn unassigned_worker_baseline_allowed() {
        for action in ["report", "status", "salvage", "assign_next", "start_task"] {
            assert!(authorize_tool_call(
                "worker",
                GrantLookup::Unassigned { reclaimed: false },
                "swarm",
                &json!({"action": action})
            )
            .is_ok());
        }
        assert!(authorize_tool_call(
            "worker",
            GrantLookup::Unassigned { reclaimed: false },
            "swarm",
            &json!({"action": "spawn"})
        )
        .is_err());
    }

    #[test]
    fn denial_message_names_remedy() {
        let error = authorize_tool_call(
            "worker",
            GrantLookup::Unassigned { reclaimed: true },
            "edit",
            &json!({}),
        )
        .expect_err("reclaimed worker must be denied");
        let message = error.to_string();
        assert!(message.contains("assign_next or start_task"));
        assert!(message.contains("swarm salvage"));
    }

    #[test]
    fn non_worker_identity_is_unaffected() {
        assert!(authorize_tool_call(
            "human-or-coordinator",
            GrantLookup::Unrestricted,
            "edit",
            &json!({})
        )
        .is_ok());
    }
}
