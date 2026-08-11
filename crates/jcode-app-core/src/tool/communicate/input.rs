use super::*;

/// Coerce top-level string fields that should be non-string JSON values
/// ("true", "4", "[..]", "{..}") back to their parsed forms. Only fields whose
/// string content parses as a JSON bool/number/array/object are rewritten;
/// genuine string fields (messages, ids, labels) are left untouched because
/// bare words are not valid JSON. Used as a one-shot retry when strict
/// deserialization of a swarm tool call fails.
pub(super) fn coerce_double_encoded_fields(input: Value) -> Value {
    let Value::Object(map) = input else {
        return input;
    };
    let coerced = map
        .into_iter()
        .map(|(key, value)| {
            let new_value = match &value {
                Value::String(s) => {
                    let trimmed = s.trim();
                    let looks_structured = matches!(
                        trimmed.chars().next(),
                        Some('[' | '{' | 't' | 'f' | 'n' | '-' | '0'..='9')
                    );
                    if looks_structured {
                        match serde_json::from_str::<Value>(trimmed) {
                            Ok(parsed) if !parsed.is_string() => parsed,
                            _ => value,
                        }
                    } else {
                        value
                    }
                }
                _ => value,
            };
            (key, new_value)
        })
        .collect();
    Value::Object(coerced)
}

/// Lenient deserializer for `CommunicateInput::nodes`: accepts a JSON array of
/// node specs, a JSON-encoded string containing that array, or null/absent.
/// Harnesses and models frequently double-encode structured tool params as
/// strings; rejecting those turned an otherwise-valid seed_graph call into a
/// hard error.
pub(super) fn deserialize_nodes_lenient<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Vec<crate::protocol::TaskGraphNodeSpec>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let parsed: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
                D::Error::custom(format!(
                    "'nodes' was a string but not valid JSON: {e}. Pass a JSON array of node specs."
                ))
            })?;
            serde_json::from_value(parsed)
                .map(Some)
                .map_err(D::Error::custom)
        }
        Some(other) => serde_json::from_value(other)
            .map(Some)
            .map_err(D::Error::custom),
    }
}

#[derive(Clone, Deserialize)]
pub(super) struct CommunicateInput {
    pub(super) action: String,
    #[serde(default)]
    pub(super) message: Option<String>,
    #[serde(default)]
    pub(super) to_session: Option<String>,
    #[serde(default)]
    pub(super) proposer_session: Option<String>,
    #[serde(default)]
    pub(super) reason: Option<String>,
    #[serde(default)]
    pub(super) target_session: Option<String>,
    #[serde(default)]
    pub(super) role: Option<String>,
    #[serde(default)]
    pub(super) working_dir: Option<String>,
    #[serde(default)]
    pub(super) initial_message: Option<String>,
    #[serde(default)]
    pub(super) prompt: Option<String>,
    #[serde(default)]
    pub(super) limit: Option<usize>,
    #[serde(default)]
    pub(super) task_id: Option<String>,
    #[serde(default)]
    pub(super) spawn_if_needed: Option<bool>,
    #[serde(default)]
    pub(super) prefer_spawn: Option<bool>,
    #[serde(default)]
    pub(super) plan_items: Option<Vec<PlanItem>>,
    #[serde(default)]
    pub(super) node_id: Option<String>,
    #[serde(default)]
    pub(super) gate_id: Option<String>,
    /// Task-DAG node specs for task_graph/expand_node/inject_gap actions.
    /// Accepts either a JSON array or a JSON-encoded string of that array,
    /// because harnesses/models frequently double-encode structured params.
    #[serde(default, deserialize_with = "deserialize_nodes_lenient")]
    pub(super) nodes: Option<Vec<crate::protocol::TaskGraphNodeSpec>>,
    /// Handoff artifact (object) for complete_node.
    #[serde(default)]
    pub(super) artifact: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) target_status: Option<Vec<String>>,
    #[serde(default)]
    pub(super) session_ids: Option<Vec<String>>,
    #[serde(default)]
    pub(super) mode: Option<String>,
    /// For task_graph, explicitly replace a non-empty persisted graph.
    #[serde(default)]
    pub(super) replace_existing: Option<bool>,
    #[serde(default)]
    pub(super) timeout_minutes: Option<u64>,
    #[serde(default)]
    pub(super) wake: Option<bool>,
    #[serde(default)]
    pub(super) background: Option<bool>,
    #[serde(default)]
    pub(super) notify: Option<bool>,
    #[serde(default)]
    pub(super) delivery: Option<CommDeliveryMode>,
    #[serde(default)]
    pub(super) concurrency_limit: Option<usize>,
    #[serde(default)]
    pub(super) force: Option<bool>,
    #[serde(default)]
    pub(super) retain_agents: Option<bool>,
    #[serde(default)]
    pub(super) status: Option<String>,
    #[serde(default)]
    pub(super) validation: Option<String>,
    #[serde(default)]
    pub(super) follow_up: Option<String>,
    #[serde(default)]
    pub(super) spawn_mode: Option<String>,
    /// One-line summary shown collapsed in the recipient's UI for long
    /// message/report bodies. Required when the body exceeds the collapse
    /// threshold.
    #[serde(default)]
    pub(super) tldr: Option<String>,
    /// Per-spawn model override for spawn/assign_task/assign_next/run_plan
    /// spawns. Takes precedence over agents.swarm_model config.
    #[serde(default)]
    pub(super) model: Option<String>,
    /// Reasoning effort for spawned agents (none|low|medium|high|xhigh|max).
    #[serde(default)]
    pub(super) effort: Option<String>,
    /// Short human-readable label for a spawned agent shown in swarm UI.
    /// Required and nonblank for the explicit `spawn` action.
    #[serde(default)]
    pub(super) label: Option<String>,
    /// Free-form subagent type/role for a spawned agent (e.g. "explore",
    /// "implement", "verify", "reviewer"). Chosen per-spawn by the
    /// orchestrator. Surfaces in swarm UI for observability and injects a
    /// light role-posture nudge into the worker's first turn.
    #[serde(default)]
    pub(super) subagent_type: Option<String>,
}

