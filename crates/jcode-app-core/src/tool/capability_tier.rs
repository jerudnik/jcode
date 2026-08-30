//! Server-derived capability tiers for swarm plan workers.
//!
//! A worker's authority comes from what its plan node is for, not from its
//! prompt. The server derives a tier from the node's kind at assignment time
//! and enforces it in the tool dispatch path, so a worker whose prompt says
//! "you may edit files" still cannot edit files when its node is an explore,
//! critique, or synthesize node. Nothing a worker sends over the wire can
//! raise its own tier: installation happens only in the server's assignment
//! seam, and the registry is keyed by session id.
//!
//! Tier meanings:
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
//! action tier, and the execution directory scope. Each layer can only deny.

use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::{LazyLock, RwLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapabilityTier {
    ReadOnly,
    Verify,
    Write,
}

impl CapabilityTier {
    /// Derive the tier from a plan node's kind. `None` is a plain task with
    /// status-quo authority. An unrecognized kind is a contract violation
    /// somewhere upstream, so it gets the most restricted tier rather than
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

impl fmt::Display for CapabilityTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Debug)]
struct SessionCapability {
    tier: CapabilityTier,
    swarm_id: String,
    task_id: String,
}

static SESSION_CAPABILITIES: LazyLock<RwLock<HashMap<String, SessionCapability>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Install the tier for an assigned worker session. Returns the previous
/// assignment's `(swarm_id, task_id)` when a stale entry was replaced, so the
/// caller can log it: a stale entry means some earlier run ended without
/// clearing, which is worth knowing about but must not wedge the worker.
pub(crate) fn install_session_capability(
    session_id: &str,
    tier: CapabilityTier,
    swarm_id: &str,
    task_id: &str,
) -> Option<(String, String)> {
    let mut capabilities = SESSION_CAPABILITIES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = capabilities.insert(
        session_id.to_string(),
        SessionCapability {
            tier,
            swarm_id: swarm_id.to_string(),
            task_id: task_id.to_string(),
        },
    );
    previous.filter(|prior| prior.task_id != task_id || prior.swarm_id != swarm_id)
        .map(|prior| (prior.swarm_id, prior.task_id))
}

pub(crate) fn clear_session_capability(session_id: &str) {
    SESSION_CAPABILITIES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(session_id);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapabilityTierError {
    session_id: String,
    tier: CapabilityTier,
    tool: String,
    swarm_id: String,
    task_id: String,
    detail: String,
}

impl fmt::Display for CapabilityTierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tool '{}' was refused by the {} capability tier for task '{}' in swarm '{}': {}. \
             The tier comes from the assigned node's kind and cannot be raised from inside \
             the session; report findings with the swarm tool instead",
            self.tool, self.tier, self.task_id, self.swarm_id, self.detail
        )
    }
}

impl std::error::Error for CapabilityTierError {}

/// Swarm actions that observe, report, or complete assigned graph work.
/// Everything else on the swarm tool either creates agents, reassigns work,
/// or mutates the graph beyond the caller's own node, and is denied under a
/// restricted tier.
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

/// Tools with no filesystem or agent-topology effects, safe under any tier.
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

