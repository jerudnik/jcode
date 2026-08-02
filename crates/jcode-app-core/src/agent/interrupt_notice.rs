//! What the model is told when a turn is cut short.
//!
//! A cancel and a server reload both interrupt a tool call, but they are not
//! the same event: only a reload restarts the process, so only a reload can
//! honestly promise the work is resumable. Keeping these notices together, and
//! away from the streaming loop, is what makes that distinction reviewable.

use crate::agent::ToolCall;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReloadInterruptedToolResult {
    pub(super) message: String,
    pub(super) tool_is_error: bool,
    pub(super) evidence_status: jcode_session_types::SessionLogStatus,
    pub(super) evidence_error_class: Option<&'static str>,
}

/// Notice recorded for tool calls that never ran because the turn was cut
/// short. Says which of the two causes actually happened.
pub(super) fn skipped_tool_notice(is_reload: bool) -> &'static str {
    if is_reload {
        "[Skipped - server reloading]"
    } else {
        "[Skipped - cancelled]"
    }
}

pub(super) fn interrupted_tool_result(
    tc: &ToolCall,
    elapsed_secs: f64,
    is_reload: bool,
) -> ReloadInterruptedToolResult {
    // A cancel is not a restart. Resumability and the "selfdev asked for this"
    // exemption only hold for an actual reload, so a cancel gets a plain,
    // truthful message instead of an invitation to resume.
    if !is_reload {
        return ReloadInterruptedToolResult {
            message: format!(
                "[Tool '{}' interrupted by cancel after {:.1}s]",
                tc.name, elapsed_secs
            ),
            tool_is_error: true,
            evidence_status: jcode_session_types::SessionLogStatus::Interrupted,
            evidence_error_class: Some("interrupted_by_cancel"),
        };
    }

    if tc.name == "selfdev" {
        return ReloadInterruptedToolResult {
            message: "Reload initiated. Process restarting...".to_string(),
            tool_is_error: false,
            evidence_status: jcode_session_types::SessionLogStatus::Ok,
            evidence_error_class: None,
        };
    }

    let action = tc
        .input
        .get("action")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let is_wait_like = (tc.name == "bg" && action == "wait")
        || (tc.name == "swarm" && matches!(action, "await_members" | "run_plan"));

    if is_wait_like {
        let input = serde_json::to_string(&tc.input).unwrap_or_else(|_| "{}".to_string());
        return ReloadInterruptedToolResult {
            message: format!(
                "[Tool '{}' wait interrupted by server reload after {:.1}s. The underlying operation may still be running. Resume the wait by rerunning the same tool call with input: {}]",
                tc.name, elapsed_secs, input
            ),
            tool_is_error: true,
            evidence_status: jcode_session_types::SessionLogStatus::Interrupted,
            evidence_error_class: Some("resumable_interrupted_wait"),
        };
    }

    ReloadInterruptedToolResult {
        message: format!(
            "[Tool '{}' interrupted by server reload after {:.1}s]",
            tc.name, elapsed_secs
        ),
        tool_is_error: true,
        evidence_status: jcode_session_types::SessionLogStatus::Interrupted,
        evidence_error_class: Some("interrupted_by_reload"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool_call(name: &str, input: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "toolu_test".to_string(),
            name: name.to_string(),
            input,
            intent: None,
            thought_signature: None,
        }
    }

    #[test]
    fn reload_interrupted_bg_wait_is_interrupted_and_resumable() {
        let tc = tool_call(
            "bg",
            json!({"action": "wait", "task_id": "bg-123", "max_wait_seconds": 300}),
        );

        let result = interrupted_tool_result(&tc, 1.2, true);

        assert!(
            result.tool_is_error,
            "resumable waits must be visible to downstream renderers as not completed"
        );
        assert_eq!(
            result.evidence_status,
            jcode_session_types::SessionLogStatus::Interrupted,
            "resumable waits must not be recorded as completed work"
        );
        assert_eq!(
            result.evidence_error_class,
            Some("resumable_interrupted_wait")
        );
        assert!(result.message.contains("Resume the wait"));
        assert!(result.message.contains("may still be running"));
        assert!(result.message.contains("\"task_id\":\"bg-123\""));
        assert!(result.message.contains("\"max_wait_seconds\":300"));
    }

    /// R05-FIX-1 (user-visible half): the tool result handed back to the model
    /// names the cause that actually happened, and only a real reload
    /// advertises the interrupted work as resumable. Before the fix both
    /// causes produced the reload wording, because the only available input
    /// was a single shared bit.
    #[test]
    fn interrupted_tool_result_reports_cancel_and_reload_differently() {
        let tc = tool_call("bash", json!({"command": "sleep 10"}));

        let cancelled = interrupted_tool_result(&tc, 1.2, false);
        assert!(
            !cancelled.message.contains("server reload"),
            "cancel must not claim a server reload: {}",
            cancelled.message
        );
        assert!(cancelled.message.contains("interrupted by cancel"));
        assert_eq!(
            cancelled.evidence_error_class,
            Some("interrupted_by_cancel")
        );

        let reloaded = interrupted_tool_result(&tc, 1.2, true);
        assert!(reloaded.message.contains("interrupted by server reload"));
        assert_eq!(reloaded.evidence_error_class, Some("interrupted_by_reload"));
    }

    /// A cancelled wait-like tool must not be told it can resume: the process
    /// is not restarting, so the underlying operation is simply gone.
    #[test]
    fn cancelled_wait_like_tool_is_not_advertised_as_resumable() {
        let tc = tool_call("bg", json!({"action": "wait", "task_id": "bg-123"}));

        let cancelled = interrupted_tool_result(&tc, 1.2, false);
        assert!(
            !cancelled.message.contains("Resume the wait"),
            "cancelled wait must not advertise reload-style resumability: {}",
            cancelled.message
        );

        // The reload case keeps its resume affordance.
        assert!(
            interrupted_tool_result(&tc, 1.2, true)
                .message
                .contains("Resume the wait")
        );
    }

    /// selfdev asks for its own restart, so a reload is expected and not an
    /// error. A cancel of the same tool is a genuine interruption.
    #[test]
    fn cancelled_selfdev_is_an_error_but_reload_is_expected() {
        let tc = tool_call("selfdev", json!({"action": "reload"}));

        assert!(!interrupted_tool_result(&tc, 0.5, true).tool_is_error);
        assert!(interrupted_tool_result(&tc, 0.5, false).tool_is_error);
    }

    #[test]
    fn skipped_tool_notice_names_the_cause() {
        assert_eq!(skipped_tool_notice(true), "[Skipped - server reloading]");
        assert_eq!(skipped_tool_notice(false), "[Skipped - cancelled]");
    }

    #[test]
    fn reload_interrupted_non_wait_tool_remains_error() {
        let tc = tool_call("bash", json!({"command": "sleep 10"}));

        let result = interrupted_tool_result(&tc, 1.2, true);

        assert!(result.tool_is_error);
        assert_eq!(
            result.evidence_status,
            jcode_session_types::SessionLogStatus::Interrupted
        );
        assert_eq!(result.evidence_error_class, Some("interrupted_by_reload"));
        assert!(result.message.contains("interrupted by server reload"));
    }
}