impl CommunicateInput {
    pub(super) fn spawn_initial_message(&self) -> Option<String> {
        self.initial_message.clone().or_else(|| self.prompt.clone())
    }

    pub(super) fn required_spawn_label(&self) -> anyhow::Result<String> {
        let label = self
            .label
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("'label' is required for spawn action"))?
            .trim();
        if label.is_empty() {
            return Err(anyhow::anyhow!(
                "'label' must not be blank for spawn action"
            ));
        }
        Ok(label.to_string())
    }

    /// `task_graph` is a seed/new-workflow action, not an append operation.
    /// Default to replacement so persisted graph state can never leak into a
    /// fresh tool invocation. Callers extending a graph must use expand_node or
    /// inject_gap instead.
    pub(super) fn replace_existing_graph(&self) -> bool {
        self.replace_existing.unwrap_or(true)
    }
}

/// Map common action synonyms/typos to the canonical swarm action name. Models
/// frequently invent near-miss verbs (e.g. `inbox` for reading messages, `send`
/// for `message`), which previously produced an "Unknown action" error. Unknown
/// inputs are returned unchanged so the normal validation path still reports them.
pub(super) fn canonical_swarm_action(action: &str) -> &str {
    match action.trim().to_ascii_lowercase().as_str() {
        "send" | "msg" | "send_message" => "message",
        "dm_session" | "direct_message" | "whisper" => "dm",
        "broadcast_all" | "announce" => "broadcast",
        "agents" | "members" | "list_agents" | "list_members" | "roster" => "list",
        "swarms" | "fleet" | "fleet_status" | "list_fleet" => "list_swarms",
        "models" | "model_list" | "list_model" | "list_providers" | "list_routes" => "list_models",
        "plan" | "status_plan" => "plan_status",
        "seed_graph" | "seed" | "graph" | "seed_plan" | "seed_tasks" | "create_graph" => {
            "task_graph"
        }
        "execute_plan" | "start_plan" | "drive_plan" => "run_plan",
        "await" | "wait" | "wait_members" | "await_all" => "await_members",
        "assign" => "assign_task",
        "kill" | "terminate" => "stop",
        _ => action,
    }
}