/// Enforce the session's capability tier for one tool call. `Ok(())` when the
/// session has no installed tier (not an assigned plan worker) or the tier
/// permits the call. Fail closed: under a restricted tier, a tool that is not
/// affirmatively known to be safe is denied.
pub(crate) fn authorize_tool_call(
    session_id: &str,
    tool_name: &str,
    input: &Value,
) -> Result<(), CapabilityTierError> {
    let capability = {
        let capabilities = SESSION_CAPABILITIES
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match capabilities.get(session_id) {
            Some(capability) => capability.clone(),
            None => return Ok(()),
        }
    };

    if capability.tier == CapabilityTier::Write {
        return Ok(());
    }

    let deny = |detail: String| {
        Err(CapabilityTierError {
            session_id: session_id.to_string(),
            tier: capability.tier,
            tool: tool_name.to_string(),
            swarm_id: capability.swarm_id.clone(),
            task_id: capability.task_id.clone(),
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
            if capability.tier == CapabilityTier::Verify {
                Ok(())
            } else {
                deny("shell execution is not part of this node's work".to_string())
            }
        }
        "write" | "edit" | "multiedit" | "patch" | "apply_patch" => {
            deny("file mutation is not part of this node's work".to_string())
        }
        _ => deny("this tool is not classified as safe for a restricted tier".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn unique_session(name: &str) -> String {
        format!("cap-tier-test-{name}-{}", std::process::id())
    }

    #[test]
    fn derivation_matrix_maps_kinds_to_tiers() {
        assert_eq!(
            CapabilityTier::from_node_kind(Some("explore")),
            CapabilityTier::ReadOnly
        );
        assert_eq!(
            CapabilityTier::from_node_kind(Some("critique")),
            CapabilityTier::ReadOnly
        );
        assert_eq!(
            CapabilityTier::from_node_kind(Some("synthesize")),
            CapabilityTier::ReadOnly
        );
        assert_eq!(
            CapabilityTier::from_node_kind(Some("verify")),
            CapabilityTier::Verify
        );
        assert_eq!(
            CapabilityTier::from_node_kind(Some("implement")),
            CapabilityTier::Write
        );
        assert_eq!(
            CapabilityTier::from_node_kind(Some("fix")),
            CapabilityTier::Write
        );
        assert_eq!(CapabilityTier::from_node_kind(None), CapabilityTier::Write);
    }

    #[test]
    fn unknown_kind_gets_most_restricted_tier() {
        assert_eq!(
            CapabilityTier::from_node_kind(Some("deploy-to-prod")),
            CapabilityTier::ReadOnly
        );
        // Case and whitespace do not change the mapping.
        assert_eq!(
            CapabilityTier::from_node_kind(Some(" Explore ")),
            CapabilityTier::ReadOnly
        );
    }

    #[test]
    fn unbound_session_is_unrestricted() {
        let session = unique_session("unbound");
        assert!(authorize_tool_call(&session, "write", &json!({})).is_ok());
        assert!(authorize_tool_call(&session, "bash", &json!({})).is_ok());
    }

    #[test]
    fn read_only_tier_denies_mutation_and_shell() {
        let session = unique_session("readonly");
        install_session_capability(&session, CapabilityTier::ReadOnly, "swarm-a", "n1");
        for tool in ["write", "edit", "multiedit", "patch", "apply_patch", "bash", "nix"] {
            let error = authorize_tool_call(&session, tool, &json!({}))
                .expect_err("mutating tool must be denied under read-only tier");
            assert!(error.to_string().contains("read-only"), "{error}");
        }
        for tool in ["read", "ls", "agentgrep", "todo", "websearch"] {
            assert!(
                authorize_tool_call(&session, tool, &json!({})).is_ok(),
                "read tool '{tool}' must stay allowed"
            );
        }
        clear_session_capability(&session);
    }

    #[test]
    fn verify_tier_allows_shell_but_not_file_mutation() {
        let session = unique_session("verify");
        install_session_capability(&session, CapabilityTier::Verify, "swarm-a", "gate1");
        assert!(authorize_tool_call(&session, "bash", &json!({"command": "true"})).is_ok());
        assert!(authorize_tool_call(&session, "nix", &json!({})).is_ok());
        for tool in ["write", "edit", "multiedit", "patch", "apply_patch"] {
            assert!(
                authorize_tool_call(&session, tool, &json!({})).is_err(),
                "file mutation tool '{tool}' must be denied under verify tier"
            );
        }
        clear_session_capability(&session);
    }

    #[test]
    fn restricted_tier_gates_swarm_actions() {
        let session = unique_session("swarm-actions");
        install_session_capability(&session, CapabilityTier::ReadOnly, "swarm-a", "n2");
        for action in ["report", "complete_node", "expand_node", "dm", "status"] {
            assert!(
                authorize_tool_call(&session, "swarm", &json!({"action": action})).is_ok(),
                "swarm action '{action}' must stay allowed"
            );
        }
        for action in ["spawn", "run_plan", "task_graph", "assign_task", "stop", "freeze"] {
            assert!(
                authorize_tool_call(&session, "swarm", &json!({"action": action})).is_err(),
                "swarm action '{action}' must be denied under a restricted tier"
            );
        }
        clear_session_capability(&session);
    }

    #[test]
    fn unclassified_tool_is_denied_under_restricted_tier() {
        let session = unique_session("unknown-tool");
        install_session_capability(&session, CapabilityTier::ReadOnly, "swarm-a", "n3");
        assert!(authorize_tool_call(&session, "mcp__anything__doit", &json!({})).is_err());
        assert!(authorize_tool_call(&session, "subagent", &json!({})).is_err());
        clear_session_capability(&session);
    }

    #[test]
    fn clear_restores_full_authority() {
        let session = unique_session("lifecycle");
        install_session_capability(&session, CapabilityTier::ReadOnly, "swarm-a", "n4");
        assert!(authorize_tool_call(&session, "edit", &json!({})).is_err());
        clear_session_capability(&session);
        assert!(authorize_tool_call(&session, "edit", &json!({})).is_ok());
    }

    #[test]
    fn install_reports_replaced_stale_assignment() {
        let session = unique_session("stale");
        assert!(
            install_session_capability(&session, CapabilityTier::ReadOnly, "swarm-a", "n5")
                .is_none()
        );
        // Same assignment re-installed: not stale.
        assert!(
            install_session_capability(&session, CapabilityTier::ReadOnly, "swarm-a", "n5")
                .is_none()
        );
        let stale = install_session_capability(&session, CapabilityTier::Write, "swarm-b", "n6");
        assert_eq!(stale, Some(("swarm-a".to_string(), "n5".to_string())));
        clear_session_capability(&session);
    }
}
